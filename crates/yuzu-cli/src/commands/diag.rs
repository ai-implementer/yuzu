//! 診断の表示ヘルパ（`yuzu lint` / `yuzu check` 共通）。
//!
//! 出力形式は `--format` で選ぶ（既定 human）。フォーマッタは文字列を返す
//! 純粋関数にして、標準出力への書き出しは [`report`] だけが行う。
//! **json 形式では JSON オブジェクト以外を標準出力へ書かない**のが不変条件で、
//! これを 1 箇所で守るために集計行と終了コードの判定まで [`report`] に集約している。

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use serde::Serialize;
use yuzu_core::{DiagBase, Diagnostic, LintOptions, Severity};

use crate::out::outln;

/// `yuzu.toml` の警告（`input.dir` がルート外など。未知キー・型不一致は
/// 診断ではなく読み込みエラー）を [`Diagnostic`] へ写す。
/// yuzu-config は yuzu-core に依存しないため、変換はここで行う
pub fn config_diagnostics(rc: &yuzu_config::ResolvedConfig) -> Vec<Diagnostic> {
    rc.diagnostics
        .iter()
        .map(|d| Diagnostic {
            rule: d.rule,
            severity: Severity::Warning,
            // yuzu.toml は content の外にあるのでプロジェクトルート基点
            base: DiagBase::ProjectRoot,
            rel: std::path::PathBuf::from(yuzu_config::CONFIG_FILE_NAME),
            span: Some(yuzu_core::SourceSpan {
                start_line: d.line,
                start_col: d.col,
                end_line: d.line,
                end_col: d.col,
            }),
            message: d.message.clone(),
            fix: None,
        })
        .collect()
}

/// `yuzu.toml` の lint 設定を core の [`LintOptions`] へ写す（`lint` / `check` 共通。
/// 変換を 1 箇所に置き、片方のコマンドだけ配線されて有効ルールが食い違うのを防ぐ）。
///
/// `external_links` はこの実行で外部リンク検査を行うか（`check --external-links`
/// だけ true。`lint` は常に false）。false なら `external-link-broken` を
/// 「評価しなかったルール」として渡し、その抑制が `unused-lint-suppression` に
/// ならないようにする（例外指定のある原稿で既定のオフライン CI が落ちない）
pub fn lint_options(rc: &yuzu_config::ResolvedConfig, external_links: bool) -> LintOptions {
    let mut unevaluated_rules = std::collections::BTreeSet::new();
    if !external_links {
        unevaluated_rules.insert(yuzu_core::rules::EXTERNAL_LINK_BROKEN.id.to_string());
    }
    LintOptions {
        max_directory_depth: rc.config.lint.max_directory_depth,
        terms: rc.config.lint.terms.clone(),
        // ルール ID → bool を無解釈で写す（「不在 = 有効」の解釈は core の漏斗が持つ）
        rules: rc.config.lint.rules.clone(),
        unevaluated_rules,
        // 判定できなかった出現箇所は check が外部検査の後に埋める
        unevaluated_occurrences: Vec::new(),
    }
}

/// 診断の出力形式（`--format`）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, clap::ValueEnum)]
pub enum Format {
    /// 人向けの 1 行形式
    #[default]
    Human,
    /// 単一 JSON オブジェクト（CI が消費する契約）
    Json,
    /// GitHub Actions のワークフローコマンド（PR の diff 行へ注釈が付く）
    Github,
}

/// 出力に必要なパス情報とページ数（`check` / `lint` 共通）
pub struct Context<'a> {
    /// プロジェクトルート（絶対）
    pub root: &'a Path,
    /// content ディレクトリ（絶対）。github 形式のパス解決に使う
    pub content_dir: &'a Path,
    /// 検査したページ数（「問題ありません（N ページ）」と `summary.pages`）
    pub pages: usize,
    /// frontmatter `lintDisable` で抑制した件数（集計行と `summary.suppressed`）
    pub suppressed: usize,
    /// `lint.rules` の全体無効化で落とした件数（集計行と `summary.disabled`）
    pub disabled: usize,
    /// `check --external-links` で検査できなかった外部 URL の数（DNS 失敗・
    /// タイムアウト・5xx・429。集計行と `summary.skipped`。環境依存の失敗を
    /// 診断に混ぜず、ここへ逃がす）
    pub skipped: usize,
}

