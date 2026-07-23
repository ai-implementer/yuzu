//! frontmatter `aliases`（旧 URL → リダイレクト）の正規化と検証。
//!
//! エイリアスは route 形式（`guide/old-name/`）へ正規化して扱う。
//! 正規化の実装はここに 1 つだけ置き、render（リダイレクト HTML の出力先）と
//! check（衝突検査）で解釈を揃える。

use std::collections::HashMap;
use std::collections::HashSet;

use crate::MarkdownOptions;
use crate::diagnostics::{Diagnostic, Severity};
use crate::markdown;
use crate::model::{Page, SourceSpan};

/// エイリアス文字列を route 形式（`old/path/`。ルートは `""`）へ正規化する。
/// 先頭 `/` と末尾スラッシュの省略は吸収し、サイト外・不正なパスは Err で返す
pub(crate) fn normalize_alias(raw: &str) -> Result<String, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("空のエイリアスは指定できません".to_string());
    }
    if trimmed.contains("://") {
        return Err(format!(
            "エイリアスはサイト内パスで指定してください（URL は不可）: {trimmed}"
        ));
    }
    if trimmed.contains('#') || trimmed.contains('?') {
        return Err(format!(
            "エイリアスにフラグメント・クエリは使えません: {trimmed}"
        ));
    }
    if trimmed.contains('\\') {
        return Err(format!(
            "エイリアスの区切りは / を使ってください: {trimmed}"
        ));
    }
    let core = trimmed.trim_start_matches('/').trim_end_matches('/');
    if core.is_empty() {
        return Err("サイトルートへのエイリアスは指定できません".to_string());
    }
    if core.split('/').any(|seg| seg.is_empty() || seg == "..") {
        return Err(format!("エイリアスのパスが不正です: {trimmed}"));
    }
    Ok(format!("{core}/"))
}

/// ページの正規化済みエイリアス route 一覧（ページ内の重複は除去）。
/// 不正な値はスキップする（エラー報告は [`validate_aliases`] の責務。
/// ビルド経路では validate が先にエラーで止めるため、ここに不正値は来ない）
pub fn alias_routes(page: &Page) -> Vec<String> {
    let mut seen = HashSet::new();
    page.frontmatter
        .aliases
        .iter()
        .filter_map(|raw| normalize_alias(raw).ok())
        .filter(|route| seen.insert(route.clone()))
        .collect()
}

/// 全ページのエイリアスを検証する。
///
/// - `alias-invalid`: 正規化できない値（空・URL・`..` 等）
/// - `alias-conflict`: 実ページ route との衝突（自ページ含む）、
///   および他エイリアスとの重複（ページ内・ページ間）
///
/// いずれも Severity::Error（ビルドすると実ページ・他リダイレクトを
/// 上書きしてしまうため、check では失敗・build では中断にする）。
/// 診断には frontmatter の該当行の span を付ける（`opts` はそのパース用）
pub fn validate_aliases(pages: &[Page], opts: &MarkdownOptions) -> Vec<Diagnostic> {
    let mut diags = Vec::new();
    let routes: HashMap<&str, &Page> = pages.iter().map(|p| (p.route.as_str(), p)).collect();
    // 正規化済みエイリアス → 最初に宣言したページ（ページ間重複の検出用）
    let mut claimed: HashMap<String, &Page> = HashMap::new();

    for page in pages {
        let fm = markdown::frontmatter_raw(&page.source, opts);
        for raw in &page.frontmatter.aliases {
            let span = fm
                .as_ref()
                .map(|(text, fm_span)| alias_span(text, fm_span, raw));
            let route = match normalize_alias(raw) {
                Ok(route) => route,
                Err(message) => {
                    diags.push(diag(page, span, "alias-invalid", message));
                    continue;
                }
            };
            if let Some(hit) = routes.get(route.as_str()) {
                let target = if hit.rel == page.rel {
                    "このページ自身".to_string()
                } else {
                    format!("ページ {} ", hit.rel.display())
                };
                diags.push(diag(
                    page,
                    span,
                    "alias-conflict",
                    format!("エイリアス /{route} は{target}の URL と衝突しています"),
                ));
                continue;
            }
            match claimed.get(route.as_str()) {
                Some(first) if first.rel == page.rel => {
                    diags.push(diag(
                        page,
                        span,
                        "alias-conflict",
                        format!("エイリアス /{route} が重複しています"),
                    ));
                }
                Some(first) => {
                    diags.push(diag(
                        page,
                        span,
                        "alias-conflict",
                        format!(
                            "エイリアス /{route} はページ {} のエイリアスと重複しています",
                            first.rel.display()
                        ),
                    ));
                }
                None => {
                    claimed.insert(route, page);
                }
            }
        }
    }
    diags
}

