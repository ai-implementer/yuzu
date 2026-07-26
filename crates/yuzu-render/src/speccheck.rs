//! openapi / jsonschema ブロックの検証（`yuzu check` 用）。
//!
//! 描画（[`crate::render_site`]）は「Err を返さない」方針で、失敗しても
//! エラーボックス HTML や小さな注記にしてビルドを継続する。そのため
//! **仕様ファイルを消す・壊しても終了コードは 0 のまま**だった。
//! ここは描画と同じ解釈で検証だけを行い、`yuzu check` に報告させる
//! （描画側の寛容さは変えない = 執筆中にビルドが止まらない）。
//!
//! 検証を yuzu-render に置くのは、apispec のパーサ（版判定・`$ref` の
//! ファイル間解決）がこの crate にしか無いため。yuzu-core に別実装を
//! 作ると解釈がズレるので、`file:` の解釈とファイル読みだけは core の
//! 1 実装（`parse_spec_file_ref` / `resolve_spec_file`）を共有する。

use std::path::Path;

use yuzu_core::{Diagnostic, MarkdownOptions, Page, Severity};

use crate::apispec::{self, SpecFiles, SpecKind};

/// 検証用のファイル読み込み口。描画側の `ProjectSpecFiles` と違い
/// 外部依存フラグ（本文キャッシュ非対象化）は持たない
struct CheckSpecFiles<'a> {
    root: &'a Path,
}

impl SpecFiles for CheckSpecFiles<'_> {
    fn read(&self, rel: &str) -> Result<String, String> {
        yuzu_core::resolve_spec_file(self.root, rel)
    }
}

/// ` ```openapi ` / ` ```jsonschema ` ブロックを検証する（`yuzu check` 用）。
///
/// - `spec-error`（Error）: `file:` 参照の切れ・ルート外、仕様のパース失敗、
///   未対応バージョン、`$ref` 先ファイルの読み込み・パース失敗
/// - `spec-warning`（Warning）: 参照ファイル数の上限超過など、描画が注記へ
///   縮退するだけで意味は通るもの
///
/// 引数の並びは [`yuzu_core::validate_includes`] と揃えてある
pub fn validate_api_specs(pages: &[Page], root: &Path, opts: &MarkdownOptions) -> Vec<Diagnostic> {
    let files = CheckSpecFiles { root };
    let mut diags = Vec::new();
    for page in pages {
        for fence in yuzu_core::extract_fence_blocks(&page.source, opts) {
            let Some(kind) = fence.lang.as_deref().and_then(spec_kind) else {
                continue;
            };
            // `file: <パス>` の 1 行ならファイルを読んでから検証、
            // それ以外はインライン仕様としてそのまま検証する
            let (source, origin) = match yuzu_core::parse_spec_file_ref(&fence.body) {
                Some(rel) => match files.read(rel) {
                    Ok(text) => (text, Some(rel.to_string())),
                    Err(message) => {
                        diags.push(diag(page, &fence, message, Severity::Error));
                        continue;
                    }
                },
                None => (fence.body.clone(), None),
            };
            for issue in apispec::check_spec(kind, &source, origin.as_deref(), &files) {
                let severity = if issue.fatal {
                    Severity::Error
                } else {
                    Severity::Warning
                };
                diags.push(diag(page, &fence, issue.message, severity));
            }
        }
    }
    diags
}

fn spec_kind(lang: &str) -> Option<SpecKind> {
    match lang {
        "openapi" => Some(SpecKind::OpenApi),
        "jsonschema" => Some(SpecKind::JsonSchema),
        _ => None,
    }
}

