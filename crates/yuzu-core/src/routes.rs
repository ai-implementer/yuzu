//! ページ route（出力 URL）の一意性と妥当性の検証。
//!
//! - `route-conflict` — `foo.md` と `foo/index.md` はどちらも route `foo/` になり、
//!   同じ `foo/index.html` を出力する。レンダリングはページ並列（rayon）なので
//!   検出しないと**勝者が実行ごとに変わる**（出力マニフェストは `BTreeSet` で
//!   同じ rel を 1 件に潰すため痕跡も残らない）
//! - `unsafe-page-path` — yuzu は slug 化をせずファイル名をそのまま route にする。
//!   URL で意味を持つ文字（`#` `?` `%` 空白・非 ASCII 等）は route → URL の変換点
//!   （`urlpath::encode_path`）がパーセントエンコードするので許容し、**出力パスとして
//!   書けない文字**（`\` と制御文字）だけを拒否する。設定・frontmatter 由来の route
//!   （合成ページ・エイリアス）は加えて Windows の予約文字も全 OS で拒否する
//!
//! [`crate::validate_aliases`] と対になる「出力 URL」の担当で、
//! check（診断一覧）と render（書き出し前の中断）の両方から呼ぶ。

use std::collections::HashMap;

use crate::diagnostics::{DiagBase, Diagnostic};
use crate::model::Page;
use crate::rules;

/// 実ファイル名の route にできない文字。
///
/// `\` は `output::resolve_output_rel` が出力 rel として拒否する（Windows の
/// 区切り。URL にすると `/` と混同される）。制御文字（改行・タブ等）は
/// [`unsafe_path_chars`] で別途弾く。
///
/// `#` `?` `%` 引用符・山括弧・空白・非 ASCII は**ここでは弾かない**。
/// 表示・書き出しの直前に `urlpath::encode_path` が `%XX` にするので、
/// `a#b.md` の URL は `/a%23b/`、`a%23b.md` は `/a%2523b/` になり、
/// サーバがデコードすると物理パスと一致する。実ファイル名は執筆者の OS が
/// 作れる範囲に既に収まっている（Windows なら `?` 等のファイルはそもそも無い）
const UNSAFE_PATH_CHARS: &[char] = &['\\'];

/// 設定・frontmatter 由来の route（合成ページ・エイリアス）にできない文字。
///
/// 実ファイル名と違い、設定値と alias は**どの OS でビルドしても**同じ出力パスを
/// 作らせるので、Windows がファイル名に使えない `< > : " | ? *`（と `\`）は
/// 全プラットフォームで拒否する。Linux で通った `search.page = "a?b"` が Windows の
/// dist 書き出し途中で I/O エラーになる事態を、事前診断に変える
pub(crate) const PORTABLE_UNSAFE_PATH_CHARS: &[char] = &['\\', '<', '>', ':', '"', '|', '?', '*'];

/// `rel`（`/` 区切り）に含まれる出力パスとして使えない文字（出現順・重複なし）。
/// `portable` なら Windows の予約文字も対象にする（合成ページ・エイリアス用）
pub(crate) fn unsafe_path_chars(rel: &str, portable: bool) -> Vec<char> {
    let set = if portable {
        PORTABLE_UNSAFE_PATH_CHARS
    } else {
        UNSAFE_PATH_CHARS
    };
    let mut found: Vec<char> = Vec::new();
    for c in rel.chars() {
        if (set.contains(&c) || c.is_control()) && !found.contains(&c) {
            found.push(c);
        }
    }
    found
}

