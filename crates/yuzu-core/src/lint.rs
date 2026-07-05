//! 文書規約の lint ルール（`yuzu lint` / `yuzu check`）。
//!
//! 初期ルール:
//! - `duplicate-h1` — 本文 h1 が 2 個以上（テーマはページタイトルを h1 相当で表示する）
//! - `heading-level-skip` — 隣接見出し間でレベルが 2 以上深くなる（markdownlint MD001 相当）
//! - `frontmatter-unknown-key` — 既知キー以外のトップレベルキー（typo 検出）

use crate::diagnostics::{Diagnostic, Severity};
use crate::error::CoreError;
use crate::frontmatter::{KNOWN_KEYS, yaml_body};
use crate::markdown;
use crate::model::{Page, SourceSpan, TocEntry};
use crate::MarkdownOptions;

pub(crate) fn lint_page(page: &Page, opts: &MarkdownOptions) -> Result<Vec<Diagnostic>, CoreError> {
    let mut out = Vec::new();
    check_headings(&page.toc, page, &mut out);
    check_frontmatter_keys(page, opts, &mut out);
    out.sort_by_key(|d| d.span.map_or((0, 0), |s| (s.start_line, s.start_col)));
    Ok(out)
}

/// 見出し規約（toc だけで判定する。span は toc 由来）
fn check_headings(toc: &[TocEntry], page: &Page, out: &mut Vec<Diagnostic>) {
    // duplicate-h1: 2 個目以降の h1 を報告
    let mut h1s = toc.iter().filter(|t| t.level == 1);
    if let Some(first_h1) = h1s.next() {
        for dup in h1s {
            out.push(Diagnostic {
                rule: "duplicate-h1",
                severity: Severity::Warning,
                rel: page.rel.clone(),
                span: Some(dup.span),
                message: format!(
                    "本文の h1 が複数あります（最初の h1 は {} 行目「{}」）",
                    first_h1.span.start_line, first_h1.text
                ),
            });
        }
    }

    // heading-level-skip: 隣接見出し間のみ判定（h2 → h4 など）
    for pair in toc.windows(2) {
        let (prev, next) = (&pair[0], &pair[1]);
        if next.level > prev.level + 1 {
            out.push(Diagnostic {
                rule: "heading-level-skip",
                severity: Severity::Warning,
                rel: page.rel.clone(),
                span: Some(next.span),
                message: format!(
                    "h{} の直後に h{} が来ています（h{} を挟んでください）",
                    prev.level,
                    next.level,
                    prev.level + 1
                ),
            });
        }
    }
}

/// frontmatter の未知キー検出。
/// パースは build 時に済んでいる（不正 YAML はここへ来る前に CoreError）ため、
/// ここでの再パース失敗は黙って無視する
fn check_frontmatter_keys(page: &Page, opts: &MarkdownOptions, out: &mut Vec<Diagnostic>) {
    let Some((raw, fm_span)) = markdown::frontmatter_raw(&page.source, opts) else {
        return;
    };
    let Ok(value) = serde_yaml_ng::from_str::<serde_yaml_ng::Value>(yaml_body(&raw)) else {
        return;
    };
    let Some(mapping) = value.as_mapping() else {
        return;
    };

    for key in mapping.keys().filter_map(|k| k.as_str()) {
        if KNOWN_KEYS.contains(&key) {
            continue;
        }
        out.push(Diagnostic {
            rule: "frontmatter-unknown-key",
            severity: Severity::Warning,
            rel: page.rel.clone(),
            span: Some(key_span(&raw, &fm_span, key)),
            message: format!(
                "frontmatter に未知のキー `{key}` があります（対応キー: {}）",
                KNOWN_KEYS.join("/")
            ),
        });
    }
}

/// キーの行番号を raw 内の前方一致で探す（見つからなければ frontmatter 全体）。
/// raw は `---` 区切り行込みなので、行オフセットは fm_span.start_line 起点
fn key_span(raw: &str, fm_span: &SourceSpan, key: &str) -> SourceSpan {
    for (idx, line) in raw.lines().enumerate() {
        if line.trim_start().starts_with(&format!("{key}:")) {
            let line_no = fm_span.start_line + idx;
            return SourceSpan {
                start_line: line_no,
                start_col: 1,
                end_line: line_no,
                end_col: line.chars().count().max(1),
            };
        }
    }
    *fm_span
}

#[cfg(test)]
mod tests {
    use std::fs;

    use crate::{MarkdownOptions, Page, build_source_pages};

    fn page_from(source: &str) -> Page {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("index.md"), source).unwrap();
        build_source_pages(dir.path(), &[], &MarkdownOptions::default())
            .unwrap()
            .into_iter()
            .next()
            .unwrap()
    }

    fn lint(source: &str) -> Vec<crate::Diagnostic> {
        super::lint_page(&page_from(source), &MarkdownOptions::default()).unwrap()
    }

    #[test]
    fn h1_が重複すると警告() {
        let diags = lint("# 一つ目\n\n本文\n\n# 二つ目\n\n# 三つ目\n");
        let dups: Vec<_> = diags.iter().filter(|d| d.rule == "duplicate-h1").collect();
        assert_eq!(dups.len(), 2, "2 個目以降を報告: {diags:?}");
        assert_eq!(dups[0].span.unwrap().start_line, 5);
        assert!(dups[0].message.contains("1 行目"), "{}", dups[0].message);
    }

    #[test]
    fn 見出しレベルの飛びを検出する() {
        let diags = lint("# t\n\n## h2\n\n#### h4\n");
        let skips: Vec<_> = diags
            .iter()
            .filter(|d| d.rule == "heading-level-skip")
            .collect();
        assert_eq!(skips.len(), 1, "{diags:?}");
        assert_eq!(skips[0].span.unwrap().start_line, 5);
        assert!(skips[0].message.contains("h3 を挟んで"), "{}", skips[0].message);
    }

    #[test]
    fn h2_h3_h2_は許容() {
        assert!(lint("# t\n\n## a\n\n### b\n\n## c\n").is_empty());
        // 浅くなる方向の飛び（h4 → h2）も許容（MD001 と同じ）
        assert!(lint("## a\n\n### b\n\n#### c\n\n## d\n").is_empty());
    }

    #[test]
    fn frontmatter_の未知キーを行番号付きで報告() {
        let diags = lint("---\ntitle: x\ntags: [a, b]\n---\n\n# t\n");
        let unknown: Vec<_> = diags
            .iter()
            .filter(|d| d.rule == "frontmatter-unknown-key")
            .collect();
        assert_eq!(unknown.len(), 1, "{diags:?}");
        assert!(unknown[0].message.contains("`tags`"), "{}", unknown[0].message);
        assert_eq!(unknown[0].span.unwrap().start_line, 3, "tags: の行");
    }

    #[test]
    fn 既知キーのみなら診断なし() {
        let diags = lint(
            "---\ntitle: x\norder: 1\ndraft: false\ndescription: 説明\nllms: true\n---\n\n# t\n",
        );
        assert!(diags.is_empty(), "{diags:?}");
    }

    #[test]
    fn frontmatter_なしでも動く() {
        assert!(lint("# t\n\n本文\n").is_empty());
    }
}
