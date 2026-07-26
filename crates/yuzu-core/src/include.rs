//! 外部ファイル参照の解決と検証。記法は 2 つある:
//!
//! - コンテンツインクルード（` ```rust file="src/api.rs" lines=10-25 `）
//! - openapi / jsonschema ブロックの `file: <パス>` 参照
//!
//! 2 つが同居しているのは、**ルート配下を canonicalize で強制する同じ規律**
//! （[`read_under_root`]）を共有するため。読み込み・行切り出し・参照の検証の
//! 実装はここに 1 つだけ置き、描画（yuzu-render）・検索（yuzu-index）・
//! `yuzu check` が共有する（2 箇所で解釈するとズレる）。

use std::path::Path;

use crate::MarkdownOptions;
use crate::diagnostics::{DiagBase, Diagnostic, Severity};
use crate::markdown;
use crate::markdown::fence::{IncludeSpec, parse_fence_info};
use crate::model::Page;

/// プロジェクトルート配下を canonicalize で強制してファイルを読む。
/// `label` は表示用の呼び名（「参照ファイル」/「仕様ファイル」）で、
/// メッセージの文言以外は 2 記法で完全に同じ規律
fn read_under_root(root: &Path, rel: &str, label: &str) -> Result<String, String> {
    let root = root
        .canonicalize()
        .map_err(|e| format!("プロジェクトルートを解決できません: {e}"))?;
    let canonical = root
        .join(rel)
        .canonicalize()
        .map_err(|e| format!("{label} {rel} を読めません: {e}"))?;
    if !canonical.starts_with(&root) {
        return Err(format!(
            "{label} {rel} はプロジェクトルートの外を指しています"
        ));
    }
    std::fs::read_to_string(&canonical).map_err(|e| format!("{label} {rel} を読めません: {e}"))
}

// --- コンテンツインクルード（`file=`） ---

/// 引用元ファイルを読み、指定行範囲を切り出す。
/// 失敗（ルート外・不在・範囲外）は表示用メッセージを Err で返す
pub fn resolve_include(root: &Path, spec: &IncludeSpec) -> Result<String, String> {
    let text = read_under_root(root, &spec.path, "参照ファイル")?;
    let Some((start, end)) = spec.lines else {
        return Ok(text);
    };
    let lines: Vec<&str> = text.lines().collect();
    if start > lines.len() {
        return Err(format!(
            "参照ファイル {} の行 {start}-{end} は範囲外です（全 {} 行）",
            spec.path,
            lines.len()
        ));
    }
    // 終端はファイル末尾までに丸める（先頭が範囲内なら引用は成立する）
    let end = end.min(lines.len());
    let mut out = lines[start - 1..end].join("\n");
    out.push('\n');
    Ok(out)
}

/// 全ページのコンテンツインクルードを検証する（`yuzu check` 用）。
/// 参照切れ・ルート外・行範囲外を `include-error`（Error）として報告する
pub fn validate_includes(pages: &[Page], root: &Path, opts: &MarkdownOptions) -> Vec<Diagnostic> {
    let mut diags = Vec::new();
    for page in pages {
        for fence in markdown::extract_fence_meta(&page.source, opts) {
            let (_, meta) = parse_fence_info(&fence.info);
            let Some(spec) = &meta.include else {
                continue;
            };
            if let Err(message) = resolve_include(root, spec) {
                diags.push(Diagnostic {
                    rule: "include-error",
                    severity: Severity::Error,
                    base: DiagBase::Content,
                    rel: page.rel.clone(),
                    span: Some(fence.span),
                    message,
                    fix: None,
                });
            }
        }
    }
    diags
}

// --- API 仕様（openapi / jsonschema）の `file:` 参照 ---

/// ブロック本文が `file: <パス>` の 1 行だけならそのパスを返す。
/// 複数行はインライン仕様（YAML / JSON）とみなす。
/// 呼び出し元は [`resolve_spec_source`] だけ（parse と read を別々に
/// 組み合わせる重複を外から書けないよう非公開にしている）
pub(crate) fn parse_spec_file_ref(body: &str) -> Option<&str> {
    let trimmed = body.trim();
    if trimmed.lines().count() != 1 {
        return None;
    }
    let rel = trimmed.strip_prefix("file:")?.trim();
    (!rel.is_empty()).then_some(rel)
}

/// 仕様ファイルをプロジェクトルート相対で読む（canonicalize でルート配下を強制）。
/// 失敗は表示用メッセージを Err で返す（[`resolve_include`] と同じ規約）
pub fn resolve_spec_file(root: &Path, rel: &str) -> Result<String, String> {
    read_under_root(root, rel, "仕様ファイル")
}

/// API 仕様ブロックの本文をどう解釈したか
#[derive(Debug, PartialEq, Eq)]
pub enum SpecSource<'a> {
    /// 本文がそのまま仕様（YAML / JSON）。ファイルは読んでいない
    Inline(&'a str),
    /// `file: <パス>` の外部参照。`text` は読み込み済みの中身
    File {
        /// プロジェクトルート相対のパス
        rel: &'a str,
        text: String,
    },
}