/// 診断文面用に文字列化する（制御文字はコードポイントで可視化する）
pub(crate) fn describe_chars(chars: &[char]) -> String {
    chars
        .iter()
        .map(|c| {
            if c.is_control() {
                format!("U+{:04X}", *c as u32)
            } else {
                format!("`{c}`")
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// 全ページの route（出力 URL）の一意性と妥当性を検証する。
///
/// いずれも `Severity::Error`（check は失敗・build は書き出し前に中断）。
/// ファイル配置そのものの問題なので位置情報は持たない
/// （`directory-too-deep` と同じ扱いで `span` は None）。
pub fn validate_routes(pages: &[Page]) -> Vec<Diagnostic> {
    let mut diags = Vec::new();
    // 先着（走査はパス順で安定するので、どちらを衝突側として報告するかは決定的）
    let mut claimed: HashMap<&str, &Page> = HashMap::new();

    for page in pages {
        check_page_path(page, &mut diags);
        match claimed.get(page.route.as_str()) {
            // 合成ページ（設定から生成）が絡む衝突は、直す場所も報告先も違う。
            // 実在しないファイルを rel にすると `--format github` の注釈が
            // 付かないので、報告は可能な限り**実ページ側の rel** で行う
            Some(first) if page.is_generated() || first.is_generated() => {
                let (rel, message) = match (first.generated, page.generated) {
                    // 両方が合成（`markdown.glossary.page` と `search.page` が同値）。
                    // 実ページが無いので合成側の rel で報告する（github 注釈は
                    // 付かないが、直す場所はどちらも設定なので許容）
                    (Some(a), Some(b)) => (
                        page.rel.clone(),
                        format!(
                            "自動生成される{}と{}の URL /{} が衝突しています。`{}` か `{}` を変えてください",
                            a.label(),
                            b.label(),
                            page.route,
                            a.config_key(),
                            b.config_key()
                        ),
                    ),
                    _ => {
                        let (kind, real): (_, &Page) = match page.generated {
                            Some(kind) => (kind, first),
                            None => (first.generated.expect("片方は必ず合成"), page),
                        };
                        (
                            real.rel.clone(),
                            format!(
                                "自動生成される{}の URL /{} がページ {} と衝突しています。`{}` かページのファイル名を変えてください",
                                kind.label(),
                                page.route,
                                real.rel.display(),
                                kind.config_key()
                            ),
                        )
                    }
                };
                diags.push(Diagnostic {
                    rule: rules::ROUTE_CONFLICT.id,
                    severity: rules::ROUTE_CONFLICT.severity,
                    base: DiagBase::Content,
                    rel,
                    span: None,
                    message,
                    fix: None,
                });
            }
            Some(first) => diags.push(Diagnostic {
                rule: rules::ROUTE_CONFLICT.id,
                severity: rules::ROUTE_CONFLICT.severity,
                base: DiagBase::Content,
                rel: page.rel.clone(),
                span: None,
                message: format!(
                    "URL /{} がページ {} と衝突しています（`x.md` と `x/index.md` は同じ URL になります）。どちらかのファイル名を変えてください",
                    page.route,
                    first.rel.display()
                ),
                fix: None,
            }),
            None => {
                claimed.insert(page.route.as_str(), page);
            }
        }
    }
    diags
}

/// ファイル名（合成ページは設定値）に出力パスとして書けない文字が入っていないか。
///
/// **警告ではなくエラーにする**。`\` を含む rel は `output::write_under` が
/// 書き出しを拒否するため、通すと render の途中で I/O エラーになる。
/// 制御文字は URL・HTML・ログのどれでも見えない切れ目になる。
/// 書き出す前に止めるのが唯一の整合策。合成ページは設定由来なので
/// Windows の予約文字も拒否する（[`PORTABLE_UNSAFE_PATH_CHARS`]）
fn check_page_path(page: &Page, out: &mut Vec<Diagnostic>) {
    let rel = crate::urlpath::rel_to_slash(&page.rel);
    let found = unsafe_path_chars(&rel, page.is_generated());
    if found.is_empty() {
        return;
    }
    let list = describe_chars(&found);
    out.push(Diagnostic {
        rule: rules::UNSAFE_PAGE_PATH.id,
        severity: rules::UNSAFE_PAGE_PATH.severity,
        base: DiagBase::Content,
        rel: page.rel.clone(),
        span: None,
        message: match page.generated {
            // 合成ページは設定から作られるので、直す場所はファイル名ではない
            Some(kind) => format!(
                "`{}` に出力パスとして使えない文字（{list}）が含まれています。yuzu は値をそのまま出力先のパスにするため、{}を書き出せません（Windows で使えない `< > : \" | ? *` はどの OS でも拒否します）",
                kind.config_key(),
                kind.label()
            ),
            None => format!(
                "ファイル名に出力パスとして使えない文字（{list}）が含まれています。yuzu はファイル名をそのまま出力先のパスにするため、このページを書き出せません。ファイル名を変えてください"
            ),
        },
        fix: None,
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Frontmatter, GeneratedKind};
    use std::path::PathBuf;

    fn page(rel: &str, route: &str) -> Page {
        Page {
            src: PathBuf::from("/content").join(rel),
            rel: PathBuf::from(rel),
            route: route.to_string(),
            frontmatter: Frontmatter::default(),
            title: "t".to_string(),
            toc: Vec::new(),
            labels: Vec::new(),
            crossref_offset: Default::default(),
            generated: None,
            source: "# t\n".to_string(),
        }
    }

    fn generated_page(rel: &str, route: &str, kind: GeneratedKind) -> Page {
        Page {
            generated: Some(kind),
            ..page(rel, route)
        }
    }

    #[test]
    fn 衝突がなければ診断ゼロ() {
        let pages = [
            page("index.md", ""),
            page("guide/index.md", "guide/"),
            page("guide/start.md", "guide/start/"),
        ];
        assert!(validate_routes(&pages).is_empty());
    }

    #[test]
    fn 同じ_url_になるページを検出する() {
        // 走査順（sort_by_file_name）では guide/index.md が先に来る
        let pages = [page("guide/index.md", "guide/"), page("guide.md", "guide/")];
        let diags = validate_routes(&pages);

        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].rule, "route-conflict");
        assert_eq!(diags[0].severity, crate::Severity::Error);
        assert_eq!(
            diags[0].rel,
            PathBuf::from("guide.md"),
            "報告は後着のページ"
        );
        assert!(
            diags[0].message.contains("guide/index.md"),
            "先着のページも文面に出す: {}",
            diags[0].message
        );
    }

    #[test]
    fn 診断は位置情報を持たない() {
        let pages = [page("a/index.md", "a/"), page("a.md", "a/")];
        assert!(validate_routes(&pages)[0].span.is_none());
    }

    #[test]
    fn 三つ以上の衝突は先着以外をすべて報告する() {
        let pages = [
            page("a/index.md", "a/"),
            page("a.md", "a/"),
            page("b.md", "a/"),
        ];
        assert_eq!(
            validate_routes(&pages)
                .iter()
                .filter(|d| d.rule == "route-conflict")
                .count(),
            2
        );
    }

    #[test]
    fn 合成ページとの衝突は実ページ側の_rel_と設定キーで報告する() {
        // 合成ページ（用語集・検索結果）は実在しないファイルを rel に持つため、
        // `--format github` の注釈が付くよう報告は実ページ側で行う
        for (kind, key) in [
            (GeneratedKind::Glossary, "markdown.glossary.page"),
            (GeneratedKind::Search, "search.page"),
        ] {
            for pages in [
                // 合成が先着でも後着でも同じ報告になる
                [
                    generated_page("glossary.md", "glossary/", kind),
                    page("glossary.md", "glossary/"),
                ],
                [
                    page("glossary.md", "glossary/"),
                    generated_page("glossary.md", "glossary/", kind),
                ],
            ] {
                let diags = validate_routes(&pages);
                assert_eq!(diags.len(), 1, "{kind:?}: {diags:?}");
                assert_eq!(diags[0].rule, "route-conflict");
                assert_eq!(diags[0].rel, PathBuf::from("glossary.md"));
                assert!(
                    diags[0].message.contains(&format!("`{key}`")),
                    "{kind:?}: {}",
                    diags[0].message
                );
                assert!(
                    diags[0].message.contains(kind.label()),
                    "{kind:?}: {}",
                    diags[0].message
                );
            }
        }
    }

    #[test]
    fn 両方が合成ページの衝突は両方の設定キーを案内する() {
        // `markdown.glossary.page` と `search.page` に同じ値を設定した場合。
        // 実ページが無いので rel は合成側になる（直す場所はどちらも設定）
        let pages = [
            generated_page("search.md", "search/", GeneratedKind::Glossary),
            generated_page("search.md", "search/", GeneratedKind::Search),
        ];
        let diags = validate_routes(&pages);
        assert_eq!(diags.len(), 1, "{diags:?}");
        assert_eq!(diags[0].rule, "route-conflict");
        assert!(
            diags[0].message.contains("`markdown.glossary.page`")
                && diags[0].message.contains("`search.page`"),
            "{}",
            diags[0].message
        );
    }

    #[test]
    fn 合成ページの_unsafe_page_path_は設定キーを案内する() {
        for (kind, key) in [
            (GeneratedKind::Glossary, "markdown.glossary.page"),
            (GeneratedKind::Search, "search.page"),
        ] {
            let diags = validate_routes(&[generated_page("a\\b.md", "a\\b/", kind)]);
            let hits: Vec<_> = diags
                .iter()
                .filter(|d| d.rule == "unsafe-page-path")
                .collect();
            assert_eq!(hits.len(), 1, "{kind:?}: {diags:?}");
            assert!(
                hits[0].message.contains(&format!("`{key}`")),
                "{kind:?}: {}",
                hits[0].message
            );
        }
    }

    #[test]
    fn 出力パスとして使えない文字を含むファイル名はエラーになる() {
        // `\` は output::write_under が拒否する（通すと書き出し途中で I/O エラー）。
        // 制御文字は URL・HTML・ログのどれでも見えない切れ目になる
        for rel in ["a\\b.md", "guide/x\ty.md", "a\nb.md"] {
            let route = rel.trim_end_matches(".md").to_string() + "/";
            let diags = validate_routes(&[page(rel, &route)]);
            let hits: Vec<_> = diags
                .iter()
                .filter(|d| d.rule == "unsafe-page-path")
                .collect();
            assert_eq!(hits.len(), 1, "{rel:?}: {diags:?}");
            assert_eq!(hits[0].severity, crate::Severity::Error);
            assert!(hits[0].span.is_none(), "ファイル配置の問題なので span なし");
            assert!(
                hits[0].message.contains("出力パス"),
                "{rel:?}: {}",
                hits[0].message
            );
        }
    }

    #[test]
    fn 合成ページは_windows_で使えない文字もどの_os_でも拒否する() {
        // 設定値はどの OS でビルドしても同じ出力パスを作るので、Linux で通って
        // Windows の書き出し途中で I/O エラーになる形を事前診断にする
        for (rel, route) in [
            ("a?b.md", "a?b/"),
            ("a<b>.md", "a<b>/"),
            ("a\"b.md", "a\"b/"),
            ("a|b.md", "a|b/"),
            ("a*b.md", "a*b/"),
            ("a:b.md", "a:b/"),
        ] {
            let diags = validate_routes(&[generated_page(rel, route, GeneratedKind::Search)]);
            assert_eq!(diags.len(), 1, "{rel:?}: {diags:?}");
            assert_eq!(diags[0].rule, "unsafe-page-path");
            assert!(
                diags[0].message.contains("Windows"),
                "{rel:?}: {}",
                diags[0].message
            );
            // 同じ文字でも実ファイル名は OS が作れた時点で正当（URL 化でエンコードされる）
            assert!(
                validate_routes(&[page(rel, route)]).is_empty(),
                "{rel:?} は実ファイルなら許容"
            );
        }
        // `#` `%` 空白・非 ASCII は合成ページでも許容
        for (rel, route) in [
            ("a#b.md", "a#b/"),
            ("a%23b.md", "a%23b/"),
            ("設 計.md", "設 計/"),
        ] {
            assert!(
                validate_routes(&[generated_page(rel, route, GeneratedKind::Glossary)]).is_empty(),
                "{rel:?}"
            );
        }
    }

    #[test]
    fn 制御文字はコードポイントで報告する() {
        let diags = validate_routes(&[page("a\tb.md", "a\tb/")]);
        assert!(
            diags[0].message.contains("U+0009"),
            "見えない文字を可視化する: {}",
            diags[0].message
        );
    }

    #[test]
    fn 通常のファイル名は報告しない() {
        // 日本語・ハイフン・アンダースコア・半角スペースは対象外。
        // `#` `?` `%` 引用符・山括弧も、URL 化で `%XX` にエンコードされるので許容
        // （`a%23b.md` は `/a%2523b/` になり、サーバのデコード後に物理パスと一致）
        for (rel, route) in [
            ("index.md", ""),
            ("guide/getting-started.md", "guide/getting-started/"),
            ("設計/概要.md", "設計/概要/"),
            ("a b.md", "a b/"),
            ("a#b.md", "a#b/"),
            ("a?b.md", "a?b/"),
            ("a%23b.md", "a%23b/"),
            (r#"a"b'c.md"#, r#"a"b'c/"#),
            ("guide/x<y>.md", "guide/x<y>/"),
        ] {
            let diags = validate_routes(&[page(rel, route)]);
            assert!(diags.is_empty(), "{rel:?}: {diags:?}");
        }
    }
}
