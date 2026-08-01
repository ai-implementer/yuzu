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

mod aliases;
pub mod cache;
mod diagnostics;
mod error;
mod frontmatter;
mod include;
mod linkcheck;
mod lint;
mod markdown;
mod model;
mod nav;
pub mod output;
mod routes;
mod scan;
mod traits;
pub mod urlpath;

use std::fs;
use std::path::{Path, PathBuf};

pub use aliases::{alias_routes, validate_aliases};
pub use cache::{BuildCache, CacheStats, CachedBody, CachedMeta, CachedSection};
pub use diagnostics::{DiagBase, Diagnostic, Severity};
pub use error::CoreError;
pub use include::{
    IncludeRef, SpecRefError, SpecSource, collect_include_specs, resolve_include,
    resolve_spec_file, resolve_spec_source, validate_includes, validate_spec_refs,
};
pub use markdown::crossref::CaptionKind;
pub use markdown::fence::{CodeBlockMeta, IncludeSpec};
pub use markdown::fragment::FRAGMENT_LANG;
pub use markdown::{FenceBlock, RenderedBody, extract_fence_blocks};
pub use model::{
    CrossrefLabel, Frontmatter, NavNode, Page, PlainSection, SiteModel, SourceSpan, TocEntry,
};
pub use nav::{NavGroup, nav_groups, route_group_key};
pub use output::{OutputTracker, WriteOutcome};
pub use routes::validate_routes;
pub use scan::IgnoreMatcher;
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
    /// 図表番号をサイト全体の通し番号にするか（`markdown.crossref.numbering`）。
    /// true なら [`build_site_model`] がサイドバー表示順でオフセットを割り当てる
    pub crossref_site_numbering: bool,
    /// 用語集・略語（`markdown.glossary`）。既定（空辞書）なら何も起きない
    pub glossary: GlossaryOptions,
}

impl Default for MarkdownOptions {
    fn default() -> Self {
        Self {
            gfm: true,
            math: true,
            mermaid: true,
            crossref_site_numbering: false,
            glossary: GlossaryOptions::default(),
        }
    }
}

/// 用語集・略語の挙動設定（設定ファイルの `markdown.glossary` から写す）。
///
/// [`LintOptions`] と同じく yuzu-config 非依存の中立型で、cli が設定から写す。
/// 既定は「辞書が空 = 何も起きない」で、`page` / `page_title` の既定文字列は
/// yuzu-config 側の `GlossaryConfig::default()` が唯一の持ち主
/// （2 箇所に既定値を書くと片方だけ変わる）
#[derive(Debug, Clone)]
pub struct GlossaryOptions {
    /// 用語辞書（略語 → 説明文）
    pub terms: std::collections::BTreeMap<String, String>,
    /// 本文中の初出を `<abbr title="説明">略語</abbr>` にするか
    pub abbr: bool,
    /// 用語集ページの route 元（`content` 相対・拡張子なし）。空ならページを作らない
    pub page: String,
    /// 用語集ページのタイトル。空ならページ名から補う
    pub page_title: String,
}

impl Default for GlossaryOptions {
    fn default() -> Self {
        Self {
            terms: std::collections::BTreeMap::new(),
            abbr: true,
            page: String::new(),
            page_title: String::new(),
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
        // openapi / jsonschema は設定で無効化できないので常に true
        _ => is_spec_lang(lang),
    }
}

/// API 仕様ブロックのフェンス言語（本文に `file: <パス>` 参照を書ける言語）。
/// **ここが唯一の定義**で、[`is_special_render_lang`] と [`validate_spec_refs`]
/// の両方がこれを見る。yuzu-render の `SpecKind` への写像との一致は
/// speccheck のユニットテストが縛る
pub const SPEC_LANGS: &[&str] = &["openapi", "jsonschema"];

/// フェンス言語が API 仕様ブロックか
pub fn is_spec_lang(lang: &str) -> bool {
    SPEC_LANGS.contains(&lang)
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
    // 用語集ページは nav 構築より前に混ぜる（サイドバー・パンくず・pager・
    // 通し番号の順序決めがすべて pages を入力にしているため）
    if let Some(page) = glossary_page(content_dir, opts)? {
        pages.push(page);
    }
    let nav = nav::build_nav(&pages);
    if opts.crossref_site_numbering {
        assign_crossref_offsets(&mut pages, &nav);
    }
    Ok(SiteModel { pages, nav })
}

/// サイト全体の通し番号（`markdown.crossref.numbering: "site"`）用に、
/// 各ページの採番開始オフセットを**サイドバー表示順**で割り当てる。
///
/// ページ本文 HTML はキャッシュされるため、オフセットが変わると古い番号が
/// 残る。cli は routesKey にラベル個数を含めて全 body を無効化すること
fn assign_crossref_offsets(pages: &mut [Page], nav: &[NavNode]) {
    // nav（表示順）のフラットな route 列 → ページの並び替え順を作る
    let mut order: Vec<&str> = Vec::new();
    fn walk<'a>(nodes: &'a [NavNode], out: &mut Vec<&'a str>) {
        for node in nodes {
            if let Some(route) = &node.route {
                out.push(route);
            }
            walk(&node.children, out);
        }
    }
    walk(nav, &mut order);

