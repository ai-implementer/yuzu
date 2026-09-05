//! `yuzu check --external-links`: 外部リンク（http / https）の到達性検査（opt-in）。
//!
//! HTTP は **curl へ委譲する**。workspace には HTTP クライアントも TLS も無く、
//! `ureq` + `rustls` を足すと ring の C/asm ビルドと 20 超の crate が配布バイナリと
//! 4 プラットフォームのリリースビルドに入る（comrak / syntect で onig を避けたのと
//! 同じ規律）。opt-in の検査だけが外部ツールに依存し、既定経路は決定的・オフラインの
//! まま（凍結した設計判断「ネットワーク I/O は既定経路に入れない」）。
//!
//! 分類（`--format json` の契約を環境依存にしないため）:
//! - HTTP 4xx（429 を除く）→ `external-link-broken`（warning・抑制可）を出現箇所ごとに
//! - DNS 失敗・タイムアウト・TLS エラー・5xx・429・curl の失敗 → 診断にせず
//!   `summary.skipped`（URL 単位）に計上し、理由は warn ログへ
//! - curl が無い → 実行エラー（exit 2）
//!
//! 同一 URL は 1 回だけ取得する（順序は BTreeMap で決定的。診断の並びは
//! `diag::report` が確定させる）

use std::collections::BTreeMap;
use std::process::Command;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use anyhow::{Context as _, bail};
use yuzu_core::{DiagBase, Diagnostic, ExternalLink, rules};

/// 検査結果。`diags` は出現箇所ごと、`skipped` は検査できなかった URL の数
pub struct Outcome {
    pub diags: Vec<Diagnostic>,
    pub skipped: usize,
}

/// 同時に走らせる curl の数
const WORKERS: usize = 8;
/// 接続確立までの上限秒
const CONNECT_TIMEOUT_SECS: &str = "10";
/// 1 URL の全体上限秒（リダイレクト込み）
const MAX_TIME_SECS: &str = "20";
const USER_AGENT: &str = concat!("yuzu-linkcheck/", env!("CARGO_PKG_VERSION"));
/// ブラウザ相当の Accept（crates.io 等は無いと 404 を返す）
const ACCEPT: &str = "Accept: text/html,application/xhtml+xml,*/*;q=0.8";

pub fn check(links: &[ExternalLink]) -> anyhow::Result<Outcome> {
    // URL → 出現箇所（同じ URL は 1 回だけ取得し、結果を全出現箇所へ配る）
    let mut by_url: BTreeMap<&str, Vec<&ExternalLink>> = BTreeMap::new();
    for link in links {
        by_url.entry(link.url.as_str()).or_default().push(link);
    }
    let urls: Vec<&str> = by_url.keys().copied().collect();
    let probes = probe_all(&urls)?;

    let mut diags = Vec::new();
    let mut skipped = 0;
    for (url, probe) in urls.iter().zip(&probes) {
        match classify(probe) {
            Verdict::Ok => {}
            Verdict::Broken(status) => {
                for link in &by_url[url] {
                    diags.push(Diagnostic {
                        rule: rules::EXTERNAL_LINK_BROKEN.id,
                        severity: rules::EXTERNAL_LINK_BROKEN.severity,
                        base: DiagBase::Content,
                        rel: link.rel.clone(),
                        span: Some(link.span),
                        message: format!("外部リンク `{url}` が HTTP {status} を返しました"),
                        fix: None,
                    });
                }
            }
            Verdict::Skipped(reason) => {
                skipped += 1;
                tracing::warn!(url = %url, reason = %reason, "外部リンクを検査できませんでした（スキップ）");
            }
        }
    }
    Ok(Outcome { diags, skipped })
}

/// curl 1 回の結果
#[derive(Debug, Clone, PartialEq, Eq)]
enum Probe {
    /// 最終応答の HTTP ステータス（リダイレクトは追った後）
    Status(u16),
    /// 応答が得られなかった（DNS・接続・タイムアウト・TLS 等。curl の stderr）
    Failed(String),
}

#[derive(Debug, PartialEq, Eq)]
enum Verdict {
    Ok,
    Broken(u16),
    Skipped(String),
}

fn classify(probe: &Probe) -> Verdict {
    match probe {
        Probe::Status(s) if (200..400).contains(s) => Verdict::Ok,
        // レート制限は相手側の一時的な状態（再実行で消える）
        Probe::Status(429) => Verdict::Skipped("HTTP 429（レート制限）".to_string()),
        Probe::Status(s) if (400..500).contains(s) => Verdict::Broken(*s),
        // 5xx・1xx・想定外はリンクの問題と断定できない
        Probe::Status(s) => Verdict::Skipped(format!("HTTP {s}")),
        Probe::Failed(reason) => Verdict::Skipped(reason.clone()),
    }
}

