//! ビルドパイプライン: clean → ページ HTML（rayon 並列） → テーマアセット →
//! syntect CSS → public パススルー → build_id
//!
//! ページ生成はページ間に依存が無いため並列化している（Phase 32）。
//! 集約出力（nav は各ページに埋まるが構築は事前・llms / 404 / アセット）は
//! 直列のまま = インクリメンタルビルドの層構造は不変

use std::fs;

use minijinja::context;
use rayon::prelude::*;

use yuzu_config::ResolvedConfig;
use yuzu_core::{BuildCache, CachedBody, MarkdownOptions, OutputTracker, SiteModel};

use crate::assets;
use crate::context::{NavCtx, NavOrder, PageCtx, SiteCtx, build_breadcrumbs};
use crate::error::RenderError;
use crate::shared::RenderShared;
use crate::urls::UrlResolver;

/// ページに注入するライブリロード方式
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LiveReloadMode {
    /// 注入なし（通常ビルド / `dev.liveReload: false`）
    #[default]
    None,
    /// build_id ポーリング（`build --watch`）
    Poll,
    /// WebSocket（`yuzu dev`）
    Ws,
}

/// インクリメンタルビルドの文脈（すべて None = 従来のフルビルドと同一動作）
#[derive(Default)]
pub struct RenderCtx<'a> {
    /// ページ派生物のキャッシュ（本文 HTML・llms 正規化 md）
    pub cache: Option<&'a BuildCache>,
    /// 出力トラッキング（孤児掃除マニフェスト用）。Some なら dist の clean をしない
    pub outputs: Option<&'a OutputTracker>,
    /// セッション共有の重い状態（テンプレート Env・ハイライタ・syntect CSS）
    pub shared: Option<&'a RenderShared>,
}

/// レンダリング一式の入力
pub struct RenderParams<'a> {
    pub config: &'a ResolvedConfig,
    pub site: &'a SiteModel,
    pub live_reload: LiveReloadMode,
    pub ctx: RenderCtx<'a>,
    /// ページ（content 相対 `/` 区切り）→ 最終コミット日（YYYY-MM-DD）。
    /// git の実行は cli 層の責務で、render はデータを受け取るだけ。
    /// None = 最終更新日を表示しない
    pub git_dates: Option<&'a std::collections::HashMap<String, String>>,
}