impl SpecSource<'_> {
    /// 実際に解釈する仕様テキスト
    pub fn text(&self) -> &str {
        match self {
            Self::Inline(body) => body,
            Self::File { text, .. } => text,
        }
    }

    /// `$ref` の相対解決の基点となるルート相対パス。
    /// インラインは None（= プロジェクトルート基準）
    pub fn origin(&self) -> Option<&str> {
        match self {
            Self::Inline(_) => None,
            Self::File { rel, .. } => Some(rel),
        }
    }
}

/// `file:` 参照の解決に失敗したときの情報
#[derive(Debug)]
pub struct SpecRefError<'a> {
    /// 参照パス（構造化ログのフィールド用）
    pub rel: &'a str,
    /// 表示用メッセージ（[`resolve_spec_file`] と同じ文言）
    pub message: String,
}

/// ブロック本文を「実際に解釈する仕様テキスト」へ解決する。
///
/// **読み込み口はクロージャで受ける。** 描画側は外部依存フラグを立てる単一
/// チョークポイント（`ProjectSpecFiles::read`）を、`yuzu check` 側はルート固定の
/// 読み込みを渡す。core が「どう読むか」を決めないので、依存フラグの立て漏れが
/// 構造的に起きない描画側の不変条件を壊さない。
///
/// インラインは本文を借用のまま返す（trim もしない = 呼び出し側がこれまで
/// 渡していた文字列とバイト等価）。読み込みは `file:` のときだけ 1 回呼ばれる
pub fn resolve_spec_source<'a>(
    body: &'a str,
    read: impl FnOnce(&str) -> Result<String, String>,
) -> Result<SpecSource<'a>, SpecRefError<'a>> {
    let Some(rel) = parse_spec_file_ref(body) else {
        return Ok(SpecSource::Inline(body));
    };
    match read(rel) {
        Ok(text) => Ok(SpecSource::File { rel, text }),
        Err(message) => Err(SpecRefError { rel, message }),
    }
}

