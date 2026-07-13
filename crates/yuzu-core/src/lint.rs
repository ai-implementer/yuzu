//! 文書規約の lint ルール（`yuzu lint` / `yuzu check`）。
//!
//! ルール:
//! - `duplicate-h1` — 本文 h1 が 2 個以上（テーマはページタイトルを h1 相当で表示する）
//! - `heading-level-skip` — 隣接見出し間でレベルが 2 以上深くなる（markdownlint MD001 相当）
//! - `frontmatter-unknown-key` — 既知キー以外のトップレベルキー（typo 検出）
//! - `directory-too-deep` — content 配下のディレクトリ階層が深すぎる
//!   （`lint.maxDirectoryDepth` 設定時のみ）
//! - `term-variant` — 用語ゆれ（`lint.terms` の辞書設定時のみ。正表記への統一を促す）

use crate::diagnostics::{Diagnostic, Severity};
use crate::error::CoreError;
use crate::frontmatter::{KNOWN_KEYS, yaml_body};
use crate::markdown;
use crate::model::{Page, SourceSpan, TocEntry};
use crate::{LintOptions, MarkdownOptions};

pub(crate) fn lint_page(
    page: &Page,
    opts: &MarkdownOptions,
    lint: &LintOptions,
) -> Result<Vec<Diagnostic>, CoreError> {
    let mut out = Vec::new();
    check_headings(&page.toc, page, &mut out);
    check_frontmatter_keys(page, opts, &mut out);
    if let Some(max_depth) = lint.max_directory_depth {
        check_directory_depth(page, max_depth, &mut out);
    }
    if !lint.terms.is_empty() {
        check_terms(page, opts, &lint.terms, &mut out);
    }
    out.sort_by_key(|d| d.span.map_or((0, 0), |s| (s.start_line, s.start_col)));
    Ok(out)
}

