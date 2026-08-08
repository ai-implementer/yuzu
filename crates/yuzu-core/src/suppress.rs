//! lint 抑制の適用（ページ単位 = frontmatter `lintDisable` / 行単位 = 抑制コメント）。
//!
//! - **ページ単位**: [`Page::frontmatter`]（`lint_disable`）をそのまま使う
//! - **行単位**: `<!-- yuzu-lint-disable-next-line <rule> ... -->` の HTML コメントが
//!   「空行を飛ばした次の内容行」の診断だけを抑制する（文字列解釈は
//!   `markdown::suppress_comment` に一元化）
//!
//! 適用は診断の報告直前に一括で行う（check / lint / lint --fix が同じ漏斗を通る）。
//! 検証（未知名・抑制不可名・未使用・壊れたコメント）もここに一元化する —
//! 「どの抑制が実際に使われたか」は適用時にしか分からないため、
//! 検証をパース層に割ると未使用検出と二重報告の回避が成立しない。
//!
//! 抑制できるのは warning ルールのみ（[`crate::rules::Rule::suppressible`]）。
//! error は build を中断する・壊れた出力を防ぐ正なので対象外。
//! `config-*` は `yuzu.jsonc` を指す（`DiagBase::ProjectRoot`）ため、
//! 突き合わせがページ単位で成立せず構造的に素通りする。

use std::collections::{BTreeSet, HashMap};
use std::path::Path;

use crate::MarkdownOptions;
use crate::diagnostics::{DiagBase, Diagnostic};
use crate::markdown::{self, suppress_comment, suppress_comment::SuppressCommentKind};
use crate::model::{Page, SourceSpan};
use crate::rules;

/// [`apply_suppressions`] の結果
pub struct SuppressionOutcome {
    /// 抑制適用後の診断（`invalid-lint-suppression` / `unused-lint-suppression` を含む）
    pub diags: Vec<Diagnostic>,
    /// 抑制で落とした件数（集計行・`--format json` の表示用）
    pub suppressed: usize,
}