    let rank: std::collections::HashMap<&str, usize> = order
        .iter()
        .enumerate()
        .map(|(i, route)| (*route, i))
        .collect();
    // 表示順（nav に出ないページは末尾へ。パス順で安定させる）
    let mut indices: Vec<usize> = (0..pages.len()).collect();
    indices.sort_by_key(|&i| {
        (
            rank.get(pages[i].route.as_str())
                .copied()
                .unwrap_or(usize::MAX),
            i,
        )
    });

    let mut running = markdown::crossref::Numbering::default();
    for i in indices {
        pages[i].crossref_offset = running;
        // ページ内番号（1 始まり）を通し番号へ読み替える。
        // 本文 HTML 側も同じオフセットから採番するので両者は一致する
        let mut counts = markdown::crossref::Numbering::default();
        for label in &mut pages[i].labels {
            counts.next(label.kind);
            label.number += running.get(label.kind);
        }
        running.add(&counts);
    }
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
    let mut pages = load_pages(content_dir, ignore, opts)?;
    // 用語集ページはソースが無いので fmt / lint の対象にはならないが、
    // **リンク検査の有効ターゲット**（`[用語集](../glossary.md#api)`）と
    // route 衝突検査には要るのでここでも混ぜる。実際の除外は
    // `generated` を見る各呼び出し側の責務
    if let Some(page) = glossary_page(content_dir, opts)? {
        pages.push(page);
    }
    Ok(pages)
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
        let (frontmatter, title, toc, labels) = match cached {
            Some(meta) => (meta.frontmatter, meta.title, meta.toc, meta.labels),
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
                            labels: meta.labels.clone(),
                        },
                    );
                }
                (meta.frontmatter, title, meta.toc, meta.labels)
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
            labels,
            crossref_offset: Default::default(),
            source,
            generated: false,
        });
    }
    Ok(pages)
}

/// 用語集ページ（`markdown.glossary`）を合成する。辞書が空 / route が空なら `None`。
///
/// **`Page` として作って `pages` に混ぜる**のが要点で、こうすると nav・パンくず・
/// pager・sitemap・llms.txt・検索索引・route 衝突検査・routesKey がすべて既存経路の
/// ままで効く（HTML を単発で書き出す 404.html 方式ではこれらを個別に配線し直すことになる）。
/// メタは通常ページと同じ [`markdown::extract_meta`] から取るので、
/// 見出しアンカーの採番が本文 HTML と自動的に一致する
fn glossary_page(content_dir: &Path, opts: &MarkdownOptions) -> Result<Option<Page>, CoreError> {
    let Some(rel) = markdown::glossary::page_rel(&opts.glossary) else {
        return Ok(None);
    };
    let heading = match opts.glossary.page_title.trim() {
        "" => scan::stem_title(&rel),
        title => title.to_string(),
    };
    let source = markdown::glossary::page_markdown(&heading, &opts.glossary.terms);
    let src = content_dir.join(&rel);
    let meta = markdown::extract_meta(&source, opts, &src)?;
    let title = meta
        .frontmatter
        .title
        .clone()
        .or(meta.first_h1)
        .unwrap_or(heading);
    let route = scan::route_for_rel(&rel);
    Ok(Some(Page {
        src,
        rel,
        route,
        frontmatter: meta.frontmatter,
        title,
        toc: meta.toc,
        labels: meta.labels,
        crossref_offset: Default::default(),
        source,
        generated: true,
    }))
}

/// パス2: ページ本文を HTML 化する。
///
/// - コードブロックは [`CodeBlockRenderer`] に通し、`Some(html)` が返れば
///   その HTML で丸ごと差し替える（syntect ハイライトや `<pre class="mermaid">` 化）
/// - リンク・画像の URL は [`UrlRewriter`] に通す（base path 解決・`.md` リンク解決）
/// - ` ```include file="..." ` の Markdown 断片は本文へ展開する。`root` は
///   その基準ディレクトリ（`None` なら断片はエラーボックスになる = 単体テスト用）
pub fn render_body_html(
    page: &Page,
    opts: &MarkdownOptions,
    code: &dyn CodeBlockRenderer,
    urls: &dyn UrlRewriter,
    root: Option<&Path>,
) -> Result<RenderedBody, CoreError> {
    markdown::render_body_html(page, opts, code, urls, root)
}

/// ページ本文を h2/h3 見出し境界で分割したプレーンテキストセクションを返す（検索用）。
/// 先頭要素はリード文（anchor/heading = None）。h4〜h6 は直近セクションに併合される。
/// `index_code = true`（`search.indexCode`）でフェンスコードブロックの本文も含める
/// （インデントコードブロックと、特別レンダリングされる言語
/// [`is_special_render_lang`] は除く）
/// `root` を渡すと `file=` のコンテンツインクルードを展開して索引する
/// （`None` なら展開しない = ライブラリ単体テスト・従来動作）
pub fn extract_plain_sections(
    page: &Page,
    opts: &MarkdownOptions,
    index_code: bool,
    root: Option<&Path>,
) -> Result<Vec<PlainSection>, CoreError> {
    markdown::extract_plain_sections(&page.source, opts, index_code, root)
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
