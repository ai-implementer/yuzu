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
    let mut pages = load_pages(content_dir, ignore, opts)?;
    pages.retain(|page| {
        if page.frontmatter.draft {
            tracing::debug!(path = %page.rel.display(), "draft のため除外");
        }
        !page.frontmatter.draft
    });
    let nav = nav::build_nav(&pages);
    Ok(SiteModel { pages, nav })
}

/// `content_dir` 以下の全ページを列挙する（`yuzu fmt` / `lint` / `check` 用）。
///
/// [`build_site_model`] と違い **`draft: true` も除外しない**（リポジトリ内の
/// ソースは公開前でも規約対象にする）。ナビは構築しない。
/// ignore glob の扱いと走査順（パスのソート順）は [`build_site_model`] と同じ
pub fn build_source_pages(
    content_dir: &Path,
    ignore: &[String],
    opts: &MarkdownOptions,
) -> Result<Vec<Page>, CoreError> {
    load_pages(content_dir, ignore, opts)
}

/// 走査＋メタ抽出の共通部（draft を含む全ページ）
fn load_pages(
    content_dir: &Path,
    ignore: &[String],
    opts: &MarkdownOptions,
) -> Result<Vec<Page>, CoreError> {
    let files = scan::scan_markdown_files(content_dir, ignore)?;
    let mut pages = Vec::new();

    for file in files {
        let source = fs::read_to_string(&file.abs).map_err(|source| CoreError::Io {
            path: file.abs.clone(),
            source,
        })?;
        let meta = markdown::extract_meta(&source, opts, &file.abs)?;

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
    Ok(pages)
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

/// ページ本文のプレーンテキストを抽出する（検索インデックス用）。
/// frontmatter・生 HTML・フェンスコードブロックは含めない
/// （インラインコードは API 名検索のため含める）
pub fn extract_plain_text(page: &Page, opts: &MarkdownOptions) -> Result<String, CoreError> {
    markdown::extract_plain_text(&page.source, opts)
}

/// ページ本文を正規化 Markdown として出力する（frontmatter は含めない）。
/// llms-full.txt の基盤（全文が要る場合は [`format_document`] を使う）
pub fn normalize_markdown(page: &Page, opts: &MarkdownOptions) -> Result<String, CoreError> {
    markdown::normalize_markdown(&page.source, opts)
}

/// ページ全文（frontmatter 込み）を整形した Markdown を返す（`yuzu fmt` 用）。
///
/// - 本文は [`normalize_markdown`] と同じ正規形（見出し ATX 化・箇条書き `-` 統一等）
/// - frontmatter は YAML を再シリアライズせずバイト温存で再結合する
/// - 冪等: `format_document` の出力を再整形しても変化しない
pub fn format_document(page: &Page, opts: &MarkdownOptions) -> Result<String, CoreError> {
    markdown::format_document(&page.source, opts)
}
