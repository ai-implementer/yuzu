//! 文書規約の lint ルール（`yuzu lint` / `yuzu check`）。
//!
//! ルール:
//! - `duplicate-h1` — 本文 h1 が 2 個以上（テーマはページタイトルを h1 相当で表示する）
//! - `heading-level-skip` — 隣接見出し間でレベルが 2 以上深くなる（markdownlint MD001 相当）
//! - `frontmatter-unknown-key` — 既知キー以外のトップレベルキー（typo 検出）
//! - `directory-too-deep` — content 配下のディレクトリ階層が深すぎる
//!   （`lint.maxDirectoryDepth` 設定時のみ）
//! - `term-variant` — 用語ゆれ（`lint.terms` の辞書設定時のみ。正表記への統一を促す）
//! - `fullwidth-alphanumeric` — 全角英数字（組み込み。既定有効）
//! - `halfwidth-kana` — 半角カナ（組み込み。既定有効）
//! - `katakana-choon` — 長音符ゆれの混在（組み込み・プロジェクト横断。既定有効）

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
    if lint.rules.fullwidth_alphanumeric || lint.rules.halfwidth_kana {
        check_char_classes(page, opts, &lint.rules, &mut out);
    }
    out.sort_by_key(|d| d.span.map_or((0, 0), |s| (s.start_line, s.start_col)));
    Ok(out)
}

/// プロジェクト横断ルール（ページ間の整合）。lint_page の後に呼ぶ
pub(crate) fn lint_project(
    pages: &[Page],
    opts: &MarkdownOptions,
    lint: &LintOptions,
) -> Result<Vec<Diagnostic>, CoreError> {
    let mut out = Vec::new();
    if lint.rules.katakana_choon {
        check_katakana_choon(pages, opts, &mut out);
    }
    out.sort_by(|a, b| {
        let key = |d: &Diagnostic| {
            (
                d.rel.clone(),
                d.span.map_or((0, 0), |s| (s.start_line, s.start_col)),
            )
        };
        key(a).cmp(&key(b))
    });
    Ok(out)
}

/// fullwidth-alphanumeric / halfwidth-kana: 文字クラスの連続 run を 1 診断にまとめる
fn check_char_classes(
    page: &Page,
    opts: &MarkdownOptions,
    rules: &crate::LintRules,
    out: &mut Vec<Diagnostic>,
) {
    let is_fullwidth_alnum = |c: char| matches!(c, '\u{FF10}'..='\u{FF19}' | '\u{FF21}'..='\u{FF3A}' | '\u{FF41}'..='\u{FF5A}');
    let is_halfwidth_kana = |c: char| matches!(c, '\u{FF61}'..='\u{FF9F}');

    for (text, span) in &markdown::extract_text_spans(&page.source, opts) {
        if rules.fullwidth_alphanumeric {
            for (offset, run) in char_class_runs(text, is_fullwidth_alnum) {
                // 全角英数字 → 半角はコード点 −0xFEE0 の単純対応
                let suggestion: String = run
                    .chars()
                    .map(|c| char::from_u32(c as u32 - 0xFEE0).unwrap_or(c))
                    .collect();
                out.push(Diagnostic {
                    rule: "fullwidth-alphanumeric",
                    severity: Severity::Warning,
                    rel: page.rel.clone(),
                    span: Some(run_span(span, offset, run.len())),
                    message: format!("全角英数字「{run}」は半角「{suggestion}」を推奨します"),
                    fix: Some(suggestion),
                });
            }
        }
        if rules.halfwidth_kana {
            for (offset, run) in char_class_runs(text, is_halfwidth_kana) {
                let suggestion = to_fullwidth_kana(run);
                out.push(Diagnostic {
                    rule: "halfwidth-kana",
                    severity: Severity::Warning,
                    rel: page.rel.clone(),
                    span: Some(run_span(span, offset, run.len())),
                    message: format!("半角カナ「{run}」は全角「{suggestion}」を推奨します"),
                    fix: Some(suggestion),
                });
            }
        }
    }
}

/// 述語を満たす文字の最長連続 run を (バイトオフセット, 部分文字列) で列挙する
fn char_class_runs(text: &str, pred: impl Fn(char) -> bool) -> Vec<(usize, &str)> {
    let mut runs = Vec::new();
    let mut start: Option<usize> = None;
    for (i, c) in text.char_indices() {
        if pred(c) {
            start.get_or_insert(i);
        } else if let Some(s) = start.take() {
            runs.push((s, &text[s..i]));
        }
    }
    if let Some(s) = start {
        runs.push((s, &text[s..]));
    }
    runs
}