/// 診断を出力して終了コードを返す（0 = 違反なし / 1 = 違反あり）。
/// 並び順（ファイル → 行 → 列）はここで確定させるので呼び出し側はソート不要
pub fn report(
    format: Format,
    mut diags: Vec<Diagnostic>,
    ctx: &Context,
) -> anyhow::Result<ExitCode> {
    sort_diagnostics(&mut diags);
    // human / json のパスは従来どおりプロジェクトルート相対
    let prefix = ctx
        .content_dir
        .strip_prefix(ctx.root)
        .unwrap_or(ctx.content_dir);
    let (errors, warnings) = counts(&diags);

    match format {
        Format::Human => {
            for line in human_lines(&diags, prefix, Path::new("")) {
                outln!("{line}");
            }
            print_summary(&diags, ctx, errors, warnings);
        }
        Format::Github => {
            // 環境変数を読むのはここだけ（パス解決関数は引数で受けてテスト可能にする）
            let workspace = std::env::var_os("GITHUB_WORKSPACE").map(PathBuf::from);
            let base = annotation_base(ctx.root, ctx.content_dir, workspace.as_deref());
            let root_base = annotation_root(ctx.root, workspace.as_deref());
            for line in github_lines(&diags, &base, &root_base) {
                outln!("{line}");
            }
            // 注釈以外の行は Actions のパーサに無視される。ジョブログで全体の件数を
            // 見る唯一の手段になるので human と同じ集計行を残す
            print_summary(&diags, ctx, errors, warnings);
        }
        Format::Json => outln!(
            "{}",
            render_json(
                &diags,
                prefix,
                Path::new(""),
                JsonSummary {
                    errors,
                    warnings,
                    pages: ctx.pages,
                    suppressed: ctx.suppressed,
                    disabled: ctx.disabled,
                    skipped: ctx.skipped,
                },
            )?
        ),
    }

    // 終了コードの判定に format を混ぜない（規約 0 / 1 / 2 は形式に依らない）
    Ok(if diags.is_empty() {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    })
}

/// 「問題ありません」「エラー N 件・警告 M 件」の集計行（human / github 共通）。
/// 抑制（`lintDisable`）・全体無効化（`lint.rules`）・スキップ（外部リンク検査）は
/// 効いたときだけ件数を付記する（すべて 0 件なら従来とバイト同一）
fn print_summary(diags: &[Diagnostic], ctx: &Context, errors: usize, warnings: usize) {
    let pages = ctx.pages;
    let mut notes = Vec::new();
    if ctx.suppressed > 0 {
        notes.push(format!("抑制 {} 件", ctx.suppressed));
    }
    if ctx.disabled > 0 {
        notes.push(format!("無効化 {} 件", ctx.disabled));
    }
    if ctx.skipped > 0 {
        notes.push(format!("スキップ {} 件", ctx.skipped));
    }
    let note = notes.join("・");
    if diags.is_empty() {
        if note.is_empty() {
            outln!("問題ありません（{pages} ページ）");
        } else {
            outln!("問題ありません（{pages} ページ・{note}）");
        }
    } else if note.is_empty() {
        outln!("エラー {errors} 件・警告 {warnings} 件");
    } else {
        outln!("エラー {errors} 件・警告 {warnings} 件（{note}）");
    }
}

/// (エラー数, 警告数)
fn counts(diags: &[Diagnostic]) -> (usize, usize) {
    let errors = diags
        .iter()
        .filter(|d| d.severity == Severity::Error)
        .count();
    (errors, diags.len() - errors)
}

/// ファイル → 行 → 列の順に安定ソートする（ルール間の順序を揃える）
fn sort_diagnostics(diags: &mut [Diagnostic]) {
    diags.sort_by(|a, b| {
        (
            &a.rel,
            a.span.map_or((0, 0), |s| (s.start_line, s.start_col)),
        )
            .cmp(&(
                &b.rel,
                b.span.map_or((0, 0), |s| (s.start_line, s.start_col)),
            ))
    });
}

/// 診断の `rel` に前置する基点を選ぶ。`yuzu.toml` のような content 外の
/// ファイルは `..` を使わずプロジェクトルート基点で組み立てる
fn base_prefix<'a>(d: &Diagnostic, content: &'a Path, root: &'a Path) -> &'a Path {
    match d.base {
        DiagBase::Content => content,
        DiagBase::ProjectRoot => root,
    }
}

