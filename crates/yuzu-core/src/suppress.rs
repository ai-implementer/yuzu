//! ページ単位の lint 抑制（frontmatter `lintDisable`）の適用。
//!
//! 収集は [`Page::frontmatter`]（`lint_disable`）にあるものをそのまま使い、
//! 適用は診断の報告直前に一括で行う（check / lint の両方が同じ漏斗を通る）。
//! `lintDisable` の検証（未知名・抑制不可名・未使用）もここに一元化する —
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
use crate::markdown;
use crate::model::Page;
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

    // パス 1: 抑制の適用。使われた (ページ, ルール) を控える（未使用検出用）
    let mut kept = Vec::with_capacity(diags.len());
    let mut suppressed = 0usize;
    let mut used: BTreeSet<(&Path, &'static str)> = BTreeSet::new();
    for d in diags {
        if d.base == DiagBase::Content {
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

    // パス 2: `lintDisable` 自身の検証（未知名・抑制不可名・未使用）。
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
            let message = match rules::find(entry) {
                None => format!(
                    "`{entry}` は診断ルール名ではありません（抑制できるルール: {}）",
                    rules::suppressible_ids().collect::<Vec<_>>().join("/")
                ),
                Some(rule) if !rule.suppressible => {
                    if rule.severity == crate::Severity::Error {
                        format!(
                            "`{entry}` は error ルールのため `lintDisable` で抑制できません（error は壊れた出力を防ぐためのルールです）"
                        )
                    } else if rule.id.starts_with("config-") {
                        format!(
                            "`{entry}` は `yuzu.jsonc` を指すルールのため、ページ単位では抑制できません"
                        )
                    } else {
                        format!("`{entry}` は抑制できません")
                    }
                }
                Some(rule) => {
                    if used.contains(&(page.rel.as_path(), rule.id)) {
                        continue; // 正しく効いた抑制
                    }
                    format!(
                        "`{entry}` の抑制はこのページで発火しませんでした（不要になった指定は削除してください）"
                    )
                }
            };
            let is_valid_name = rules::find(entry).is_some_and(|r| r.suppressible);
            let rule = if is_valid_name {
                rules::UNUSED_LINT_SUPPRESSION
            } else {
                rules::INVALID_LINT_SUPPRESSION
            };
            kept.push(Diagnostic {
                rule: rule.id,
                severity: rule.severity,
                base: DiagBase::Content,
                rel: page.rel.clone(),
                span: fm
                    .as_ref()
                    .map(|(raw, fm_span)| entry_span(raw, fm_span, entry)),
                message,
                fix: None,
            });
        }
    }

    SuppressionOutcome {
        diags: kept,
        suppressed,
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
}