/// term-variant: 用語ゆれの検出（辞書ベース）。
/// 本文のテキストノード（コード・URL は対象外）にゆれ表記が現れたら報告する。
/// 正表記の一部としての出現（例: 正「サーバー」の中の「サーバ」）は誤検出なので除外する
fn check_terms(
    page: &Page,
    opts: &MarkdownOptions,
    terms: &std::collections::BTreeMap<String, Vec<String>>,
    out: &mut Vec<Diagnostic>,
) {
    let texts = markdown::extract_text_spans(&page.source, opts);
    for (text, span) in &texts {
        for (canonical, variants) in terms {
            // 正表記の出現区間（バイト範囲）。この中の variant マッチは除外する
            let canon_ranges: Vec<(usize, usize)> = text
                .match_indices(canonical.as_str())
                .map(|(i, m)| (i, i + m.len()))
                .collect();
            for variant in variants {
                if variant.is_empty() || variant == canonical {
                    continue;
                }
                for (i, m) in text.match_indices(variant.as_str()) {
                    let end = i + m.len();
                    if canon_ranges.iter().any(|&(s, e)| s <= i && end <= e) {
                        continue;
                    }
                    out.push(Diagnostic {
                        rule: "term-variant",
                        severity: Severity::Warning,
                        rel: page.rel.clone(),
                        // comrak の列はバイト基準なので、ノード内バイトオフセットを足す
                        span: Some(SourceSpan {
                            start_line: span.start_line,
                            start_col: span.start_col + i,
                            end_line: span.start_line,
                            end_col: span.start_col + end,
                        }),
                        message: format!(
                            "「{variant}」は「{canonical}」に統一してください（lint.terms）"
                        ),
                    });
                }
            }
        }
    }
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

/// ディレクトリ階層の深さ検査（ファイル配置の問題なので span なし）。
/// 深さは content 直下 = 0 で数える（`guide/x.md` は 1）
fn check_directory_depth(page: &Page, max_depth: u32, out: &mut Vec<Diagnostic>) {
    let depth = page.rel.components().count().saturating_sub(1);
    if depth > max_depth as usize {
        out.push(Diagnostic {
            rule: "directory-too-deep",
            severity: Severity::Warning,
            rel: page.rel.clone(),
            span: None,
            message: format!(
                "content から {depth} 階層のディレクトリに置かれています（許容は {max_depth} 階層まで）"
            ),
        });
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

    use crate::{LintOptions, MarkdownOptions, Page, build_source_pages};

    /// content 相対パス `rel` にページを置いて構築する（親ディレクトリは自動作成）
    fn page_from_rel(rel: &str, source: &str) -> Page {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, source).unwrap();
        build_source_pages(dir.path(), &[], &MarkdownOptions::default())
            .unwrap()
            .into_iter()
            .next()
            .unwrap()
    }

    fn page_from(source: &str) -> Page {
        page_from_rel("index.md", source)
    }

    fn lint(source: &str) -> Vec<crate::Diagnostic> {
        super::lint_page(
            &page_from(source),
            &MarkdownOptions::default(),
            &LintOptions::default(),
        )
        .unwrap()
    }

    fn lint_depth(rel: &str, max_directory_depth: Option<u32>) -> Vec<crate::Diagnostic> {
        let opts = LintOptions {
            max_directory_depth,
            ..LintOptions::default()
        };
        super::lint_page(
            &page_from_rel(rel, "# t\n"),
            &MarkdownOptions::default(),
            &opts,
        )
        .unwrap()
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
        assert!(
            skips[0].message.contains("h3 を挟んで"),
            "{}",
            skips[0].message
        );
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
        assert!(
            unknown[0].message.contains("`tags`"),
            "{}",
            unknown[0].message
        );
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

    #[test]
    fn 階層制限が未設定なら深いパスも診断なし() {
        let diags = lint_depth("a/b/c/deep.md", None);
        assert!(diags.is_empty(), "{diags:?}");
    }

    #[test]
    fn 制限_1_で_1_階層のディレクトリは許容() {
        assert!(lint_depth("index.md", Some(1)).is_empty(), "content 直下");
        assert!(lint_depth("guide/x.md", Some(1)).is_empty(), "1 階層");
    }

    fn lint_terms(source: &str, terms: &[(&str, &[&str])]) -> Vec<crate::Diagnostic> {
        let opts = LintOptions {
            terms: terms
                .iter()
                .map(|(c, vs)| (c.to_string(), vs.iter().map(|v| v.to_string()).collect()))
                .collect(),
            ..LintOptions::default()
        };
        super::lint_page(&page_from(source), &MarkdownOptions::default(), &opts).unwrap()
    }

    #[test]
    fn 用語ゆれを行番号付きで検出する() {
        let diags = lint_terms(
            "# t\n\n本文でサーバを再起動する。\n",
            &[("サーバー", &["サーバ"])],
        );
        let hits: Vec<_> = diags.iter().filter(|d| d.rule == "term-variant").collect();
        assert_eq!(hits.len(), 1, "{diags:?}");
        assert_eq!(hits[0].span.unwrap().start_line, 3);
        assert!(
            hits[0].message.contains("「サーバ」は「サーバー」に統一"),
            "{}",
            hits[0].message
        );
    }

    #[test]
    fn 正表記の一部としての出現は誤検出しない() {
        // 「サーバー」の中に「サーバ」が含まれるが、正表記そのものなので報告しない
        let diags = lint_terms(
            "# t\n\nサーバーを再起動する。\n",
            &[("サーバー", &["サーバ"])],
        );
        assert!(diags.iter().all(|d| d.rule != "term-variant"), "{diags:?}");

        // 同一テキスト内に正とゆれが混在 → ゆれの方だけ報告
        let diags = lint_terms(
            "# t\n\nサーバーとサーバが混在。\n",
            &[("サーバー", &["サーバ"])],
        );
        let hits: Vec<_> = diags.iter().filter(|d| d.rule == "term-variant").collect();
        assert_eq!(hits.len(), 1, "{diags:?}");
    }

    #[test]
    fn コードブロックとインラインコードは対象外() {
        let diags = lint_terms(
            "# t\n\n```\nサーバ再起動\n```\n\n`サーバ` の説明。\n",
            &[("サーバー", &["サーバ"])],
        );
        assert!(diags.iter().all(|d| d.rule != "term-variant"), "{diags:?}");
    }

    #[test]
    fn 見出しの用語ゆれも検出する() {
        let diags = lint_terms("# サーバの設定\n", &[("サーバー", &["サーバ"])]);
        let hits: Vec<_> = diags.iter().filter(|d| d.rule == "term-variant").collect();
        assert_eq!(hits.len(), 1, "{diags:?}");
        assert_eq!(hits[0].span.unwrap().start_line, 1);
    }

    #[test]
    fn 複数のゆれと複数エントリを検出する() {
        let diags = lint_terms(
            "# t\n\nユーザがサーバへ接続。ユーザーは正しい。\n",
            &[("サーバー", &["サーバ"]), ("ユーザー", &["ユーザ"])],
        );
        let mut rules: Vec<&str> = diags
            .iter()
            .filter(|d| d.rule == "term-variant")
            .map(|d| d.message.as_str())
            .collect();
        rules.sort();
        assert_eq!(rules.len(), 2, "{diags:?}");
        assert!(rules[0].contains("サーバ"));
        assert!(rules[1].contains("ユーザ"));
    }

    #[test]
    fn 辞書が空なら何もしない() {
        let diags = lint_terms("# t\n\nサーバ。\n", &[]);
        assert!(diags.iter().all(|d| d.rule != "term-variant"));
    }

    #[test]
    fn 制限_1_で_2_階層のディレクトリは警告() {
        let diags = lint_depth("guide/sub/deep.md", Some(1));
        let deep: Vec<_> = diags
            .iter()
            .filter(|d| d.rule == "directory-too-deep")
            .collect();
        assert_eq!(deep.len(), 1, "{diags:?}");
        assert!(deep[0].span.is_none(), "ファイル単位の診断なので span なし");
        assert!(
            deep[0].message.contains("2 階層") && deep[0].message.contains("1 階層まで"),
            "{}",
            deep[0].message
        );
    }
}