/// github 形式でのプロジェクトルートの基点（[`annotation_base`] の content なし版）
fn annotation_root(root: &Path, workspace: Option<&Path>) -> PathBuf {
    if let Some(ws) = workspace {
        if let Ok(rel) = root.strip_prefix(ws) {
            return rel.to_path_buf();
        }
        if let (Ok(ws), Ok(r)) = (ws.canonicalize(), root.canonicalize()) {
            if let Ok(rel) = r.strip_prefix(&ws) {
                return rel.to_path_buf();
            }
        }
    }
    PathBuf::new()
}

fn severity_str(severity: Severity) -> &'static str {
    match severity {
        Severity::Error => "error",
        Severity::Warning => "warning",
    }
}

/// `content/guide/x.md:12:1: warning[rule] メッセージ` 形式。
/// ファイル単位の診断（span なし）は位置を省く
fn human_lines(diags: &[Diagnostic], prefix: &Path, root_prefix: &Path) -> Vec<String> {
    diags
        .iter()
        .map(|d| {
            let path = base_prefix(d, prefix, root_prefix).join(&d.rel);
            let severity = severity_str(d.severity);
            match d.span {
                Some(span) => format!(
                    "{}:{}:{}: {severity}[{}] {}",
                    path.display(),
                    span.start_line,
                    span.start_col,
                    d.rule,
                    d.message
                ),
                None => format!("{}: {severity}[{}] {}", path.display(), d.rule, d.message),
            }
        })
        .collect()
}

/// GitHub Actions のワークフローコマンド。
/// `::error file=docs/content/x.md,line=12,col=1,title=yuzu[broken-link]::メッセージ`
///
/// ルール ID は `title=` に入れる（注釈 UI の見出しになる。省くと "error" とだけ表示される）。
/// `endLine` / `endColumn` は出さない — yuzu の列は comrak 由来の**バイト基準**で
/// GitHub は文字基準のため、日本語行では終端がずれて範囲ハイライトが崩れる
/// （行の紐づけは正しいので注釈の実用性には影響しない）
fn github_lines(diags: &[Diagnostic], base: &Path, root_base: &Path) -> Vec<String> {
    diags
        .iter()
        .map(|d| {
            let kind = severity_str(d.severity);
            let file = escape_property(&slash(&base_prefix(d, base, root_base).join(&d.rel)));
            let title = escape_property(&format!("yuzu[{}]", d.rule));
            let pos = match d.span {
                Some(span) => format!(",line={},col={}", span.start_line, span.start_col),
                None => String::new(),
            };
            format!(
                "::{kind} file={file}{pos},title={title}::{}",
                escape_data(&d.message)
            )
        })
        .collect()
}

/// GitHub Actions の注釈は**リポジトリルート相対**のパスでないと PR の diff 行に
/// 紐づかない。yuzu の診断はプロジェクトルート相対なので、workspace
/// （`GITHUB_WORKSPACE`）が分かるときはそこからの相対へ付け替える。
/// 分からない・配下でないときはプロジェクトルート相対へフォールバックする
/// （ローカル実行では human と同じパスになる）
fn annotation_base(root: &Path, content_dir: &Path, workspace: Option<&Path>) -> PathBuf {
    if let Some(ws) = workspace {
        if let Ok(rel) = content_dir.strip_prefix(ws) {
            return rel.to_path_buf();
        }
        // シンボリックリンク経由で綴りが違う場合の保険（macOS ランナー等）
        if let (Ok(ws), Ok(dir)) = (ws.canonicalize(), content_dir.canonicalize()) {
            if let Ok(rel) = dir.strip_prefix(&ws) {
                return rel.to_path_buf();
            }
        }
    }
    content_dir
        .strip_prefix(root)
        .unwrap_or(content_dir)
        .to_path_buf()
}

/// ワークフローコマンドのメッセージ本体のエスケープ。
/// **`%` を最初に置換する**（後続の置換が生む `%` を二重変換しないため）。
/// `broken-link` は URL を生で埋め込むので `%` は実際に出現する
fn escape_data(s: &str) -> String {
    s.replace('%', "%25")
        .replace('\r', "%0D")
        .replace('\n', "%0A")
}

/// プロパティ値（`file=` / `title=`）のエスケープ。区切り文字も潰す
fn escape_property(s: &str) -> String {
    escape_data(s).replace(':', "%3A").replace(',', "%2C")
}