/// エイリアスの生文字列が書かれた frontmatter 行の span を探す。
/// 値の行 → `aliases:` キー行 → frontmatter 全体、の順でフォールバック
fn alias_span(raw_fm: &str, fm_span: &SourceSpan, alias_raw: &str) -> SourceSpan {
    let needle = alias_raw.trim();
    if !needle.is_empty() {
        for (idx, line) in raw_fm.lines().enumerate() {
            if line.contains(needle) {
                return line_span(fm_span, idx, line);
            }
        }
    }
    for (idx, line) in raw_fm.lines().enumerate() {
        if line.trim_start().starts_with("aliases:") {
            return line_span(fm_span, idx, line);
        }
    }
    *fm_span
}

/// frontmatter 内の行インデックスを文書全体の 1 行 span へ変換する
/// （raw は `---` 区切り行込みなので、行オフセットは fm_span.start_line 起点）
fn line_span(fm_span: &SourceSpan, idx: usize, line: &str) -> SourceSpan {
    let line_no = fm_span.start_line + idx;
    SourceSpan {
        start_line: line_no,
        start_col: 1,
        end_line: line_no,
        end_col: line.chars().count().max(1),
    }
}

fn diag(page: &Page, span: Option<SourceSpan>, rule: &'static str, message: String) -> Diagnostic {
    Diagnostic {
        rule,
        severity: Severity::Error,
        rel: page.rel.clone(),
        span,
        message,
        fix: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Frontmatter;
    use std::path::PathBuf;

    fn page(rel: &str, route: &str, aliases: &[&str]) -> Page {
        // 実 frontmatter 込みの source を持たせる（span 特定の検証用。
        // aliases 行は必ず 3 行目になる）
        let source = if aliases.is_empty() {
            "---\ntitle: t\n---\n\n# t\n".to_string()
        } else {
            let list = aliases
                .iter()
                .map(|a| format!("\"{a}\""))
                .collect::<Vec<_>>()
                .join(", ");
            format!("---\ntitle: t\naliases: [{list}]\n---\n\n# t\n")
        };
        Page {
            src: PathBuf::from(format!("/content/{rel}")),
            rel: PathBuf::from(rel),
            route: route.to_string(),
            frontmatter: Frontmatter {
                aliases: aliases.iter().map(|a| a.to_string()).collect(),
                ..Frontmatter::default()
            },
            title: "t".to_string(),
            toc: Vec::new(),
            source,
        }
    }

    fn validate(pages: &[Page]) -> Vec<Diagnostic> {
        validate_aliases(pages, &MarkdownOptions::default())
    }

    #[test]
    fn 正規化は先頭スラッシュと末尾省略を吸収する() {
        assert_eq!(normalize_alias("guide/old/"), Ok("guide/old/".to_string()));
        assert_eq!(normalize_alias("/guide/old"), Ok("guide/old/".to_string()));
        assert_eq!(normalize_alias(" old "), Ok("old/".to_string()));
    }

    #[test]
    fn 不正なエイリアスは拒否する() {
        for bad in [
            "",
            "  ",
            "/",
            "https://example.com/x",
            "a#frag",
            "a?q=1",
            "a//b",
            "../up",
            "a/../b",
            "a\\b",
        ] {
            assert!(normalize_alias(bad).is_err(), "拒否されるべき: {bad:?}");
        }
    }

    #[test]
    fn alias_routes_は正規化して重複を除く() {
        let p = page("a.md", "a/", &["old/", "/old", "other"]);
        assert_eq!(
            alias_routes(&p),
            vec!["old/".to_string(), "other/".to_string()]
        );
    }

    #[test]
    fn 衝突なしなら診断ゼロ() {
        let pages = vec![
            page("a.md", "a/", &["old-a/"]),
            page("b.md", "b/", &["old-b/"]),
        ];
        assert!(validate(&pages).is_empty());
    }

    #[test]
    fn 実ページ_route_との衝突を検出する() {
        // 他ページと自ページの両方
        let pages = vec![page("a.md", "a/", &["b/", "a/"]), page("b.md", "b/", &[])];
        let diags = validate(&pages);
        assert_eq!(diags.len(), 2);
        assert!(diags.iter().all(|d| d.rule == "alias-conflict"));
        assert!(diags[0].message.contains("b.md"), "{}", diags[0].message);
        assert!(
            diags[1].message.contains("このページ自身"),
            "{}",
            diags[1].message
        );
        // frontmatter の aliases 行（3 行目）を指す
        assert_eq!(diags[0].span.unwrap().start_line, 3);
        assert_eq!(diags[1].span.unwrap().start_line, 3);
    }

    #[test]
    fn エイリアス同士の衝突を検出する() {
        // ページ内重複（表記ゆれ込み）とページ間重複
        let pages = vec![
            page("a.md", "a/", &["old/", "/old"]),
            page("b.md", "b/", &["old"]),
        ];
        let diags = validate(&pages);
        assert_eq!(diags.len(), 2);
        assert!(diags[0].message.contains("重複"), "{}", diags[0].message);
        assert!(diags[1].message.contains("a.md"), "{}", diags[1].message);
    }

    #[test]
    fn 不正な形式は_alias_invalid() {
        let pages = vec![page("a.md", "a/", &["https://example.com/"])];
        let diags = validate(&pages);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].rule, "alias-invalid");
        assert_eq!(diags[0].severity, Severity::Error);
    }
}
