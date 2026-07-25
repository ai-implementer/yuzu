//! コンテンツインクルード（` ```rust file="src/api.rs" lines=10-25 `）の解決。
//!
//! 参照はプロジェクトルート相対で、canonicalize によりルート配下を強制する
//! （openapi / jsonschema の `file:` 参照と同じ規律）。読み込みと行切り出しの
//! 実装はここに 1 つだけ置き、描画（yuzu-render）・検索（yuzu-index）・
//! `yuzu check` が共有する。

use std::path::Path;

use crate::MarkdownOptions;
use crate::diagnostics::{Diagnostic, Severity};
use crate::markdown;
use crate::markdown::fence::{IncludeSpec, parse_fence_info};
use crate::model::Page;

/// 引用元ファイルを読み、指定行範囲を切り出す。
/// 失敗（ルート外・不在・範囲外）は表示用メッセージを Err で返す
pub fn resolve_include(root: &Path, spec: &IncludeSpec) -> Result<String, String> {
    let root = root
        .canonicalize()
        .map_err(|e| format!("プロジェクトルートを解決できません: {e}"))?;
    let canonical = root
        .join(&spec.path)
        .canonicalize()
        .map_err(|e| format!("参照ファイル {} を読めません: {e}", spec.path))?;
    if !canonical.starts_with(&root) {
        return Err(format!(
            "参照ファイル {} はプロジェクトルートの外を指しています",
            spec.path
        ));
    }
    let text = std::fs::read_to_string(&canonical)
        .map_err(|e| format!("参照ファイル {} を読めません: {e}", spec.path))?;
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
}