/// サイト全体を `dist/` に書き出す
pub fn render_site(params: &RenderParams) -> Result<(), RenderError> {
    let rc = params.config;
    let cfg = &rc.config;
    let output_dir = &rc.output_dir;
    let ctx = &params.ctx;

    // インクリメンタル時（outputs あり）は dist を残し、孤児掃除は cli 側が行う
    if cfg.output.clean && ctx.outputs.is_none() && output_dir.exists() {
        fs::remove_dir_all(output_dir).map_err(RenderError::io(output_dir))?;
    }
    fs::create_dir_all(output_dir).map_err(RenderError::io(output_dir))?;

    // セッション共有があれば再利用、なければこのビルド限りで構築（従来コスト）
    let local_shared = match ctx.shared {
        Some(_) => None,
        None => Some(RenderShared::new(rc)?),
    };
    let shared = ctx.shared.unwrap_or_else(|| {
        local_shared
            .as_ref()
            .expect("shared 未指定時は local_shared を構築済み")
    });
    let template = shared.env.get_template("page.jinja")?;
    let resolver = UrlResolver::new(&rc.base_url, params.site);
    // 前/次リンクの導出元（サイドバー表示順のフラット列）。全ページで共通
    let nav_order = NavOrder::new(&params.site.nav);
    let md_opts = MarkdownOptions {
        gfm: cfg.markdown.gfm,
        math: cfg.markdown.math.enabled,
        mermaid: cfg.markdown.mermaid.enabled,
    };
    let site_ctx = SiteCtx {
        title: &cfg.site.title,
        description: cfg.site.description.as_deref(),
        lang: &cfg.site.lang,
        logo_url: cfg.site.logo.as_deref().map(|p| resolver.public_url(p)),
    };
    // theme.cssVars / cssVarsDark → head に注入する CSS 変数上書き（全ページ共通）
    let theme_css_vars =
        crate::css::generate_theme_var_overrides(&cfg.theme.css_vars, &cfg.theme.css_vars_dark);

    // ページ生成の並列ループ。共有物は読み取り専用（Env / syntect / resolver /
    // nav / 日付マップ）か内部 Mutex の `&self` API（BuildCache / OutputTracker）。
    // ページ内状態は PageCodeRenderer がページローカルに持つ（`Cell` が `!Sync`
    // なので誤って共有するとコンパイルエラーになる）。出力はページごとに別
    // ファイルで、マニフェスト記録は BTreeSet のため書き込み順に依らず決定的
    params
        .site
        .pages
        .par_iter()
        .try_for_each(|page| -> Result<(), RenderError> {
            // 本文 HTML はキャッシュヒットなら comrak パースごとスキップする
            let (body, mermaid_fallback) =
                match ctx.cache.and_then(|c| c.body(&page.rel, &page.source)) {
                    Some(cached) => (cached.html, cached.mermaid_fallback),
                    None => {
                        let renderer = shared.highlighter.page_renderer();
                        let body =
                            yuzu_core::render_body_html(page, &md_opts, &renderer, &resolver)?;
                        let fallback = renderer.mermaid_fallback_occurred();
                        // 外部ファイル参照（openapi/jsonschema の file:）を使ったページは
                        // キャッシュしない: ページ source が変わらなくても仕様ファイルの
                        // 変更を次ビルドで反映するため、毎回レンダリングする
                        let cacheable = !renderer.external_deps_used();
                        if let (Some(cache), true) = (ctx.cache, cacheable) {
                            cache.store_body(
                                &page.rel,
                                &page.source,
                                CachedBody {
                                    html: body.clone(),
                                    mermaid_fallback: fallback,
                                },
                            );
                        }
                        (body, fallback)
                    }
                };
            // 「このページで mermaid.js を読み込むか」。client は従来どおり常に読み、
            // ssr はフォールバック（未対応図種等）が発生したページだけ読む
            let mermaid_js_needed = cfg.markdown.mermaid.enabled
                && match cfg.markdown.mermaid.backend {
                    yuzu_config::MermaidBackend::Client => true,
                    yuzu_config::MermaidBackend::Ssr => mermaid_fallback,
                };
            // 「このページで KaTeX を読み込むか」。comrak の数式出力は必ず
            // data-math-style="…" 属性を持つ。本文テキスト中の同じ文字列は comrak が
            // `"` を &quot; にエスケープするため、引用符込みのこの判定は誤検出しない
            let math_needed = cfg.markdown.math.enabled && body.contains("data-math-style=\"");
            // git 連携メタ（rel を / 区切りへ正規化してキーにする）
            let rel_key = page
                .rel
                .iter()
                .map(|c| c.to_string_lossy())
                .collect::<Vec<_>>()
                .join("/");
            let last_updated = params
                .git_dates
                .and_then(|dates| dates.get(&rel_key))
                .cloned();
            let edit_url = cfg
                .git
                .edit_url
                .as_ref()
                .map(|tpl| tpl.replace("{path}", &rel_key));
            let html = template.render(context! {
                site => site_ctx,
                page => PageCtx::new(page, &body, &resolver, last_updated, edit_url),
                nav => NavCtx::build(&params.site.nav, &page.route, &resolver),
                pager => nav_order.pager(&page.route, &resolver),
                breadcrumbs => build_breadcrumbs(&params.site.nav, &page.route, &resolver),
                base_url => resolver.base(),
                asset_url => resolver.asset_url(),
                live_reload_poll => params.live_reload == LiveReloadMode::Poll,
                live_reload_ws => params.live_reload == LiveReloadMode::Ws,
                mermaid_enabled => mermaid_js_needed,
                math_enabled => math_needed,
                dark_enabled => cfg.theme.dark,
                search_enabled => cfg.search.enabled,
                theme_css_vars => theme_css_vars,
            })?;
            let out_rel = page.output_rel_path(); // route + "index.html"（/ 区切り）
            assets::write_output(ctx.outputs, output_dir, &out_rel, html.as_bytes())?;
            // ページ単位 Markdown（原文バイトそのまま）。コピーボタンと llms.txt の
            // .md リンクの実体。`yuzu fmt` 運用なら正規形と一致する
            assets::write_output(
                ctx.outputs,
                output_dir,
                &page.md_rel_path(),
                page.source.as_bytes(),
            )?;
            tracing::debug!(page = %page.rel.display(), out = %out_rel, "ページ出力");
            Ok(())
        })?;

    // 404 ページ（GitHub Pages 等の静的ホスティングは 404.html を自動で使う）。
    // copy_public より前に書く = public/404.html を置いたプロジェクトは
    // そちらが上書きして優先される（テーマ上書きと同じ思想）
    let not_found = shared.env.get_template("404.jinja")?.render(context! {
        site => site_ctx,
        page => context! {
            title => "ページが見つかりません",
            description => Option::<&str>::None,
            toc => Vec::<String>::new(),
        },
        // route "404.html" はどのページ route（"" か "…/" 終わり）とも一致しない
        // = サイドバーはハイライトなしで全ツリー表示
        nav => NavCtx::build(&params.site.nav, "404.html", &resolver),
        base_url => resolver.base(),
        asset_url => resolver.asset_url(),
        live_reload_poll => params.live_reload == LiveReloadMode::Poll,
        live_reload_ws => params.live_reload == LiveReloadMode::Ws,
        mermaid_enabled => false,
        math_enabled => false,
        dark_enabled => cfg.theme.dark,
        search_enabled => cfg.search.enabled,
        theme_css_vars => theme_css_vars,
    })?;
    assets::write_output(ctx.outputs, output_dir, "404.html", not_found.as_bytes())?;

    assets::write_theme_assets(output_dir, rc.theme_dir.as_deref(), ctx.outputs)?;
    assets::write_output(
        ctx.outputs,
        output_dir,
        "_assets/css/syntect.css",
        shared.syntect_css.as_bytes(),
    )?;

    // llms.txt / llms-full.txt（copy_public より前 = ユーザが public/llms.txt を
    // 置いた場合はそちらが上書きして優先される。テーマ上書きと同じ思想）
    if cfg.llms.enabled {
        crate::llms::write_llms_files(rc, params.site, output_dir, ctx)?;
    }

    // content の同伴アセット（ページ横の画像等）。copy_public より前 = 同じパスに
    // public/ のファイルがあればそちらが上書きして優先される（テーマ上書きと同じ思想）
    assets::copy_content_assets(&rc.content_dir, &cfg.input.ignore, output_dir, ctx.outputs)?;
    assets::copy_public(rc.public_dir.as_deref(), output_dir, ctx.outputs)?;
    assets::write_build_id(output_dir, ctx.outputs)?;

    tracing::info!(
        pages = params.site.pages.len(),
        out = %output_dir.display(),
        "ビルド完了"
    );
    Ok(())
}
