//! yuzu のコア: Markdown → ドキュメントモデル → サイトモデル構築。
//!
//! Markdown パーサ（comrak）はこの crate の内部（`markdown` モジュール）に
//! 完全に隠蔽する。公開 API はパーサ非依存の自前モデル
//! （[`Page`] / [`SiteModel`] / [`NavNode`] / [`TocEntry`]）と、
//! render 側が差し込むフック trait（[`CodeBlockRenderer`] / [`UrlRewriter`]）のみ。
//!
//! 処理は 2 パス構成:
//! 1. [`build_site_model`] — 走査＋メタ抽出（frontmatter / タイトル / TOC）、
//!    `draft: true` の除外、ナビツリー構築
//! 2. [`render_body_html`] — 本文の HTML 化（コードブロック差し替え・
//!    リンク書き換えのフックを通す）

mod error;
mod frontmatter;
mod markdown;
mod model;
mod nav;
mod scan;
mod traits;

use std::fs;
use std::path::Path;

pub use error::CoreError;
pub use model::{Frontmatter, NavNode, Page, SiteModel, SourceSpan, TocEntry};
pub use traits::{CodeBlockRenderer, NoopCodeBlockRenderer, NoopUrlRewriter, UrlRewriter};

/// Markdown パースの挙動設定（設定ファイルの `markdown` セクションから写す）
#[derive(Debug, Clone)]
pub struct MarkdownOptions {
    /// GFM 拡張（表・打ち消し線・autolink・タスクリスト）を有効にするか
    pub gfm: bool,
}

impl Default for MarkdownOptions {
    fn default() -> Self {
        Self { gfm: true }
    }
}

/// パス1: `content_dir` 以下の `*.md` を走査し、サイトモデルを構築する。
///
/// - `ignore` は `content_dir` からの相対パスに対する glob（例: `**/_drafts/**`）
/// - frontmatter `draft: true` のページは除外する
/// - ナビはディレクトリ階層から自動生成し、frontmatter `title` / `order` を反映する
pub fn build_site_model(
    content_dir: &Path,
    ignore: &[String],
    opts: &MarkdownOptions,
) -> Result<SiteModel, CoreError> {
    let files = scan::scan_markdown_files(content_dir, ignore)?;
    let mut pages = Vec::new();

    for file in files {
        let source = fs::read_to_string(&file.abs).map_err(|source| CoreError::Io {
            path: file.abs.clone(),
            source,
        })?;
        let meta = markdown::extract_meta(&source, opts, &file.abs)?;

        if meta.frontmatter.draft {
            tracing::debug!(path = %file.rel.display(), "draft のため除外");
            continue;
        }

        let route = scan::route_for_rel(&file.rel);
        let title = meta
            .frontmatter
            .title
            .clone()
            .or(meta.first_h1)
            .unwrap_or_else(|| scan::stem_title(&file.rel));

        pages.push(Page {
            src: file.abs,
            rel: file.rel,
            route,
            frontmatter: meta.frontmatter,
            title,
            toc: meta.toc,
            source,
        });
    }

    let nav = nav::build_nav(&pages);
    Ok(SiteModel { pages, nav })
}

/// パス2: ページ本文を HTML 化する。
///
/// - コードブロックは [`CodeBlockRenderer`] に通し、`Some(html)` が返れば
///   その HTML で丸ごと差し替える（syntect ハイライトや `<pre class="mermaid">` 化）
/// - リンク・画像の URL は [`UrlRewriter`] に通す（base path 解決・`.md` リンク解決）
pub fn render_body_html(
    page: &Page,
    opts: &MarkdownOptions,
    code: &dyn CodeBlockRenderer,
    urls: &dyn UrlRewriter,
) -> Result<String, CoreError> {
    markdown::render_body_html(page, opts, code, urls)
}