/// テキストノード内の run 位置を SourceSpan へ（列は comrak と同じバイト基準）
fn run_span(node: &SourceSpan, offset: usize, len: usize) -> SourceSpan {
    SourceSpan {
        start_line: node.start_line,
        start_col: node.start_col + offset,
        end_line: node.start_line,
        end_col: node.start_col + offset + len,
    }
}

/// 半角カナ（U+FF61〜FF9F）→ 全角の変換候補。濁点/半濁点は可能なら合成する
fn to_fullwidth_kana(run: &str) -> String {
    // U+FF61 起点の並び順どおりの対応表
    const TABLE: &[char] = &[
        '。', '「', '」', '、', '・', 'ヲ', 'ァ', 'ィ', 'ゥ', 'ェ', 'ォ', 'ャ', 'ュ', 'ョ', 'ッ',
        'ー', 'ア', 'イ', 'ウ', 'エ', 'オ', 'カ', 'キ', 'ク', 'ケ', 'コ', 'サ', 'シ', 'ス', 'セ',
        'ソ', 'タ', 'チ', 'ツ', 'テ', 'ト', 'ナ', 'ニ', 'ヌ', 'ネ', 'ノ', 'ハ', 'ヒ', 'フ', 'ヘ',
        'ホ', 'マ', 'ミ', 'ム', 'メ', 'モ', 'ヤ', 'ユ', 'ヨ', 'ラ', 'リ', 'ル', 'レ', 'ロ', 'ワ',
        'ン', '゛', '゜',
    ];
    let mut out = String::new();
    for c in run.chars() {
        let mapped = TABLE
            .get((c as usize).wrapping_sub(0xFF61))
            .copied()
            .unwrap_or(c);
        match mapped {
            // 濁点: カ〜ト・ハ〜ホは +1 で濁音、ウはヴ
            '゛' => match out.pop() {
                Some(prev @ ('カ'..='ト' | 'ハ'..='ホ')) => {
                    out.push(char::from_u32(prev as u32 + 1).expect("カタカナ濁音"));
                }
                Some('ウ') => out.push('ヴ'),
                Some(prev) => {
                    out.push(prev);
                    out.push('゛');
                }
                None => out.push('゛'),
            },
            // 半濁点: ハ行は +2 で半濁音
            '゜' => match out.pop() {
                Some(prev @ 'ハ'..='ホ') => {
                    out.push(char::from_u32(prev as u32 + 2).expect("カタカナ半濁音"));
                }
                Some(prev) => {
                    out.push(prev);
                    out.push('゜');
                }
                None => out.push('゜'),
            },
            m => out.push(m),
        }
    }
    out
}