/// 全ページの openapi / jsonschema ブロックの `file:` 参照を検証する
/// （`yuzu check` 用）。参照切れ・ルート外を `spec-error`（Error）で報告する。
///
/// **仕様の中身（パース失敗・未対応バージョン・`$ref` 先）は見ない。**
/// apispec のパーサは yuzu-render にしか無く、そちらの `validate_api_specs` が
/// 担当する（同じ失敗を二重に報告しない分担）
pub fn validate_spec_refs(pages: &[Page], root: &Path, opts: &MarkdownOptions) -> Vec<Diagnostic> {
    let mut diags = Vec::new();
    for page in pages {
        for fence in markdown::extract_fence_blocks(&page.source, opts) {
            if !fence.lang.as_deref().is_some_and(crate::is_spec_lang) {
                continue;
            }
            // 解決そのものは描画・検査と同じ 1 実装を通す（インラインは Ok で素通り）
            if let Err(e) = resolve_spec_source(&fence.body, |rel| resolve_spec_file(root, rel)) {
                diags.push(Diagnostic {
                    rule: "spec-error",
                    severity: Severity::Error,
                    base: DiagBase::Content,
                    rel: page.rel.clone(),
                    span: Some(fence.span),
                    message: e.message,
                    fix: None,
                });
            }
        }
    }
    diags
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(path: &str, lines: Option<(usize, usize)>) -> IncludeSpec {
        IncludeSpec {
            path: path.to_string(),
            lines,
        }
    }

    fn fixture() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("src")).unwrap();
        std::fs::write(dir.path().join("src/a.rs"), "one\ntwo\nthree\nfour\n").unwrap();
        dir
    }

    #[test]
    fn 行範囲を切り出す() {
        let dir = fixture();
        assert_eq!(
            resolve_include(dir.path(), &spec("src/a.rs", Some((2, 3)))).unwrap(),
            "two\nthree\n"
        );
        assert_eq!(
            resolve_include(dir.path(), &spec("src/a.rs", Some((1, 1)))).unwrap(),
            "one\n"
        );
    }

    #[test]
    fn 範囲なしは全体() {
        let dir = fixture();
        assert_eq!(
            resolve_include(dir.path(), &spec("src/a.rs", None)).unwrap(),
            "one\ntwo\nthree\nfour\n"
        );
    }

    #[test]
    fn 終端はファイル末尾へ丸める() {
        let dir = fixture();
        assert_eq!(
            resolve_include(dir.path(), &spec("src/a.rs", Some((3, 99)))).unwrap(),
            "three\nfour\n"
        );
    }

    #[test]
    fn 開始行が範囲外ならエラー() {
        let dir = fixture();
        let err = resolve_include(dir.path(), &spec("src/a.rs", Some((9, 10)))).unwrap_err();
        assert!(err.contains("範囲外"), "{err}");
        assert!(err.contains("全 4 行"), "{err}");
    }

    #[test]
    fn 不在ファイルは拒否する() {
        let dir = fixture();
        assert!(
            resolve_include(dir.path(), &spec("src/missing.rs", None))
                .unwrap_err()
                .contains("読めません")
        );
    }

    #[test]
    fn ルート外への参照は拒否する() {
        let dir = fixture();
        // ルートを src/ に置くと ../a.rs（= dir/a.rs）はルート配下でない
        std::fs::write(dir.path().join("outside.rs"), "secret\n").unwrap();
        let err =
            resolve_include(&dir.path().join("src"), &spec("../outside.rs", None)).unwrap_err();
        assert!(err.contains("プロジェクトルートの外"), "{err}");
    }

    // --- API 仕様の `file:` 参照 ---

    /// content 直下に 1 ページだけ置いたプロジェクトを作る
    fn project(body: &str) -> (tempfile::TempDir, Vec<Page>) {
        let dir = tempfile::tempdir().unwrap();
        let content = dir.path().join("content");
        std::fs::create_dir_all(&content).unwrap();
        std::fs::write(
            content.join("index.md"),
            format!("---\ntitle: API\n---\n\n{body}"),
        )
        .unwrap();
        let pages = crate::build_source_pages(&content, &[], &MarkdownOptions::default()).unwrap();
        (dir, pages)
    }

    fn refs(dir: &std::path::Path, pages: &[Page]) -> Vec<Diagnostic> {
        validate_spec_refs(pages, dir, &MarkdownOptions::default())
    }

    #[test]
    fn file_参照の判定は一行のみ() {
        assert_eq!(parse_spec_file_ref("file: a.yaml"), Some("a.yaml"));
        assert_eq!(parse_spec_file_ref("file:a.yaml"), Some("a.yaml"));
        assert_eq!(parse_spec_file_ref("file:"), None);
        // 複数行はインライン仕様（YAML）とみなす
        assert_eq!(parse_spec_file_ref("file: a.yaml\nx: 1"), None);
        assert_eq!(parse_spec_file_ref("openapi: 3.0.3"), None);
    }

    #[test]
    fn 本文が_file_の一行なら参照先を読む() {
        let source = resolve_spec_source("file: specs/a.yaml", |rel| {
            assert_eq!(rel, "specs/a.yaml");
            Ok("openapi: 3.0.3\n".to_string())
        })
        .unwrap();
        assert_eq!(source.text(), "openapi: 3.0.3\n");
        assert_eq!(source.origin(), Some("specs/a.yaml"));
    }

    #[test]
    fn インラインは読み込み口を呼ばない() {
        let calls = std::cell::Cell::new(0);
        let body = "openapi: 3.0.3\ninfo:\n  title: X\n";
        let source = resolve_spec_source(body, |_| {
            calls.set(calls.get() + 1);
            Ok(String::new())
        })
        .unwrap();
        assert_eq!(calls.get(), 0, "インラインでファイルを読んではいけない");
        // 未 trim の本文とバイト等価（描画側が従来渡していた文字列と同じ）
        assert_eq!(source.text(), body);
        assert_eq!(source.origin(), None);
    }

    #[test]
    fn 読み込みの失敗はパスとメッセージを返す() {
        let err = resolve_spec_source("file: specs/a.yaml", |_| Err("読めません".to_string()))
            .unwrap_err();
        assert_eq!(err.rel, "specs/a.yaml");
        assert_eq!(err.message, "読めません");
    }

    #[test]
    fn 参照先のファイルが無ければ_spec_error() {
        let (dir, pages) = project("```openapi\nfile: specs/api.yaml\n```\n");
        let diags = refs(dir.path(), &pages);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].rule, "spec-error");
        assert!(diags[0].message.contains("specs/api.yaml"), "{diags:?}");
        // 行番号が付く（ファイル単位の診断ではない）
        assert!(diags[0].span.is_some());
    }

    #[test]
    fn ルート外への仕様参照は_spec_error() {
        let (dir, pages) = project("```openapi\nfile: ../outside.yaml\n```\n");
        let diags = refs(dir.path(), &pages);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].rule, "spec-error");
    }

    #[test]
    fn 参照先が読めれば診断を出さない() {
        let (dir, pages) = project("```openapi\nfile: specs/api.yaml\n```\n");
        std::fs::create_dir_all(dir.path().join("specs")).unwrap();
        // 中身が壊れていても core は黙る（中身の検証は yuzu-render の担当）
        std::fs::write(dir.path().join("specs/api.yaml"), "info:\n  title: [壊れ\n").unwrap();
        assert!(refs(dir.path(), &pages).is_empty());
    }

    #[test]
    fn インライン仕様は診断を出さない() {
        let (dir, pages) = project("```openapi\nopenapi: 3.0.3\npaths: {}\n```\n");
        assert!(refs(dir.path(), &pages).is_empty());
    }

    #[test]
    fn 仕様言語以外のフェンスは対象外() {
        let (dir, pages) = project("```rust\nfile: specs/api.yaml\n```\n");
        assert!(refs(dir.path(), &pages).is_empty());
    }
}