/// URL ごとに curl を実行する（[`WORKERS`] 並列。結果は入力と同じ順）。
/// curl を起動できなければ Err（PATH に無いのは実行エラー）
fn probe_all(urls: &[&str]) -> anyhow::Result<Vec<Probe>> {
    let results: Vec<Mutex<Option<Probe>>> = urls.iter().map(|_| Mutex::new(None)).collect();
    let next = AtomicUsize::new(0);
    let spawn_error: Mutex<Option<std::io::Error>> = Mutex::new(None);
    std::thread::scope(|scope| {
        for _ in 0..WORKERS.min(urls.len()) {
            scope.spawn(|| {
                loop {
                    let i = next.fetch_add(1, Ordering::SeqCst);
                    if i >= urls.len() {
                        break;
                    }
                    match probe(urls[i]) {
                        Ok(p) => *results[i].lock().unwrap() = Some(p),
                        Err(e) => {
                            *spawn_error.lock().unwrap() = Some(e);
                            break;
                        }
                    }
                }
            });
        }
    });
    if let Some(e) = spawn_error.into_inner().unwrap() {
        if e.kind() == std::io::ErrorKind::NotFound {
            bail!("外部リンク検査には curl が必要です（PATH に見つかりません）");
        }
        return Err(e).context("curl の起動に失敗しました");
    }
    Ok(results
        .into_iter()
        .map(|m| m.into_inner().unwrap().expect("全 URL を処理済み"))
        .collect())
}

