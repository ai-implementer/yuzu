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

pub mod cache;
mod diagnostics;
mod error;
mod frontmatter;
mod linkcheck;
mod lint;
mod markdown;
mod model;
mod nav;
pub mod output;
mod scan;
mod traits;
pub mod urlpath;

use std::fs;
use std::path::{Path, PathBuf};

pub use cache::{BuildCache, CacheStats, CachedBody, CachedMeta, CachedSection};
pub use diagnostics::{Diagnostic, Severity};
pub use error::CoreError;
pub use model::{Frontmatter, NavNode, Page, PlainSection, SiteModel, SourceSpan, TocEntry};
pub use output::{OutputTracker, WriteOutcome};
pub use traits::{CodeBlockRenderer, NoopCodeBlockRenderer, NoopUrlRewriter, UrlRewriter};

/// Markdown パースの挙動設定（設定ファイルの `markdown` セクションから写す）
#[derive(Debug, Clone)]
pub struct MarkdownOptions {
    /// GFM 拡張（表・打ち消し線・autolink・タスクリスト・alerts・脚注）を有効にするか
    pub gfm: bool,
    /// 数式拡張（`$...$` / `$$...$$` / `` $`...`$ ``）を有効にするか。gfm とは独立
    pub math: bool,
    /// mermaid コードブロックの描画（`markdown.mermaid.enabled`）が有効か。
    /// パースには影響しないが、検索抽出の特別レンダリング判定
    /// （[`is_special_render_lang`]）が参照する
    pub mermaid: bool,
}

impl Default for MarkdownOptions {
    fn default() -> Self {
        Self {
            gfm: true,
            math: true,
            mermaid: true,
        }
    }
}

/// フェンス言語がビルド時に特別レンダリングされるか（＝コードブロックとして表示されない）。
/// 検索インデックスのコード除外（`search.indexCode` 有効時）はこの述語が唯一の判定。
/// yuzu-render 側のディスパッチ（highlight.rs の `render`）と集合を同期させること。
/// mermaid / math は設定で無効化するとプレーンコード表示になるため対象から外れる
/// （ページに見えるテキストは索引される）。openapi / jsonschema は常に特別レンダリング
pub fn is_special_render_lang(lang: &str, opts: &MarkdownOptions) -> bool {
    match lang {
        "mermaid" => opts.mermaid,
        "math" => opts.math,
        "openapi" | "jsonschema" => true,
        _ => false,
    }
}

/// 文書規約 lint の挙動設定（設定ファイルの `lint` セクションから写す）
#[derive(Debug, Clone, Default)]
pub struct LintOptions {
    /// content 配下で許容するディレクトリ階層の最大深さ
    /// （直下 = 0。例: 1 なら `guide/x.md` まで）。`None` なら無制限（チェックしない）
    pub max_directory_depth: Option<u32>,
    /// 用語統一の辞書（正しい表記 → ゆれ表記のリスト）。
    /// 本文テキスト（コード・URL を除く）にゆれ表記が現れたら警告する
    pub terms: std::collections::BTreeMap<String, Vec<String>>,
    /// 組み込みの表記ゆれルール（設定の `lint.rules` から写す）
    pub rules: LintRules,
}

/// 組み込み表記ゆれルールの有効/無効（既定はすべて有効）
#[derive(Debug, Clone)]
pub struct LintRules {
    /// 全角英数字（Ｗｅｂ１２３）
    pub fullwidth_alphanumeric: bool,
    /// 半角カナ（ｶﾀｶﾅ）
    pub halfwidth_kana: bool,
    /// 長音符ゆれの混在（サーバ/サーバー。プロジェクト横断）
    pub katakana_choon: bool,
}

impl Default for LintRules {
    fn default() -> Self {
        Self {
            fullwidth_alphanumeric: true,
            halfwidth_kana: true,
            katakana_choon: true,
        }
    }
}

/// `content_dir` 以下の `.md` 以外の同伴アセット（ページ横の画像等）を列挙する。
/// 戻り値は（絶対パス, content 相対パス）のソート順。
/// `ignore` glob は [`build_site_model`] と同一の評価で、隠しファイルは除外する
pub fn collect_content_assets(
    content_dir: &Path,
    ignore: &[String],
) -> Result<Vec<(PathBuf, PathBuf)>, CoreError> {
    Ok(scan::scan_content_assets(content_dir, ignore)?
        .into_iter()
        .map(|f| (f.abs, f.rel))
        .collect())
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
    build_site_model_cached(content_dir, ignore, opts, None, false)
}

/// [`build_site_model`] のキャッシュ対応版。
/// cache があれば未変更ページのメタ抽出（comrak パース）をスキップする。
/// `include_drafts` はプレビュー用途（`--drafts`）で draft ページも含める
pub fn build_site_model_cached(
    content_dir: &Path,
    ignore: &[String],
    opts: &MarkdownOptions,
    cache: Option<&BuildCache>,
    include_drafts: bool,
) -> Result<SiteModel, CoreError> {
    let mut pages = load_pages_cached(content_dir, ignore, opts, cache)?;
    if !include_drafts {
        pages.retain(|page| {
            if page.frontmatter.draft {
                tracing::debug!(path = %page.rel.display(), "draft のため除外");
            }
            !page.frontmatter.draft
        });
    }
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
    load_pages_cached(content_dir, ignore, opts, None)
}