fn diag(
    page: &Page,
    fence: &yuzu_core::FenceBlock,
    message: String,
    severity: Severity,
) -> Diagnostic {
    Diagnostic {
        rule: match severity {
            Severity::Error => "spec-error",
            Severity::Warning => "spec-warning",
        },
        severity,
        rel: page.rel.clone(),
        span: Some(fence.span),
        message,
        fix: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// content 直下に 1 ページだけ置いたプロジェクトを作る
    fn project(body: &str) -> (tempfile::TempDir, Vec<Page>) {
        let dir = tempfile::tempdir().expect("一時ディレクトリ");
        let content = dir.path().join("content");
        std::fs::create_dir_all(&content).expect("content 作成");
        let source = format!("---\ntitle: API\n---\n\n{body}");
        std::fs::write(content.join("index.md"), &source).expect("ページ書き込み");
        let pages = yuzu_core::build_source_pages(&content, &[], &MarkdownOptions::default())
            .expect("ページ読み込み");
        (dir, pages)
    }

    fn rules(diags: &[Diagnostic]) -> Vec<&str> {
        diags.iter().map(|d| d.rule).collect()
    }

    #[test]
    fn 正しい仕様は診断を出さない() {
        let (dir, pages) =
            project("```openapi\nopenapi: 3.0.3\ninfo:\n  title: X\npaths: {}\n```\n");
        let diags = validate_api_specs(&pages, dir.path(), &MarkdownOptions::default());
        assert!(diags.is_empty(), "{diags:?}");
    }

    #[test]
    fn 参照先のファイルが無ければ_spec_error() {
        let (dir, pages) = project("```openapi\nfile: specs/api.yaml\n```\n");
        let diags = validate_api_specs(&pages, dir.path(), &MarkdownOptions::default());
        assert_eq!(rules(&diags), vec!["spec-error"]);
        assert!(diags[0].message.contains("specs/api.yaml"));
        // 行番号が付く（ファイル単位の診断ではない）
        assert!(diags[0].span.is_some());
    }

    #[test]
    fn インラインの壊れた仕様も_spec_error() {
        let (dir, pages) = project("```openapi\ninfo:\n  title: [壊れ\n```\n");
        let diags = validate_api_specs(&pages, dir.path(), &MarkdownOptions::default());
        assert_eq!(rules(&diags), vec!["spec-error"]);
        assert!(diags[0].message.contains("パースに失敗"));
    }

    #[test]
    fn 未対応バージョンは_spec_error() {
        let (dir, pages) = project("```openapi\nopenapi: 9.9.9\npaths: {}\n```\n");
        let diags = validate_api_specs(&pages, dir.path(), &MarkdownOptions::default());
        assert_eq!(rules(&diags), vec!["spec-error"]);
        assert!(diags[0].message.contains("Swagger 2.0"));
    }

    #[test]
    fn jsonschema_に版の判定はない() {
        let (dir, pages) = project("```jsonschema\ntype: object\n```\n");
        let diags = validate_api_specs(&pages, dir.path(), &MarkdownOptions::default());
        assert!(diags.is_empty(), "{diags:?}");
    }

    #[test]
    fn 特別レンダリング言語以外は対象外() {
        let (dir, pages) = project("```rust\nfn main() {}\n```\n");
        let diags = validate_api_specs(&pages, dir.path(), &MarkdownOptions::default());
        assert!(diags.is_empty(), "{diags:?}");
    }

    #[test]
    fn 参照先の_ref_が読めなければ_spec_error() {
        let (dir, pages) = project("```openapi\nfile: specs/api.yaml\n```\n");
        let specs = dir.path().join("specs");
        std::fs::create_dir_all(&specs).expect("specs 作成");
        std::fs::write(
            specs.join("api.yaml"),
            "openapi: 3.0.3\ninfo:\n  title: X\npaths:\n  /x:\n    get:\n      responses:\n        \"200\":\n          content:\n            application/json:\n              schema:\n                $ref: \"./missing.yaml#/Foo\"\n",
        )
        .expect("仕様書き込み");
        let diags = validate_api_specs(&pages, dir.path(), &MarkdownOptions::default());
        assert_eq!(rules(&diags), vec!["spec-error"]);
        assert!(diags[0].message.contains("$ref"));
    }
}
