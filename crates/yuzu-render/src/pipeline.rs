//! ビルドパイプライン: clean → ページ HTML → テーマアセット → syntect CSS →
//! public パススルー → build_id

use std::fs;

use minijinja::context;

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

    for page in &params.site.pages {
        // 本文 HTML はキャッシュヒットなら comrak パースごとスキップする
        let (body, mermaid_fallback) = match ctx.cache.and_then(|c| c.body(&page.rel, &page.source))
        {
            Some(cached) => (cached.html, cached.mermaid_fallback),
            None => {
                shared.highlighter.begin_page();
                let body =
                    yuzu_core::render_body_html(page, &md_opts, &shared.highlighter, &resolver)?;
                let fallback = shared.highlighter.mermaid_fallback_occurred();
                if let Some(cache) = ctx.cache {
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
        let html = template.render(context! {
            site => site_ctx,
            page => PageCtx::new(page, &body, &resolver),
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
    }

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

    assets::copy_public(rc.public_dir.as_deref(), output_dir, ctx.outputs)?;
    assets::write_build_id(output_dir, ctx.outputs)?;

    tracing::info!(
        pages = params.site.pages.len(),
        out = %output_dir.display(),
        "ビルド完了"
    );
    Ok(())
}