fn load_pages_cached(
    content_dir: &Path,
    ignore: &[String],
    opts: &MarkdownOptions,
    cache: Option<&BuildCache>,
) -> Result<Vec<Page>, CoreError> {
    let files = scan::scan_markdown_files(content_dir, ignore)?;
    let mut pages = Vec::new();

    for file in files {
        let source = fs::read_to_string(&file.abs).map_err(|source| CoreError::Io {
            path: file.abs.clone(),
            source,
        })?;
        let rel_key = file
            .rel
            .iter()
            .map(|c| c.to_string_lossy())
            .collect::<Vec<_>>()
            .join("/");
        let source_hash = cache.map(|_| BuildCache::source_hash(&source));

        // キャッシュヒットならメタ抽出（comrak パース）をスキップ
        let cached = cache
            .zip(source_hash.as_deref())
            .and_then(|(c, h)| c.meta(&rel_key, h));
        let (frontmatter, title, toc) = match cached {
            Some(meta) => (meta.frontmatter, meta.title, meta.toc),
            None => {
                let meta = markdown::extract_meta(&source, opts, &file.abs)?;
                let title = meta
                    .frontmatter
                    .title
                    .clone()
                    .or(meta.first_h1)
                    .unwrap_or_else(|| scan::stem_title(&file.rel));
                if let Some((c, h)) = cache.zip(source_hash.as_deref()) {
                    c.store_meta(
                        &rel_key,
                        h,
                        cache::CachedMeta {
                            frontmatter: meta.frontmatter.clone(),
                            title: title.clone(),
                            toc: meta.toc.clone(),
                        },
                    );
                }
                (meta.frontmatter, title, meta.toc)
            }
        };

        let route = scan::route_for_rel(&file.rel);
        pages.push(Page {
            src: file.abs,
            rel: file.rel,
            route,
            frontmatter,
            title,
            toc,
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

/// ページ本文を h2/h3 見出し境界で分割したプレーンテキストセクションを返す（検索用）。
/// 先頭要素はリード文（anchor/heading = None）。h4〜h6 は直近セクションに併合される。
/// `index_code = true`（`search.indexCode`）でフェンスコードブロックの本文も含める
/// （インデントコードブロックと、特別レンダリングされる言語
/// [`is_special_render_lang`] は除く）
pub fn extract_plain_sections(
    page: &Page,
    opts: &MarkdownOptions,
    index_code: bool,
) -> Result<Vec<PlainSection>, CoreError> {
    markdown::extract_plain_sections(&page.source, opts, index_code)
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

/// 文書規約の診断（`yuzu lint` / `yuzu check` 用）。
///
/// ルール: `duplicate-h1`（本文 h1 の重複）/ `heading-level-skip`
/// （見出しレベルの飛び）/ `frontmatter-unknown-key`（未知キー）/
/// `directory-too-deep`（ディレクトリ階層の深さ超過。
/// [`LintOptions::max_directory_depth`] 設定時のみ）。
/// 診断は行順でソート済み
pub fn lint_page(
    page: &Page,
    opts: &MarkdownOptions,
    lint: &LintOptions,
) -> Result<Vec<Diagnostic>, CoreError> {
    lint::lint_page(page, opts, lint)
}

/// プロジェクト横断の文書規約 lint（ページ間の整合を見るルール）。
/// 現在は `katakana-choon`（長音符ゆれの混在）のみ。
/// [`lint_page`] の後に呼んで診断を合流させる。診断は (rel, 行, 列) 順でソート済み
pub fn lint_project(
    pages: &[Page],
    opts: &MarkdownOptions,
    lint: &LintOptions,
) -> Result<Vec<Diagnostic>, CoreError> {
    lint::lint_project(pages, opts, lint)
}

/// [`Diagnostic::fix`] を持つ診断（表記ゆれ系）をソースへ適用する
/// （`yuzu lint --fix` 用）。範囲が交差する fix は先勝ちでスキップするため、
/// 適用後に再 lint → 再適用の繰り返しで不動点に到達させる想定。
/// 戻り値は (適用後ソース, 適用件数)
pub fn apply_fixes(source: &str, diags: &[Diagnostic]) -> (String, usize) {
    lint::apply_fixes(source, diags)
}

/// 内部リンク・アンカーの静的検査（`yuzu check` 用）。
///
/// - `pages` には draft 込みの全ページ（[`build_source_pages`]）を渡す。
///   リンクの**有効ターゲットは非 draft ページのみ**（ビルド成果物に実在するもの）
/// - 外部 URL（スキーム付き）はネットワークに触れず検査しない
/// - アンカーは本文 HTML と同一採番の見出し id で照合する
pub fn check_links(
    pages: &[Page],
    public_dir: Option<&Path>,
    content_dir: &Path,
    opts: &MarkdownOptions,
) -> Result<Vec<Diagnostic>, CoreError> {
    linkcheck::check_links(pages, public_dir, content_dir, opts)
}
