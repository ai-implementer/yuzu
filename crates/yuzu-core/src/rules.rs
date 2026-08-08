//! 全診断ルールのレジストリ。
//!
//! **ここが全ルールの ID・深刻度・抑制可否の唯一の定義**。新ルールはここへ登録し、
//! `Diagnostic` を構築する側は ID・severity を直書きせずこの定数を参照する
//! （`SPEC_LANGS` と同じ規律）。docs `reference/rules.md` の一覧との一致は
//! このモジュールのテストが縛る。
//!
//! yuzu-config だけは依存グラフの葉（yuzu-core 非依存）のため定数を共有できない。
//! `yuzu_config::CONFIG_RULES` との一致は yuzu-cli 側（`commands/diag.rs`）の
//! テストが縛る。

use crate::diagnostics::Severity;

/// 1 診断ルールの定義
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rule {
    /// ルール ID（kebab-case ASCII。診断出力の `warning[...]` の中身）
    pub id: &'static str,
    pub severity: Severity,
    /// frontmatter `lintDisable` でページ単位に抑制できるか。
    /// error は「壊れた出力を防ぐ正」なので常に不可（warning のみ true になり得る）
    pub suppressible: bool,
}

/// 抑制可能な warning（文書規約の lint 系）
const fn warning(id: &'static str) -> Rule {
    Rule {
        id,
        severity: Severity::Warning,
        suppressible: true,
    }
}

/// 抑制不可の warning（ページ外を指す config-* 等）
const fn warning_unsuppressible(id: &'static str) -> Rule {
    Rule {
        id,
        severity: Severity::Warning,
        suppressible: false,
    }
}

/// error（常に抑制不可）
const fn error(id: &'static str) -> Rule {
    Rule {
        id,
        severity: Severity::Error,
        suppressible: false,
    }
}

// --- yuzu lint のルール（warning） ---
pub const FULLWIDTH_ALPHANUMERIC: Rule = warning("fullwidth-alphanumeric");
pub const HALFWIDTH_KANA: Rule = warning("halfwidth-kana");
pub const KATAKANA_CHOON: Rule = warning("katakana-choon");
pub const TERM_VARIANT: Rule = warning("term-variant");
pub const DUPLICATE_H1: Rule = warning("duplicate-h1");
pub const HEADING_LEVEL_SKIP: Rule = warning("heading-level-skip");
pub const DIRECTORY_TOO_DEEP: Rule = warning("directory-too-deep");
pub const CODE_BLOCK_META: Rule = warning("code-block-meta");
pub const DUPLICATE_LABEL: Rule = warning("duplicate-label");
pub const FRONTMATTER_UNKNOWN_KEY: Rule = warning("frontmatter-unknown-key");

// --- yuzu.jsonc のルール（warning。ページ外なので lintDisable の対象外） ---
pub const CONFIG_UNKNOWN_KEY: Rule = warning_unsuppressible("config-unknown-key");
pub const CONFIG_DUPLICATE_KEY: Rule = warning_unsuppressible("config-duplicate-key");
pub const CONFIG_PATH_OUTSIDE_ROOT: Rule = warning_unsuppressible("config-path-outside-root");

// --- yuzu check が追加するルール（error） ---
pub const BROKEN_LINK: Rule = error("broken-link");
pub const BROKEN_ANCHOR: Rule = error("broken-anchor");
pub const ALIAS_INVALID: Rule = error("alias-invalid");
pub const ALIAS_CONFLICT: Rule = error("alias-conflict");
pub const ROUTE_CONFLICT: Rule = error("route-conflict");
pub const UNSAFE_PAGE_PATH: Rule = error("unsafe-page-path");
pub const INCLUDE_ERROR: Rule = error("include-error");
pub const SPEC_ERROR: Rule = error("spec-error");
pub const FMT: Rule = error("fmt");

// --- spec-error の警告版（描画が注記へ縮退するだけのもの） ---
pub const SPEC_WARNING: Rule = warning("spec-warning");

// --- 抑制機構自身のルール（warning）。自身を抑制できると古びた抑制が
// 黙って溜まるため、抑制不可で固定する ---
pub const INVALID_LINT_SUPPRESSION: Rule = warning_unsuppressible("invalid-lint-suppression");
pub const UNUSED_LINT_SUPPRESSION: Rule = warning_unsuppressible("unused-lint-suppression");

/// 全ルールの一覧（docs `reference/rules.md` との一致をテストが縛る）
pub const RULES: &[Rule] = &[
    FULLWIDTH_ALPHANUMERIC,
    HALFWIDTH_KANA,
    KATAKANA_CHOON,
    TERM_VARIANT,
    DUPLICATE_H1,
    HEADING_LEVEL_SKIP,
    DIRECTORY_TOO_DEEP,
    CODE_BLOCK_META,
    DUPLICATE_LABEL,
    FRONTMATTER_UNKNOWN_KEY,
    CONFIG_UNKNOWN_KEY,
    CONFIG_DUPLICATE_KEY,
    CONFIG_PATH_OUTSIDE_ROOT,
    BROKEN_LINK,
    BROKEN_ANCHOR,
    ALIAS_INVALID,
    ALIAS_CONFLICT,
    ROUTE_CONFLICT,
    UNSAFE_PAGE_PATH,
    INCLUDE_ERROR,
    SPEC_ERROR,
    FMT,
    SPEC_WARNING,
    INVALID_LINT_SUPPRESSION,
    UNUSED_LINT_SUPPRESSION,
];

/// ID からルール定義を引く（未知 ID は None = `lintDisable` の検証で使う）
pub fn find(id: &str) -> Option<&'static Rule> {
    RULES.iter().find(|r| r.id == id)
}

/// 抑制可能なルール ID の一覧（`lintDisable` の診断文面用）
pub fn suppressible_ids() -> impl Iterator<Item = &'static str> {
    RULES.iter().filter(|r| r.suppressible).map(|r| r.id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ルール_id_は重複しない() {
        let mut seen = std::collections::HashSet::new();
        for rule in RULES {
            assert!(seen.insert(rule.id), "重複: {}", rule.id);
        }
    }

    #[test]
    fn 抑制可能なルールは_warning_のみ() {
        for rule in RULES {
            if rule.suppressible {
                assert_eq!(
                    rule.severity,
                    Severity::Warning,
                    "{} は error なのに抑制可になっている",
                    rule.id
                );
            }
        }
    }

    /// docs のルール一覧ページ（このリポジトリ自身のドキュメント）
    fn rules_md() -> String {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../docs/content/reference/rules.md");
        std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("{} を読めない: {e}", path.display()))
    }

    #[test]
    fn 全ルールが_docs_の一覧に載っている() {
        let doc = rules_md();
        for rule in RULES {
            assert!(
                doc.contains(&format!("`{}`", rule.id)),
                "{} が docs/content/reference/rules.md に載っていない",
                rule.id
            );
        }
    }

    #[test]
    fn docs_の表のルールはすべてレジストリにある() {
        // 表の行は `| \`rule-id\` | …` の形。1 列目だけを ID として照合する
        let doc = rules_md();
        let mut checked = 0usize;
        for line in doc.lines() {
            let Some(rest) = line.strip_prefix("| `") else {
                continue;
            };
            let Some((id, _)) = rest.split_once('`') else {
                continue;
            };
            assert!(
                find(id).is_some(),
                "docs の表の `{id}` がレジストリに無い（改名か削除漏れ）"
            );
            checked += 1;
        }
        assert!(
            checked >= 22,
            "表の行が {checked} 行しか見つからない（形式が変わった？）"
        );
    }
}