/// frontmatter `lintDisable` を診断へ適用する。
/// 呼び出しは cli の報告直前（check / lint）と `lint --fix` のループ内。
/// `--fix` 側もこれを通すことで「報告されないのに書き換える」非対称を防ぐ
pub fn apply_suppressions(
    diags: Vec<Diagnostic>,
    pages: &[Page],
    opts: &MarkdownOptions,
) -> SuppressionOutcome {
    // rel → 抑制ルール集合（重複エントリはここで畳む。合成ページは
    // frontmatter を持たないので lint_disable は常に空 = 自然に対象外）
    let by_page: HashMap<&Path, BTreeSet<&str>> = pages
        .iter()
        .filter(|p| !p.is_generated() && !p.frontmatter.lint_disable.is_empty())
        .map(|p| {
            (
                p.rel.as_path(),
                p.frontmatter
                    .lint_disable
                    .iter()
                    .map(String::as_str)
                    .collect(),
            )
        })
        .collect();

    // 行単位（コメント）の収集。全ページの再パースを避けるため文字列ガードで絞る
    // （`<!--yuzu-lint-`（空白なし）も受理するので接頭辞 `yuzu-lint-` で見る。
    // コードブロック内の偽陽性は再パース 1 回のコストで済む）
    let by_page_comments: HashMap<&Path, PageComments> = pages
        .iter()
        .filter(|p| !p.is_generated() && p.source.contains("yuzu-lint-"))
        .map(|p| (p.rel.as_path(), collect_page_comments(&p.source, opts)))
        .collect();

    // パス 1: 抑制の適用。行単位（1a）→ ページ単位（1b）の順に照合する
    // （狭いスコープ優先。両方に該当する診断は行コメント側の used になり、
    // ページ単位側が unused 警告になる = 広い抑制を狭い抑制へ促す方向）。
    // 1 診断は 1 回しか落ちないので suppressed の二重加算はない
    let mut kept = Vec::with_capacity(diags.len());
    let mut suppressed = 0usize;
    let mut used: BTreeSet<(&Path, &'static str)> = BTreeSet::new();
    let mut used_line: HashMap<&Path, BTreeSet<(usize, &'static str)>> = HashMap::new();
    for d in diags {
        if d.base == DiagBase::Content {
            // 1a: 行単位。span を持つ診断だけが対象（span: None の
            // directory-too-deep 等はページ単位でのみ抑制できる）
            if let (Some(span), Some((rel, pc))) =
                (d.span, by_page_comments.get_key_value(d.rel.as_path()))
            {
                // 同じ行を狙う重複コメントは最初の 1 つだけ used にする
                // （2 つ目は unused になり削除を促す）
                let hit = pc.line.iter().position(|ls| {
                    ls.target_line == Some(span.start_line)
                        && ls.rules.iter().any(|r| r == d.rule)
                        && rules::find(d.rule).is_some_and(|ru| ru.suppressible)
                });
                if let Some(idx) = hit {
                    used_line.entry(rel).or_default().insert((idx, d.rule));
                    suppressed += 1;
                    continue;
                }
            }
            // 1b: ページ単位（frontmatter lintDisable）
            if let Some((rel, set)) = by_page.get_key_value(d.rel.as_path()) {
                if set.contains(d.rule) && rules::find(d.rule).is_some_and(|r| r.suppressible) {
                    used.insert((rel, d.rule));
                    suppressed += 1;
                    continue;
                }
            }
        }
        kept.push(d);
    }

    // パス 2a: 行コメントの検証（壊れたコメント・未知名・抑制不可名・未使用）。
    // span はすべてコメント自身を指す
    for page in pages.iter().filter(|p| !p.is_generated()) {
        let Some(pc) = by_page_comments.get(page.rel.as_path()) else {
            continue;
        };
        for (kind, span) in &pc.broken {
            kept.push(meta_diag(
                rules::INVALID_LINT_SUPPRESSION,
                page,
                Some(*span),
                broken_comment_message(kind),
            ));
        }
        let page_used = used_line.get(page.rel.as_path());
        for (idx, ls) in pc.line.iter().enumerate() {
            for name in &ls.rules {
                let (rule, message) = match invalid_entry_reason(name) {
                    Some(mut reason) => {
                        if name.contains(',') {
                            reason.push_str("（ルール名は空白区切りで指定します）");
                        }
                        (rules::INVALID_LINT_SUPPRESSION, reason)
                    }
                    None => {
                        let is_used = page_used
                            .is_some_and(|u| u.iter().any(|(i, r)| *i == idx && *r == name));
                        if is_used {
                            continue; // 正しく効いた抑制
                        }
                        (
                            rules::UNUSED_LINT_SUPPRESSION,
                            format!(
                                "`{name}` の抑制は次の内容行で発火しませんでした（不要になったコメントは削除してください）"
                            ),
                        )
                    }
                };
                kept.push(meta_diag(rule, page, Some(ls.span), message));
            }
        }
    }

    // パス 2b: `lintDisable` 自身の検証（未知名・抑制不可名・未使用）。
    // 抑制が「黙って効かない」のは config-unknown-key と同じ事故クラスなので
    // 無視せず警告する
    for page in pages
        .iter()
        .filter(|p| !p.is_generated() && !p.frontmatter.lint_disable.is_empty())
    {
        let fm = markdown::frontmatter_raw(&page.source, opts);
        let entries: BTreeSet<&str> = page
            .frontmatter
            .lint_disable
            .iter()
            .map(String::as_str)
            .collect();
        for entry in entries {
            let (rule, message) = match invalid_entry_reason(entry) {
                Some(reason) => (rules::INVALID_LINT_SUPPRESSION, reason),
                None => {
                    let id = rules::find(entry)
                        .expect("invalid でない名前は必ず引ける")
                        .id;
                    if used.contains(&(page.rel.as_path(), id)) {
                        continue; // 正しく効いた抑制
                    }
                    (
                        rules::UNUSED_LINT_SUPPRESSION,
                        format!(
                            "`{entry}` の抑制はこのページで発火しませんでした（不要になった指定は削除してください）"
                        ),
                    )
                }
            };
            kept.push(meta_diag(
                rule,
                page,
                fm.as_ref()
                    .map(|(raw, fm_span)| entry_span(raw, fm_span, entry)),
                message,
            ));
        }
    }

    SuppressionOutcome {
        diags: kept,
        suppressed,
    }
}

/// 行単位の抑制コメント 1 件（正しい next-line 指定）
struct LineSuppression {
    /// 抑制するルール名（コメント内の重複は畳み済み・ソート済み）
    rules: Vec<String>,
    /// 照合先の内容行（1 始まり）。文書末尾で内容行が無ければ None = 必ず unused
    target_line: Option<usize>,
    /// コメント自身の span（unused の報告位置）
    span: SourceSpan,
}

/// 1 ページぶんの抑制コメント（正しい指定と壊れたコメントに分類済み）
struct PageComments {
    line: Vec<LineSuppression>,
    broken: Vec<(SuppressCommentKind, SourceSpan)>,
}

/// ページの抑制コメントを収集し、各コメントの照合先（次の内容行）を確定する。
/// 「次の内容行」= コメントの後、空行と**他の抑制コメントの行**を飛ばした最初の行
/// （コメントを縦に積んだとき下のコメント行を対象に取らない・invalid の行も
/// 飛ばす = 修正中でも照合が暴れない）
fn collect_page_comments(source: &str, opts: &MarkdownOptions) -> PageComments {
    let comments = markdown::extract_suppress_comments(source, opts);
    let comment_lines: BTreeSet<usize> = comments
        .iter()
        .flat_map(|c| c.span.start_line..=c.span.end_line)
        .collect();
    let lines: Vec<&str> = source.lines().collect();
    let mut out = PageComments {
        line: Vec::new(),
        broken: Vec::new(),
    };
    for c in comments {
        match c.kind {
            SuppressCommentKind::NextLine { rules } => {
                // コメント内の重複ルール名は畳む（報告も 1 回にする）
                let rules: Vec<String> = rules
                    .into_iter()
                    .collect::<BTreeSet<_>>()
                    .into_iter()
                    .collect();
                let target_line = (c.span.end_line + 1..=lines.len()).find(|&n| {
                    !comment_lines.contains(&n) && !suppress_comment::is_content_blank(lines[n - 1])
                });
                out.line.push(LineSuppression {
                    rules,
                    target_line,
                    span: c.span,
                });
            }
            kind => out.broken.push((kind, c.span)),
        }
    }
    out
}

/// 抑制できないエントリの理由（None = 抑制可能な正しいルール名）。
/// ページ単位（lintDisable）と行コメントが同じ文言を共有する
fn invalid_entry_reason(entry: &str) -> Option<String> {
    match rules::find(entry) {
        None => Some(format!(
            "`{entry}` は診断ルール名ではありません（抑制できるルール: {}）",
            rules::suppressible_ids().collect::<Vec<_>>().join("/")
        )),
        Some(rule) if !rule.suppressible => Some(if rule.severity == crate::Severity::Error {
            format!(
                "`{entry}` は error ルールのため抑制できません（error は壊れた出力を防ぐためのルールです）"
            )
        } else if rule.id.starts_with("config-") {
            format!("`{entry}` は `yuzu.jsonc` を指すルールのため抑制できません")
        } else {
            format!("`{entry}` は抑制できません")
        }),
        Some(_) => None,
    }
}

/// 壊れた抑制コメントの診断文面
fn broken_comment_message(kind: &SuppressCommentKind) -> String {
    match kind {
        SuppressCommentKind::Unclosed => {
            "抑制コメントは 1 行で書いてください（閉じ `-->` が同じ行にありません。閉じ忘れは以降の本文を丸ごと HTML コメントとして飲み込みます）"
                .to_string()
        }
        SuppressCommentKind::UnknownDirective { directive } => format!(
            "`{directive}` は未知のディレクティブです（対応: `{}`）",
            suppress_comment::NEXT_LINE_DIRECTIVE
        ),
        SuppressCommentKind::Empty => format!(
            "抑制するルール名を空白区切りで 1 つ以上指定してください（例: `<!-- {} term-variant -->`）",
            suppress_comment::NEXT_LINE_DIRECTIVE
        ),
        SuppressCommentKind::NotStandalone => {
            "抑制コメントは単独の行に書いてください（行の途中や本文と同じ行では効きません）"
                .to_string()
        }
        SuppressCommentKind::NextLine { .. } => unreachable!("NextLine は broken に入らない"),
    }
}

/// invalid / unused 警告を組み立てる（span はコメント・エントリ自身の位置）
fn meta_diag(
    rule: rules::Rule,
    page: &Page,
    span: Option<SourceSpan>,
    message: String,
) -> Diagnostic {
    Diagnostic {
        rule: rule.id,
        severity: rule.severity,
        base: DiagBase::Content,
        rel: page.rel.clone(),
        span,
        message,
        fix: None,
    }
}

/// `lintDisable` のエントリが書かれた frontmatter 行の span を探す。
/// 値の行 → `lintDisable:` キー行 → frontmatter 全体、の順でフォールバック
/// （`aliases.rs::alias_span` と同じ方針）
fn entry_span(
    raw: &str,
    fm_span: &crate::model::SourceSpan,
    entry: &str,
) -> crate::model::SourceSpan {
    if !entry.is_empty() {
        for (idx, line) in raw.lines().enumerate() {
            if line.contains(entry) {
                return line_span(fm_span, idx, line);
            }
        }
    }
    crate::lint::key_span(raw, fm_span, "lintDisable")
}

/// frontmatter 内の行インデックスを文書全体の 1 行 span へ変換する
fn line_span(
    fm_span: &crate::model::SourceSpan,
    idx: usize,
    line: &str,
) -> crate::model::SourceSpan {
    let line_no = fm_span.start_line + idx;
    crate::model::SourceSpan {
        start_line: line_no,
        start_col: 1,
        end_line: line_no,
        end_col: line.chars().count().max(1),
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::apply_suppressions;
    use crate::{Diagnostic, LintOptions, MarkdownOptions, Page, build_source_pages};

    /// 複数ページのプロジェクトを組んでページ一覧を得る
    fn pages_of(pages_src: &[(&str, &str)]) -> Vec<Page> {
        let dir = tempfile::tempdir().unwrap();
        for (rel, source) in pages_src {
            let path = dir.path().join(rel);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(&path, source).unwrap();
        }
        build_source_pages(dir.path(), &[], &MarkdownOptions::default()).unwrap()
    }

    /// lint_page ＋ lint_project を回して抑制を適用する
    fn lint_suppressed(pages_src: &[(&str, &str)]) -> (Vec<Diagnostic>, usize) {
        let opts = MarkdownOptions::default();
        let lint_opts = LintOptions::default();
        let pages = pages_of(pages_src);
        let mut diags = Vec::new();
        for page in &pages {
            diags.extend(crate::lint_page(page, &opts, &lint_opts).unwrap());
        }
        diags.extend(crate::lint_project(&pages, &opts, &lint_opts).unwrap());
        let outcome = apply_suppressions(diags, &pages, &opts);
        (outcome.diags, outcome.suppressed)
    }

    #[test]
    fn 抑制したルールはそのページの診断から消える() {
        let (diags, suppressed) = lint_suppressed(&[(
            "index.md",
            "---\nlintDisable:\n  - fullwidth-alphanumeric\n---\n\n# t\n\nＷｅｂ。\n",
        )]);
        assert_eq!(suppressed, 1);
        assert!(
            diags.iter().all(|d| d.rule != "fullwidth-alphanumeric"),
            "{diags:?}"
        );
        // 効いた抑制に unused は出ない
        assert!(
            diags.iter().all(|d| d.rule != "unused-lint-suppression"),
            "{diags:?}"
        );
    }

    #[test]
    fn 他ページの同じルールは抑制されない() {
        let (diags, suppressed) = lint_suppressed(&[
            (
                "a.md",
                "---\nlintDisable: [fullwidth-alphanumeric]\n---\n\n# A\n\nＷｅｂ。\n",
            ),
            ("b.md", "# B\n\nＸ１。\n"),
        ]);
        assert_eq!(suppressed, 1, "{diags:?}");
        let hits: Vec<_> = diags
            .iter()
            .filter(|d| d.rule == "fullwidth-alphanumeric")
            .collect();
        assert_eq!(hits.len(), 1, "{diags:?}");
        assert!(hits[0].rel.ends_with("b.md"));
    }

    #[test]
    fn error_ルールの抑制指定は_invalid_警告になる() {
        let (diags, suppressed) =
            lint_suppressed(&[("index.md", "---\nlintDisable: [broken-link]\n---\n\n# t\n")]);
        assert_eq!(suppressed, 0);
        let hits: Vec<_> = diags
            .iter()
            .filter(|d| d.rule == "invalid-lint-suppression")
            .collect();
        assert_eq!(hits.len(), 1, "{diags:?}");
        assert!(
            hits[0].message.contains("error ルールのため"),
            "{}",
            hits[0].message
        );
        // invalid なエントリに unused を二重報告しない
        assert!(diags.iter().all(|d| d.rule != "unused-lint-suppression"));
    }

    #[test]
    fn 未知のルール名は_invalid_警告になる() {
        let (diags, _) =
            lint_suppressed(&[("index.md", "---\nlintDisable: [no-such-rule]\n---\n\n# t\n")]);
        let hit = diags
            .iter()
            .find(|d| d.rule == "invalid-lint-suppression")
            .unwrap();
        assert!(
            hit.message
                .contains("`no-such-rule` は診断ルール名ではありません"),
            "{}",
            hit.message
        );
        assert!(
            hit.message.contains("term-variant"),
            "抑制できるルールの一覧を含める: {}",
            hit.message
        );
    }

    #[test]
    fn 発火しなかった抑制は_unused_警告になる() {
        let (diags, suppressed) = lint_suppressed(&[(
            "index.md",
            "---\ntitle: x\nlintDisable:\n  - duplicate-h1\n---\n\n# t\n",
        )]);
        assert_eq!(suppressed, 0);
        let hit = diags
            .iter()
            .find(|d| d.rule == "unused-lint-suppression")
            .unwrap();
        assert!(
            hit.message
                .contains("`duplicate-h1` の抑制はこのページで発火しません"),
            "{}",
            hit.message
        );
        assert_eq!(
            hit.span.unwrap().start_line,
            4,
            "`- duplicate-h1` の行を指す: {hit:?}"
        );
    }

    #[test]
    fn config_診断は_lintdisable_の影響を受けない() {
        let opts = MarkdownOptions::default();
        let pages = pages_of(&[(
            "index.md",
            "---\nlintDisable: [config-unknown-key]\n---\n\n# t\n",
        )]);
        // config 診断は DiagBase::ProjectRoot（cli の変換と同じ形）
        let config_diag = Diagnostic {
            rule: crate::rules::CONFIG_UNKNOWN_KEY.id,
            severity: crate::rules::CONFIG_UNKNOWN_KEY.severity,
            base: crate::DiagBase::ProjectRoot,
            rel: "yuzu.jsonc".into(),
            span: None,
            message: "未知のキー".to_string(),
            fix: None,
        };
        let outcome = apply_suppressions(vec![config_diag], &pages, &opts);
        assert_eq!(outcome.suppressed, 0);
        assert!(
            outcome.diags.iter().any(|d| d.rule == "config-unknown-key"),
            "config 診断は残る: {:?}",
            outcome.diags
        );
        let invalid = outcome
            .diags
            .iter()
            .find(|d| d.rule == "invalid-lint-suppression")
            .unwrap();
        assert!(
            invalid.message.contains("`yuzu.jsonc` を指すルール"),
            "{}",
            invalid.message
        );
    }

    #[test]
    fn 長音ゆれの多数決の母数は抑制しても変わらない() {
        // 多数派（サーバー×2）のページが katakana-choon を抑制しても、
        // 出現は母数に残る = 少数派ページへの警告は従来どおり出る
        let (diags, suppressed) = lint_suppressed(&[
            (
                "a.md",
                "---\nlintDisable: [katakana-choon]\n---\n\n# A\n\nサーバーを起動。サーバーを停止。\n",
            ),
            ("b.md", "# B\n\nサーバの設定。\n"),
        ]);
        let hits: Vec<_> = diags
            .iter()
            .filter(|d| d.rule == "katakana-choon")
            .collect();
        assert_eq!(hits.len(), 1, "{diags:?}");
        assert!(hits[0].rel.ends_with("b.md"));
        // a.md 側は警告が出ていない（= 抑制も発火していない）ので unused になる
        assert_eq!(suppressed, 0);
        assert!(
            diags.iter().any(|d| d.rule == "unused-lint-suppression"),
            "{diags:?}"
        );
    }

    #[test]
    fn 重複エントリは_1_回だけ報告する() {
        let (diags, _) = lint_suppressed(&[(
            "index.md",
            "---\nlintDisable: [duplicate-h1, duplicate-h1]\n---\n\n# t\n",
        )]);
        let unused: Vec<_> = diags
            .iter()
            .filter(|d| d.rule == "unused-lint-suppression")
            .collect();
        assert_eq!(unused.len(), 1, "{diags:?}");
    }

    // --- 行単位（抑制コメント）---

    #[test]
    fn 行コメントは次の内容行の診断だけ抑制する() {
        let (diags, suppressed) = lint_suppressed(&[(
            "index.md",
            "# t\n\n<!-- yuzu-lint-disable-next-line fullwidth-alphanumeric -->\nＷｅｂ。\n\nＸ１。\n",
        )]);
        assert_eq!(suppressed, 1, "{diags:?}");
        let hits: Vec<_> = diags
            .iter()
            .filter(|d| d.rule == "fullwidth-alphanumeric")
            .collect();
        assert_eq!(hits.len(), 1, "6 行目の分だけ残る: {diags:?}");
        assert_eq!(hits[0].span.unwrap().start_line, 6);
        assert!(
            diags.iter().all(|d| d.rule != "unused-lint-suppression"),
            "{diags:?}"
        );
    }

    #[test]
    fn 空行を挟んでも次の内容行に効く() {
        let (diags, suppressed) = lint_suppressed(&[(
            "index.md",
            "# t\n\n<!-- yuzu-lint-disable-next-line fullwidth-alphanumeric -->\n\nＷｅｂ。\n",
        )]);
        assert_eq!(suppressed, 1, "{diags:?}");
        assert!(diags.iter().all(|d| d.rule != "fullwidth-alphanumeric"));
    }

    #[test]
    fn 一つのコメントで複数ルールを抑制できる() {
        let (diags, suppressed) = lint_suppressed(&[(
            "index.md",
            "# t\n\n<!-- yuzu-lint-disable-next-line fullwidth-alphanumeric halfwidth-kana -->\nＷｅｂ ﾃﾞｰﾀ。\n",
        )]);
        assert_eq!(suppressed, 2, "{diags:?}");
        assert!(
            diags
                .iter()
                .all(|d| d.rule != "fullwidth-alphanumeric" && d.rule != "halfwidth-kana"),
            "{diags:?}"
        );
        assert!(diags.iter().all(|d| d.rule != "unused-lint-suppression"));
    }

    #[test]
    fn 積んだ抑制コメントは互いを飛ばして照合する() {
        let (diags, suppressed) = lint_suppressed(&[(
            "index.md",
            "# t\n\n<!-- yuzu-lint-disable-next-line fullwidth-alphanumeric -->\n<!-- yuzu-lint-disable-next-line halfwidth-kana -->\nＷｅｂ ﾃﾞｰﾀ。\n",
        )]);
        assert_eq!(suppressed, 2, "{diags:?}");
        assert!(
            diags.iter().all(|d| d.rule != "unused-lint-suppression"),
            "両方のコメントが使われる: {diags:?}"
        );
    }

    #[test]
    fn 発火しなかったルール名はコメント行を指す_unused_警告になる() {
        let (diags, suppressed) = lint_suppressed(&[(
            "index.md",
            "# t\n\n<!-- yuzu-lint-disable-next-line duplicate-h1 -->\n本文。\n",
        )]);
        assert_eq!(suppressed, 0);
        let hit = diags
            .iter()
            .find(|d| d.rule == "unused-lint-suppression")
            .unwrap();
        assert!(
            hit.message.contains("次の内容行で発火しません"),
            "{}",
            hit.message
        );
        assert_eq!(hit.span.unwrap().start_line, 3, "コメント行を指す");
    }

    #[test]
    fn 裸コメントは_invalid_警告になる() {
        let (diags, _) = lint_suppressed(&[(
            "index.md",
            "# t\n\n<!-- yuzu-lint-disable-next-line -->\n本文。\n",
        )]);
        let hit = diags
            .iter()
            .find(|d| d.rule == "invalid-lint-suppression")
            .unwrap();
        assert!(hit.message.contains("1 つ以上指定"), "{}", hit.message);
    }

    #[test]
    fn 未知ディレクティブは_invalid_警告になる() {
        // 予約語彙（disable-line）も未知として拾う
        let (diags, _) = lint_suppressed(&[(
            "index.md",
            "# t\n\n<!-- yuzu-lint-disable-line term-variant -->\n本文。\n",
        )]);
        let hit = diags
            .iter()
            .find(|d| d.rule == "invalid-lint-suppression")
            .unwrap();
        assert!(
            hit.message.contains("未知のディレクティブ")
                && hit.message.contains("yuzu-lint-disable-next-line"),
            "{}",
            hit.message
        );
    }

    #[test]
    fn 閉じ忘れコメントは_invalid_警告になる() {
        let (diags, suppressed) = lint_suppressed(&[(
            "index.md",
            "# t\n\n<!-- yuzu-lint-disable-next-line fullwidth-alphanumeric\n\nＷｅｂ。\n",
        )]);
        assert_eq!(suppressed, 0);
        let hit = diags
            .iter()
            .find(|d| d.rule == "invalid-lint-suppression")
            .unwrap();
        assert!(hit.message.contains("閉じ"), "{}", hit.message);
        let span = hit.span.unwrap();
        assert_eq!(
            (span.start_line, span.end_line),
            (3, 3),
            "報告位置は開始行の 1 行に絞る: {span:?}"
        );
    }

    #[test]
    fn 段落中のインラインコメントは_invalid_警告になる() {
        let (diags, suppressed) = lint_suppressed(&[(
            "index.md",
            "# t\n\n本文 <!-- yuzu-lint-disable-next-line term-variant --> 続き。\n",
        )]);
        assert_eq!(suppressed, 0);
        let hit = diags
            .iter()
            .find(|d| d.rule == "invalid-lint-suppression")
            .unwrap();
        assert!(hit.message.contains("単独の行"), "{}", hit.message);
    }

    #[test]
    fn error_ルール名の行コメントは_invalid_警告になる() {
        let (diags, _) = lint_suppressed(&[(
            "index.md",
            "# t\n\n<!-- yuzu-lint-disable-next-line broken-link -->\n本文。\n",
        )]);
        let hit = diags
            .iter()
            .find(|d| d.rule == "invalid-lint-suppression")
            .unwrap();
        assert!(
            hit.message.contains("error ルールのため"),
            "{}",
            hit.message
        );
        assert!(
            diags.iter().all(|d| d.rule != "unused-lint-suppression"),
            "invalid に unused を二重報告しない: {diags:?}"
        );
    }

    #[test]
    fn カンマ区切りのルール名はヒント付き_invalid_になる() {
        let (diags, _) = lint_suppressed(&[(
            "index.md",
            "# t\n\n<!-- yuzu-lint-disable-next-line fullwidth-alphanumeric,halfwidth-kana -->\nＷｅｂ。\n",
        )]);
        let hit = diags
            .iter()
            .find(|d| d.rule == "invalid-lint-suppression")
            .unwrap();
        assert!(
            hit.message.contains("空白区切りで指定します"),
            "{}",
            hit.message
        );
    }

    #[test]
    fn 行単位とページ単位が重なると行単位が使われページ単位は_unused_になる() {
        let (diags, suppressed) = lint_suppressed(&[(
            "index.md",
            "---\nlintDisable: [fullwidth-alphanumeric]\n---\n\n# t\n\n<!-- yuzu-lint-disable-next-line fullwidth-alphanumeric -->\nＷｅｂ。\n",
        )]);
        assert_eq!(suppressed, 1, "{diags:?}");
        assert!(diags.iter().all(|d| d.rule != "fullwidth-alphanumeric"));
        let unused: Vec<_> = diags
            .iter()
            .filter(|d| d.rule == "unused-lint-suppression")
            .collect();
        assert_eq!(unused.len(), 1, "ページ単位側だけ unused: {diags:?}");
        assert!(
            unused[0].message.contains("このページで発火しません"),
            "ページ単位の文面: {}",
            unused[0].message
        );
    }

    #[test]
    fn コードブロック内の記法例は抑制コメントとして解釈されない() {
        let (diags, suppressed) = lint_suppressed(&[(
            "index.md",
            "# t\n\n```md\n<!-- yuzu-lint-disable-next-line term-variant -->\n```\n\n`<!-- yuzu-lint-disable-next-line x -->` の説明。\n",
        )]);
        assert_eq!(suppressed, 0);
        assert!(
            diags.iter().all(
                |d| d.rule != "invalid-lint-suppression" && d.rule != "unused-lint-suppression"
            ),
            "{diags:?}"
        );
    }

    #[test]
    fn span_を持たない診断は行コメントでは抑制されず_unused_になる() {
        let opts = MarkdownOptions::default();
        let pages = pages_of(&[(
            "index.md",
            "<!-- yuzu-lint-disable-next-line directory-too-deep -->\n# t\n",
        )]);
        // directory-too-deep はファイル単位（span なし）の warning
        let diag = Diagnostic {
            rule: crate::rules::DIRECTORY_TOO_DEEP.id,
            severity: crate::rules::DIRECTORY_TOO_DEEP.severity,
            base: crate::DiagBase::Content,
            rel: "index.md".into(),
            span: None,
            message: "深すぎる".to_string(),
            fix: None,
        };
        let outcome = apply_suppressions(vec![diag], &pages, &opts);
        assert_eq!(outcome.suppressed, 0);
        assert!(
            outcome.diags.iter().any(|d| d.rule == "directory-too-deep"),
            "span なしは行コメントで落ちない: {:?}",
            outcome.diags
        );
        assert!(
            outcome
                .diags
                .iter()
                .any(|d| d.rule == "unused-lint-suppression"),
            "{:?}",
            outcome.diags
        );
    }

    #[test]
    fn 引用ブロック内の行コメントも効く() {
        let (diags, suppressed) = lint_suppressed(&[(
            "index.md",
            "# t\n\n> <!-- yuzu-lint-disable-next-line fullwidth-alphanumeric -->\n> Ｗｅｂ。\n",
        )]);
        assert_eq!(suppressed, 1, "{diags:?}");
        assert!(diags.iter().all(|d| d.rule != "fullwidth-alphanumeric"));
    }

    #[test]
    fn コメント内の重複ルール名は_1_回だけ報告する() {
        let (diags, _) = lint_suppressed(&[(
            "index.md",
            "# t\n\n<!-- yuzu-lint-disable-next-line duplicate-h1 duplicate-h1 -->\n本文。\n",
        )]);
        let unused: Vec<_> = diags
            .iter()
            .filter(|d| d.rule == "unused-lint-suppression")
            .collect();
        assert_eq!(unused.len(), 1, "{diags:?}");
    }
}