/// curl で 1 URL を GET し、最終ステータスを読む。
/// `-o` で本文を捨て `-w %{http_code}` だけを標準出力に出させる
/// （HEAD は拒否するサーバが多いので使わない）
fn probe(url: &str) -> std::io::Result<Probe> {
    let null = if cfg!(windows) { "NUL" } else { "/dev/null" };
    let output = Command::new("curl")
        .args([
            "-sS",
            // URL のグロブ展開を止める。`?q=[1-2]` は 2 回取得されて `%{http_code}` が
            // `404404` に連結され、`?filter[name]=x` は構文エラーになり、どちらも
            // 壊れたリンクなのに skipped 扱いになる（レビュー指摘）
            "--globoff",
            "-o",
            null,
            "-L",
            "--max-redirs",
            "10",
            "--connect-timeout",
            CONNECT_TIMEOUT_SECS,
            "--max-time",
            MAX_TIME_SECS,
            "-A",
            USER_AGENT,
            // crates.io はブラウザ相当の Accept が無いと 404 を返す（docs 自身の
            // dogfooding で判明）。他のサイトでも「HTML を求めている」ことを明示する
            "-H",
            ACCEPT,
            "-w",
            "%{http_code}",
            "--",
            url,
        ])
        .output()?;
    let code = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let reason = if stderr.is_empty() {
            format!(
                "curl が終了コード {} で失敗しました",
                output.status.code().unwrap_or(-1)
            )
        } else {
            stderr
        };
        return Ok(Probe::Failed(reason));
    }
    match code.parse::<u16>() {
        Ok(status) if status > 0 => Ok(Probe::Status(status)),
        _ => Ok(Probe::Failed(format!(
            "curl の応答コードを解釈できません: {code:?}"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::path::PathBuf;

    use yuzu_core::SourceSpan;

    use super::*;

    /// 依存を増やさない最小 HTTP サーバ（テスト専用）。パスでステータスを決める
    fn serve() -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let mut stream = match stream {
                    Ok(s) => s,
                    Err(_) => continue,
                };
                let mut buf = [0u8; 4096];
                let n = stream.read(&mut buf).unwrap_or(0);
                let request = String::from_utf8_lossy(&buf[..n]);
                let path = request
                    .lines()
                    .next()
                    .and_then(|line| line.split_whitespace().nth(1))
                    .unwrap_or("/")
                    .to_string();
                // クエリを除いたパスで判定する（`?q=[1-2]` 付きも同じ応答）
                let (status, extra) = match path.split('?').next().unwrap_or(&path) {
                    "/ok" => ("200 OK", String::new()),
                    "/moved" => ("302 Found", "Location: /ok\r\n".to_string()),
                    "/missing" => ("404 Not Found", String::new()),
                    "/gone" => ("410 Gone", String::new()),
                    "/busy" => ("429 Too Many Requests", String::new()),
                    "/boom" => ("500 Internal Server Error", String::new()),
                    _ => ("404 Not Found", String::new()),
                };
                let _ = stream.write_all(
                    format!("HTTP/1.1 {status}\r\n{extra}Content-Length: 2\r\nConnection: close\r\n\r\nok")
                        .as_bytes(),
                );
            }
        });
        format!("http://{addr}")
    }

    fn link(rel: &str, line: usize, url: &str) -> ExternalLink {
        ExternalLink {
            rel: PathBuf::from(rel),
            span: SourceSpan {
                start_line: line,
                start_col: 1,
                end_line: line,
                end_col: 1,
            },
            url: url.to_string(),
            is_image: false,
        }
    }

    #[test]
    fn 分類は_4xx_だけを壊れ扱いにする() {
        assert_eq!(classify(&Probe::Status(200)), Verdict::Ok);
        assert_eq!(classify(&Probe::Status(301)), Verdict::Ok);
        assert_eq!(classify(&Probe::Status(404)), Verdict::Broken(404));
        assert_eq!(classify(&Probe::Status(403)), Verdict::Broken(403));
        assert_eq!(classify(&Probe::Status(410)), Verdict::Broken(410));
        assert!(matches!(classify(&Probe::Status(429)), Verdict::Skipped(_)));
        assert!(matches!(classify(&Probe::Status(500)), Verdict::Skipped(_)));
        assert!(matches!(classify(&Probe::Status(503)), Verdict::Skipped(_)));
        assert!(matches!(
            classify(&Probe::Failed("timeout".into())),
            Verdict::Skipped(_)
        ));
    }

    /// curl を実際に起動してローカルサーバへ当てる（curl が無い環境では失敗する =
    /// CI と開発コンテナには入っている）
    #[test]
    fn ローカルサーバに対して_4xx_を診断し到達不能をスキップに数える() {
        let base = serve();
        let links = vec![
            link("index.md", 3, &format!("{base}/ok")),
            link("index.md", 5, &format!("{base}/moved")),
            link("index.md", 7, &format!("{base}/missing")),
            // 同じ URL の 2 回目 = 取得は 1 回・診断は 2 件
            link("guide/a.md", 2, &format!("{base}/missing")),
            link("guide/a.md", 4, &format!("{base}/gone")),
            link("guide/a.md", 6, &format!("{base}/busy")),
            link("guide/a.md", 8, &format!("{base}/boom")),
            // 誰も listen していないポート → 接続拒否
            link("guide/a.md", 10, "http://127.0.0.1:1/"),
        ];
        let outcome = check(&links).expect("curl が動く");

        let mut got: Vec<(String, usize, &str)> = outcome
            .diags
            .iter()
            .map(|d| {
                (
                    d.rel.to_string_lossy().into_owned(),
                    d.span.unwrap().start_line,
                    d.rule,
                )
            })
            .collect();
        got.sort();
        assert_eq!(
            got,
            [
                ("guide/a.md".to_string(), 2, "external-link-broken"),
                ("guide/a.md".to_string(), 4, "external-link-broken"),
                ("index.md".to_string(), 7, "external-link-broken"),
            ],
            "{:?}",
            outcome.diags
        );
        assert!(
            outcome
                .diags
                .iter()
                .any(|d| d.message.contains("HTTP 404") && d.message.contains("/missing")),
            "{:?}",
            outcome.diags
        );
        assert_eq!(outcome.skipped, 3, "429・500・接続拒否の 3 URL");
        assert!(
            outcome
                .diags
                .iter()
                .all(|d| d.severity == yuzu_core::Severity::Warning)
        );
    }

    /// 角括弧・波括弧入りの URL を curl のグロブに解釈させず、書かれたまま 1 回だけ
    /// 取得する。グロブが効くと `[1-2]` は 2 回取得されて応答コードが `404404` に
    /// 連結され、`[name]` は構文エラーになり、どちらも skipped へ逃げてしまう
    #[test]
    fn 角括弧付きの_url_はグロブ展開せずそのまま_1_回取得する() {
        let base = serve();
        let links = vec![
            link("index.md", 3, &format!("{base}/missing?q=[1-2]")),
            link("index.md", 5, &format!("{base}/missing?filter[name]=x")),
            link("index.md", 7, &format!("{base}/ok?ids={{1,2}}")),
        ];
        let outcome = check(&links).expect("curl が動く");
        let mut lines: Vec<usize> = outcome
            .diags
            .iter()
            .map(|d| d.span.unwrap().start_line)
            .collect();
        lines.sort();
        assert_eq!(lines, [3, 5], "{:?}", outcome.diags);
        assert!(
            outcome.diags.iter().all(|d| d.message.contains("HTTP 404")),
            "{:?}",
            outcome.diags
        );
        assert_eq!(outcome.skipped, 0, "グロブ由来の失敗が skipped に化けない");
    }

    #[test]
    fn リンクが無ければ何も起動しない() {
        let outcome = check(&[]).unwrap();
        assert!(outcome.diags.is_empty());
        assert_eq!(outcome.skipped, 0);
    }
}