/// json / github へ出すパスは常に `/` 区切りにする（Windows 対策）。
/// human は従来どおりプラットフォーム表記のまま
fn slash(path: &Path) -> String {
    path.components()
        .map(|c| c.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

/// JSON 出力の 1 診断。CI が消費する公開契約なので、内部の
/// [`yuzu_core::Diagnostic`] とは意図的に別の型にする（内部型の変更で契約が
/// 黙って壊れるのを防ぐ）。**キーの削除・改名はしない（追加のみ）**
#[derive(Serialize)]
struct JsonDiagnostic<'a> {
    /// ルール ID（`docs/content/reference/rules.md` の一覧に対応）
    rule: &'a str,
    /// `error` / `warning`
    severity: &'a str,
    /// プロジェクトルート相対・`/` 区切り（例 `content/guide/x.md`）
    path: String,
    /// 1 始まりの行。ファイル単位の診断（`fmt` 等）は null
    line: Option<usize>,
    /// 1 始まりの列（comrak 由来のバイト基準）。同上
    column: Option<usize>,
    message: &'a str,
    /// `yuzu lint --fix` で自動修正できるか
    fixable: bool,
}

#[derive(Serialize)]
struct JsonSummary {
    errors: usize,
    warnings: usize,
    /// 検査したページ数
    pages: usize,
    /// frontmatter `lintDisable` で抑制した件数（キー追加のみ = 契約準拠）
    suppressed: usize,
    /// `lint.rules` の全体無効化で落とした件数（キー追加のみ = 契約準拠）
    disabled: usize,
    /// `check --external-links` で検査できなかった外部 URL の数（キー追加のみ = 契約準拠。
    /// 環境依存の失敗を診断に載せないための逃がし先）
    skipped: usize,
}

#[derive(Serialize)]
struct JsonReport<'a> {
    diagnostics: Vec<JsonDiagnostic<'a>>,
    summary: JsonSummary,
}