/// katakana-choon: 長音符ゆれの混在検出（プロジェクト横断・多数決）。
/// ー を全除去した正規化キーでカタカナ語をグループ化し、複数の表記が
/// 混在していたら**少数派の出現箇所**に警告する（同数なら両方に警告）。
/// 最短の表記が 3 文字未満のグループは対象外
/// （「カド/カード」のような別語の偶然一致を除外。「サーバ/サーバー」は対象）
fn check_katakana_choon(pages: &[Page], opts: &MarkdownOptions, out: &mut Vec<Diagnostic>) {
    use std::collections::BTreeMap;

    // 正規化キー → 表記 → 出現位置（BTreeMap で決定的な走査順）
    type Occurrences = BTreeMap<String, Vec<(std::path::PathBuf, SourceSpan)>>;
    let mut groups: BTreeMap<String, Occurrences> = BTreeMap::new();

    let is_katakana = |c: char| matches!(c, '\u{30A1}'..='\u{30FA}' | 'ー');
    for page in pages {
        for (text, span) in &markdown::extract_text_spans(&page.source, opts) {
            for (offset, word) in char_class_runs(text, is_katakana) {
                // ー のみ・実質 1 文字の run は語ではないので除外
                let key: String = word.chars().filter(|&c| c != 'ー').collect();
                if key.chars().count() < 2 {
                    continue;
                }
                groups
                    .entry(key)
                    .or_default()
                    .entry(word.to_string())
                    .or_default()
                    .push((page.rel.clone(), run_span(span, offset, word.len())));
            }
        }
    }

    for variants in groups.values() {
        if variants.len() < 2 {
            continue; // 表記が 1 種類なら混在なし
        }
        // 最短の表記が 3 文字未満なら別語の偶然一致とみなして除外（カド/カード等）
        let min_surface = variants
            .keys()
            .map(|w| w.chars().count())
            .min()
            .unwrap_or(0);
        if min_surface < 3 {
            continue;
        }
        let max_count = variants.values().map(Vec::len).max().unwrap_or(0);
        // 単独多数派の表記（同数タイなら None = 正解が決められないので自動修正なし）
        let majority: Option<&String> = {
            let mut top = variants.iter().filter(|(_, o)| o.len() == max_count);
            match (top.next(), top.next()) {
                (Some((w, _)), None) => Some(w),
                _ => None,
            }
        };
        let summary = variants
            .iter()
            .map(|(w, occs)| format!("{w}: {} 回", occs.len()))
            .collect::<Vec<_>>()
            .join(" / ");
        for (word, occs) in variants {
            if majority == Some(word) {
                continue; // 単独多数派は正とみなして報告しない
            }
            for (rel, span) in occs {
                out.push(Diagnostic {
                    rule: "katakana-choon",
                    severity: Severity::Warning,
                    rel: rel.clone(),
                    span: Some(*span),
                    message: format!(
                        "長音符ゆれ「{word}」が混在しています（{summary}。多数派への統一を推奨）"
                    ),
                    fix: majority.cloned(),
                });
            }
        }
    }
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
                        fix: Some(canonical.clone()),
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
                fix: None,
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
                fix: None,
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
            fix: None,
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
            fix: None,
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

/// `fix` を持つ診断をソースへ適用する（`yuzu lint --fix` 用）。
/// span の列は comrak と同じ 1 始まり・バイト基準（fix 対象ルールの end_col は排他）。
/// 位置の後ろから適用して前方のオフセットを保ち、範囲が交差する fix は
/// 先勝ちでスキップする（スキップ分は次の再 lint で再検出される）。
/// 戻り値は (適用後ソース, 適用件数)
pub(crate) fn apply_fixes(source: &str, diags: &[Diagnostic]) -> (String, usize) {
    // 行頭のバイトオフセット（1 始まり行番号 − 1 で引く）
    let mut line_starts = vec![0usize];
    line_starts.extend(
        source
            .bytes()
            .enumerate()
            .filter(|&(_, b)| b == b'\n')
            .map(|(i, _)| i + 1),
    );

    // (開始バイト, 終了バイト, 置換文字列) へ変換。範囲外・非文字境界は防御的に捨てる
    let mut edits: Vec<(usize, usize, &str)> = Vec::new();
    for d in diags {
        let (Some(span), Some(fix)) = (d.span, d.fix.as_deref()) else {
            continue;
        };
        if span.start_col == 0 || span.end_col == 0 {
            continue;
        }
        let Some(&line_s) = line_starts.get(span.start_line.wrapping_sub(1)) else {
            continue;
        };
        let Some(&line_e) = line_starts.get(span.end_line.wrapping_sub(1)) else {
            continue;
        };
        let start = line_s + span.start_col - 1;
        let end = line_e + span.end_col - 1;
        if start >= end
            || end > source.len()
            || !source.is_char_boundary(start)
            || !source.is_char_boundary(end)
        {
            continue;
        }
        edits.push((start, end, fix));
    }

    edits.sort_by_key(|&(start, end, _)| std::cmp::Reverse((start, end)));
    let mut out = source.to_string();
    let mut applied = 0usize;
    let mut last_start = usize::MAX;
    for (start, end, fix) in edits {
        if end > last_start {
            continue; // 適用済み範囲と交差（同一範囲の重複含む）
        }
        out.replace_range(start..end, fix);
        last_start = start;
        applied += 1;
    }
    (out, applied)
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
    fn 全角英数字を変換候補付きで検出する() {
        let diags = lint("# t\n\nＷｅｂ１２３ を使う。\n");
        let hits: Vec<_> = diags
            .iter()
            .filter(|d| d.rule == "fullwidth-alphanumeric")
            .collect();
        assert_eq!(hits.len(), 1, "連続 run は 1 診断: {diags:?}");
        assert!(
            hits[0].message.contains("「Ｗｅｂ１２３」は半角「Web123」"),
            "{}",
            hits[0].message
        );
        assert_eq!(hits[0].span.unwrap().start_line, 3);
    }

    #[test]
    fn 半角カナを濁点合成込みで検出する() {
        let diags = lint("# t\n\nﾃﾞｰﾀﾍﾞｰｽ移行。\n");
        let hits: Vec<_> = diags
            .iter()
            .filter(|d| d.rule == "halfwidth-kana")
            .collect();
        assert_eq!(hits.len(), 1, "{diags:?}");
        assert!(
            hits[0].message.contains("全角「データベース」"),
            "濁点が合成される: {}",
            hits[0].message
        );
    }

    #[test]
    fn 半角カナの半濁点も合成する() {
        let diags = lint("# t\n\nﾊﾟｽ。\n");
        let hit = diags.iter().find(|d| d.rule == "halfwidth-kana").unwrap();
        assert!(hit.message.contains("全角「パス」"), "{}", hit.message);
    }

    #[test]
    fn コードブロック内の全角半角は対象外() {
        let diags = lint("# t\n\n```\nＷｅｂ ﾃﾞｰﾀ\n```\n\n`Ｘ１` の説明。\n");
        assert!(
            diags
                .iter()
                .all(|d| d.rule != "fullwidth-alphanumeric" && d.rule != "halfwidth-kana"),
            "{diags:?}"
        );
    }

    #[test]
    fn 組み込みルールは無効化できる() {
        let opts = LintOptions {
            rules: crate::LintRules {
                fullwidth_alphanumeric: false,
                halfwidth_kana: false,
                katakana_choon: false,
            },
            ..LintOptions::default()
        };
        let diags = super::lint_page(
            &page_from("# t\n\nＷｅｂ と ﾃﾞｰﾀ。\n"),
            &MarkdownOptions::default(),
            &opts,
        )
        .unwrap();
        assert!(diags.is_empty(), "{diags:?}");
    }

    /// 複数ページからプロジェクトを組んで lint_project を回す
    fn lint_project_of(pages_src: &[(&str, &str)]) -> Vec<crate::Diagnostic> {
        let dir = tempfile::tempdir().unwrap();
        for (rel, source) in pages_src {
            let path = dir.path().join(rel);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(&path, source).unwrap();
        }
        let pages = build_source_pages(dir.path(), &[], &MarkdownOptions::default()).unwrap();
        super::lint_project(&pages, &MarkdownOptions::default(), &LintOptions::default()).unwrap()
    }

    #[test]
    fn 長音ゆれの混在は少数派に警告する() {
        let diags = lint_project_of(&[
            ("a.md", "# A\n\nサーバーを起動。サーバーを停止。\n"),
            ("b.md", "# B\n\nサーバの設定。\n"),
        ]);
        let hits: Vec<_> = diags
            .iter()
            .filter(|d| d.rule == "katakana-choon")
            .collect();
        assert_eq!(hits.len(), 1, "少数派（サーバ 1 回）のみ: {diags:?}");
        assert!(hits[0].rel.ends_with("b.md"));
        assert!(
            hits[0].message.contains("サーバ: 1 回") && hits[0].message.contains("サーバー: 2 回"),
            "{}",
            hits[0].message
        );
    }

    #[test]
    fn 長音ゆれの同数タイは両方に警告する() {
        let diags = lint_project_of(&[
            ("a.md", "# A\n\nインタフェース。\n"),
            ("b.md", "# B\n\nインターフェース。\n"),
        ]);
        let hits: Vec<_> = diags
            .iter()
            .filter(|d| d.rule == "katakana-choon")
            .collect();
        assert_eq!(hits.len(), 2, "同数は両方: {diags:?}");
    }

    #[test]
    fn 正規化キーが短い語は長音ゆれの対象外() {
        // カド と カード は正規化キー「カド」= 2 文字 → 偶然一致として除外
        let diags = lint_project_of(&[
            ("a.md", "# A\n\nカードで払う。\n"),
            ("b.md", "# B\n\n道のカドを曲がる。\n"),
        ]);
        assert!(
            diags.iter().all(|d| d.rule != "katakana-choon"),
            "{diags:?}"
        );
    }

    #[test]
    fn 表記が一種類なら長音ゆれの警告なし() {
        let diags = lint_project_of(&[
            ("a.md", "# A\n\nサーバーを起動。\n"),
            ("b.md", "# B\n\nサーバーを停止。\n"),
        ]);
        assert!(diags.is_empty(), "{diags:?}");
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

    #[test]
    fn 表記ゆれ診断は変換候補を_fix_に持つ() {
        let diags = lint("# t\n\nＷｅｂ１２３ と ﾃﾞｰﾀ。\n");
        let full = diags
            .iter()
            .find(|d| d.rule == "fullwidth-alphanumeric")
            .unwrap();
        assert_eq!(full.fix.as_deref(), Some("Web123"));
        let kana = diags.iter().find(|d| d.rule == "halfwidth-kana").unwrap();
        assert_eq!(kana.fix.as_deref(), Some("データ"));

        let diags = lint_terms("# t\n\nサーバを再起動。\n", &[("サーバー", &["サーバ"])]);
        let term = diags.iter().find(|d| d.rule == "term-variant").unwrap();
        assert_eq!(term.fix.as_deref(), Some("サーバー"));
    }

    #[test]
    fn 機械修正できない診断は_fix_なし() {
        let diags = lint("---\ntitle: x\ntags: [a]\n---\n\n# 一つ目\n\n# 二つ目\n\n#### h4\n");
        assert!(!diags.is_empty());
        assert!(diags.iter().all(|d| d.fix.is_none()), "{diags:?}");
    }

    #[test]
    fn 長音ゆれの単独多数派は少数派の_fix_になる() {
        let diags = lint_project_of(&[
            ("a.md", "# A\n\nサーバーを起動。サーバーを停止。\n"),
            ("b.md", "# B\n\nサーバの設定。\n"),
        ]);
        let hit = diags.iter().find(|d| d.rule == "katakana-choon").unwrap();
        assert_eq!(hit.fix.as_deref(), Some("サーバー"));
    }

    #[test]
    fn 長音ゆれの同数タイは_fix_なし() {
        let diags = lint_project_of(&[
            ("a.md", "# A\n\nインタフェース。\n"),
            ("b.md", "# B\n\nインターフェース。\n"),
        ]);
        let hits: Vec<_> = diags
            .iter()
            .filter(|d| d.rule == "katakana-choon")
            .collect();
        assert_eq!(hits.len(), 2, "{diags:?}");
        assert!(
            hits.iter().all(|d| d.fix.is_none()),
            "正解を決められないので自動修正しない: {hits:?}"
        );
    }

    #[test]
    fn apply_fixes_はマルチバイト本文の正しい位置を置換する() {
        // 同一行に term-variant（マルチバイト）と fullwidth-alphanumeric が混在
        let source = "# t\n\n日本語の前置きがあるサーバを再起動し、Ｘ１も直す。\n";
        let opts = LintOptions {
            terms: [("サーバー".to_string(), vec!["サーバ".to_string()])]
                .into_iter()
                .collect(),
            ..LintOptions::default()
        };
        let diags =
            super::lint_page(&page_from(source), &MarkdownOptions::default(), &opts).unwrap();
        let (fixed, n) = super::apply_fixes(source, &diags);
        assert_eq!(n, 2, "{diags:?}");
        assert_eq!(
            fixed,
            "# t\n\n日本語の前置きがあるサーバーを再起動し、X1も直す。\n"
        );
    }

    #[test]
    fn apply_fixes_の適用後は再_lint_で_fix_対象が出ない() {
        // 冪等性: 1 回の適用で表記ゆれが収束する
        let source = "# t\n\nＷｅｂ の ﾃﾞｰﾀ をサーバへ。\n";
        let opts = LintOptions {
            terms: [("サーバー".to_string(), vec!["サーバ".to_string()])]
                .into_iter()
                .collect(),
            ..LintOptions::default()
        };
        let diags =
            super::lint_page(&page_from(source), &MarkdownOptions::default(), &opts).unwrap();
        let (fixed, n) = super::apply_fixes(source, &diags);
        assert_eq!(n, 3, "{diags:?}");
        assert_eq!(fixed, "# t\n\nWeb の データ をサーバーへ。\n");

        let rest =
            super::lint_page(&page_from(&fixed), &MarkdownOptions::default(), &opts).unwrap();
        assert!(rest.iter().all(|d| d.fix.is_none()), "{rest:?}");
    }

    #[test]
    fn 交差する_fix_は先勝ちでスキップされる() {
        let mk = |start_col: usize, end_col: usize, fix: &str| crate::Diagnostic {
            rule: "term-variant",
            severity: crate::Severity::Warning,
            rel: "x.md".into(),
            span: Some(crate::SourceSpan {
                start_line: 1,
                start_col,
                end_line: 1,
                end_col,
            }),
            message: String::new(),
            fix: Some(fix.to_string()),
        };
        // [3,6) を先に適用（後ろ優先）し、交差する [1,4) はスキップ
        let (fixed, n) = super::apply_fixes("abcdef", &[mk(1, 4, "X"), mk(3, 6, "Y")]);
        assert_eq!(n, 1);
        assert_eq!(fixed, "abYf");

        // 同一範囲の重複（term-variant と katakana-choon の二重報告相当）は 1 回だけ
        let (fixed, n) = super::apply_fixes("abcdef", &[mk(2, 5, "Z"), mk(2, 5, "Z")]);
        assert_eq!(n, 1);
        assert_eq!(fixed, "aZef");
    }
}