fn render_json(
    diags: &[Diagnostic],
    prefix: &Path,
    root_prefix: &Path,
    summary: JsonSummary,
) -> anyhow::Result<String> {
    let report = JsonReport {
        diagnostics: diags
            .iter()
            .map(|d| JsonDiagnostic {
                rule: d.rule,
                severity: severity_str(d.severity),
                path: slash(&base_prefix(d, prefix, root_prefix).join(&d.rel)),
                line: d.span.map(|s| s.start_line),
                column: d.span.map(|s| s.start_col),
                message: &d.message,
                fixable: d.fix.is_some(),
            })
            .collect(),
        summary,
    };
    Ok(serde_json::to_string_pretty(&report)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use yuzu_core::SourceSpan;

    /// config-* ルールのレジストリ照合。yuzu-config は依存グラフの葉で
    /// `yuzu_core::rules` を参照できないため、両方に依存するここで
    /// 双方向＋濃度を縛る（speccheck の SPEC_LANGS テストと同型）
    #[test]
    fn config_ルールはレジストリと双方向に一致する() {
        use yuzu_core::rules;
        for id in yuzu_config::CONFIG_RULES {
            let rule = rules::find(id).unwrap_or_else(|| panic!("{id} がレジストリに無い"));
            // 変換（config_diagnostics）は severity を Warning 固定にしている
            assert_eq!(rule.severity, yuzu_core::Severity::Warning, "{id}");
            assert!(!rule.suppressible, "{id} はページ外なので抑制不可のはず");
        }
        let in_registry = rules::RULES
            .iter()
            .filter(|r| r.id.starts_with("config-"))
            .count();
        assert_eq!(in_registry, yuzu_config::CONFIG_RULES.len());
    }

    /// 全カウンタ 0 の集計。個別の値は `JsonSummary { errors: 1, ..summary(3) }` の
    /// 形で名前付きに上書きする（位置引数の取り違えをなくす）
    fn summary(pages: usize) -> JsonSummary {
        JsonSummary {
            errors: 0,
            warnings: 0,
            pages,
            suppressed: 0,
            disabled: 0,
            skipped: 0,
        }
    }

    fn span(line: usize, col: usize) -> SourceSpan {
        SourceSpan {
            start_line: line,
            start_col: col,
            end_line: line,
            end_col: col + 1,
        }
    }

    fn diag(
        rule: &'static str,
        severity: Severity,
        rel: &str,
        span: Option<SourceSpan>,
        fix: Option<&str>,
    ) -> Diagnostic {
        Diagnostic {
            rule,
            severity,
            base: DiagBase::Content,
            rel: PathBuf::from(rel),
            span,
            message: "メッセージ".to_string(),
            fix: fix.map(str::to_string),
        }
    }

    #[test]
    fn human_形式は従来どおりの1行を出す() {
        let diags = vec![diag(
            "duplicate-h1",
            Severity::Warning,
            "guide/x.md",
            Some(span(12, 1)),
            None,
        )];
        let lines = human_lines(&diags, Path::new("content"), Path::new(""));
        assert_eq!(
            lines[0],
            "content/guide/x.md:12:1: warning[duplicate-h1] メッセージ"
        );
    }

    #[test]
    fn span_なしの診断は位置を省いて出す() {
        let diags = vec![diag("fmt", Severity::Error, "x.md", None, None)];
        let lines = human_lines(&diags, Path::new("content"), Path::new(""));
        assert_eq!(lines[0], "content/x.md: error[fmt] メッセージ");
    }

    #[test]
    fn github_形式は深刻度で_error_と_warning_を出し分ける() {
        let diags = vec![
            diag(
                "broken-link",
                Severity::Error,
                "a.md",
                Some(span(3, 5)),
                None,
            ),
            diag(
                "duplicate-h1",
                Severity::Warning,
                "b.md",
                Some(span(1, 1)),
                None,
            ),
        ];
        let lines = github_lines(&diags, Path::new("docs/content"), Path::new(""));
        assert!(lines[0].starts_with("::error file=docs/content/a.md,line=3,col=5,"));
        assert!(
            lines[0].contains("title=yuzu%5Bbroken-link%5D")
                || lines[0].contains("title=yuzu[broken-link]")
        );
        assert!(lines[1].starts_with("::warning file=docs/content/b.md,line=1,col=1,"));
    }

    #[test]
    fn github_形式はファイル単位の診断に位置を付けない() {
        let diags = vec![diag("fmt", Severity::Error, "x.md", None, None)];
        let lines = github_lines(&diags, Path::new("content"), Path::new(""));
        assert!(lines[0].starts_with("::error file=content/x.md,title="));
        assert!(!lines[0].contains("line="));
    }

    #[test]
    fn github_形式のメッセージはパーセントと改行をエスケープする() {
        // broken-link は URL を生で埋め込むため percent エンコードが実際に混入する
        let mut d = diag(
            "broken-link",
            Severity::Error,
            "a.md",
            Some(span(1, 1)),
            None,
        );
        d.message = "リンク先 `%E8%A6%8B.md` が\n見つかりません".to_string();
        let lines = github_lines(&[d], Path::new("content"), Path::new(""));
        assert!(lines[0].ends_with("::リンク先 `%25E8%25A6%258B.md` が%0A見つかりません"));
    }

    #[test]
    fn github_形式のパスは_workspace_からの相対になる() {
        let base = annotation_base(
            Path::new("/w/yuzu/docs"),
            Path::new("/w/yuzu/docs/content"),
            Some(Path::new("/w/yuzu")),
        );
        assert_eq!(base, PathBuf::from("docs/content"));
    }

    #[test]
    fn github_形式は_workspace_が無ければプロジェクトルート相対へ落ちる() {
        let base = annotation_base(
            Path::new("/w/yuzu/docs"),
            Path::new("/w/yuzu/docs/content"),
            None,
        );
        assert_eq!(base, PathBuf::from("content"));
    }

    #[test]
    fn github_形式は_workspace_の外なら付け替えない() {
        let base = annotation_base(
            Path::new("/other/proj"),
            Path::new("/other/proj/content"),
            Some(Path::new("/w/yuzu")),
        );
        assert_eq!(base, PathBuf::from("content"));
    }

    #[test]
    fn json_形式は診断と集計を1つのオブジェクトに入れる() {
        let diags = vec![diag(
            "broken-link",
            Severity::Error,
            "guide/x.md",
            Some(span(12, 1)),
            None,
        )];
        let (errors, warnings) = counts(&diags);
        let out = render_json(
            &diags,
            Path::new("content"),
            Path::new(""),
            JsonSummary {
                errors,
                warnings,
                ..summary(16)
            },
        )
        .unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert!(v.is_object());
        assert_eq!(v["diagnostics"][0]["rule"], "broken-link");
        assert_eq!(v["diagnostics"][0]["severity"], "error");
        assert_eq!(v["diagnostics"][0]["path"], "content/guide/x.md");
        assert_eq!(v["diagnostics"][0]["line"], 12);
        assert_eq!(v["summary"]["errors"], 1);
        assert_eq!(v["summary"]["warnings"], 0);
        assert_eq!(v["summary"]["pages"], 16);
        assert_eq!(v["summary"]["suppressed"], 0, "抑制ゼロでもキーは必ず出す");
        assert_eq!(v["summary"]["disabled"], 0, "無効化ゼロでもキーは必ず出す");
        assert_eq!(v["summary"]["skipped"], 0, "スキップゼロでもキーは必ず出す");
    }

    #[test]
    fn json_の_summary_に_skipped_が入る() {
        let out = render_json(
            &[],
            Path::new("content"),
            Path::new(""),
            JsonSummary {
                skipped: 4,
                ..summary(3)
            },
        )
        .unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["summary"]["skipped"], 4);
        assert_eq!(v["summary"]["warnings"], 0);
    }

    #[test]
    fn json_の_summary_に_suppressed_が入る() {
        let out = render_json(
            &[],
            Path::new("content"),
            Path::new(""),
            JsonSummary {
                suppressed: 2,
                ..summary(3)
            },
        )
        .unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["summary"]["suppressed"], 2);
        assert_eq!(v["summary"]["errors"], 0);
    }

    #[test]
    fn json_の_summary_に_disabled_が入る() {
        let out = render_json(
            &[],
            Path::new("content"),
            Path::new(""),
            JsonSummary {
                disabled: 5,
                ..summary(3)
            },
        )
        .unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["summary"]["disabled"], 5);
        assert_eq!(v["summary"]["suppressed"], 0);
    }

    /// lint.rules で無効化できる ID の一覧はレジストリの suppressible 集合と一致する
    /// （yuzu-config は葉でレジストリを参照できないため、両方に依存するここで縛る。
    /// CONFIG_RULES のテストと同型）
    #[test]
    fn 無効化できるルール一覧はレジストリの抑制可能集合と双方向に一致する() {
        use yuzu_core::rules;
        for id in yuzu_config::DISABLEABLE_RULES {
            let rule = rules::find(id).unwrap_or_else(|| panic!("{id} がレジストリに無い"));
            assert!(
                rule.suppressible,
                "{id} は抑制不可なのに lint.rules で無効化可になっている"
            );
        }
        let suppressible = rules::RULES.iter().filter(|r| r.suppressible).count();
        assert_eq!(
            suppressible,
            yuzu_config::DISABLEABLE_RULES.len(),
            "濃度一致 = 双方向"
        );
    }

    #[test]
    fn json_形式はファイル単位の診断の行と列を_null_にする() {
        let diags = vec![diag("fmt", Severity::Error, "x.md", None, None)];
        let out = render_json(
            &diags,
            Path::new("content"),
            Path::new(""),
            JsonSummary {
                errors: 1,
                ..summary(1)
            },
        )
        .unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert!(v["diagnostics"][0]["line"].is_null());
        assert!(v["diagnostics"][0]["column"].is_null());
    }

    #[test]
    fn json_形式は自動修正できる診断を_fixable_にする() {
        let diags = vec![
            diag(
                "term-variant",
                Severity::Warning,
                "a.md",
                Some(span(1, 1)),
                Some("サーバ"),
            ),
            diag(
                "duplicate-h1",
                Severity::Warning,
                "b.md",
                Some(span(1, 1)),
                None,
            ),
        ];
        let out = render_json(
            &diags,
            Path::new("content"),
            Path::new(""),
            JsonSummary {
                warnings: 2,
                ..summary(2)
            },
        )
        .unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["diagnostics"][0]["fixable"], true);
        assert_eq!(v["diagnostics"][1]["fixable"], false);
    }

    #[test]
    fn 診断はファイル_行_列の順に並ぶ() {
        let mut diags = vec![
            diag("a", Severity::Warning, "b.md", Some(span(1, 1)), None),
            diag("b", Severity::Warning, "a.md", Some(span(9, 1)), None),
            diag("c", Severity::Warning, "a.md", Some(span(2, 3)), None),
        ];
        sort_diagnostics(&mut diags);
        let rels: Vec<_> = diags
            .iter()
            .map(|d| (d.rel.display().to_string(), d.span.unwrap().start_line))
            .collect();
        assert_eq!(
            rels,
            vec![
                ("a.md".to_string(), 2),
                ("a.md".to_string(), 9),
                ("b.md".to_string(), 1)
            ]
        );
    }

    #[test]
    fn 集計はエラーと警告を数える() {
        let diags = vec![
            diag("a", Severity::Error, "a.md", None, None),
            diag("b", Severity::Warning, "b.md", None, None),
            diag("c", Severity::Warning, "c.md", None, None),
        ];
        assert_eq!(counts(&diags), (1, 2));
    }
}
