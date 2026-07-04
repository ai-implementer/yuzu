//! ビルドパイプライン: clean → ページ HTML → テーマアセット → syntect CSS →
//! public パススルー → build_id

use std::fs;

use minijinja::context;

use yuzu_config::ResolvedConfig;
use yuzu_core::{MarkdownOptions, SiteModel};

use crate::assets;
use crate::context::{NavCtx, PageCtx, SiteCtx};
use crate::css;
use crate::error::RenderError;
use crate::highlight::SyntectCodeRenderer;
use crate::templates;
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

/// レンダリング一式の入力
pub struct RenderParams<'a> {
    pub config: &'a ResolvedConfig,
    pub site: &'a SiteModel,
    pub live_reload: LiveReloadMode,
}

/// サイト全体を `dist/` に書き出す
pub fn render_site(params: &RenderParams) -> Result<(), RenderError> {
    let rc = params.config;
    let cfg = &rc.config;
    let output_dir = &rc.output_dir;

    if cfg.output.clean && output_dir.exists() {
        fs::remove_dir_all(output_dir).map_err(RenderError::io(output_dir))?;
    }
    fs::create_dir_all(output_dir).map_err(RenderError::io(output_dir))?;

    let env = templates::build_env(rc.theme_dir.as_deref())?;
    let template = env.get_template("page.jinja")?;
    let resolver = UrlResolver::new(&rc.base_url, params.site);
    let highlighter =
        SyntectCodeRenderer::new(cfg.markdown.highlight.enabled, cfg.markdown.mermaid.enabled);
    let md_opts = MarkdownOptions {
        gfm: cfg.markdown.gfm,
    };
    let site_ctx = SiteCtx {
        title: &cfg.site.title,
        description: cfg.site.description.as_deref(),
        lang: &cfg.site.lang,
    };

    for page in &params.site.pages {
        let body = yuzu_core::render_body_html(page, &md_opts, &highlighter, &resolver)?;
        let html = template.render(context! {
            site => site_ctx,
            page => PageCtx::new(page, &body, &resolver),
            nav => NavCtx::build(&params.site.nav, &page.route, &resolver),
            base_url => resolver.base(),
            asset_url => resolver.asset_url(),
            live_reload_poll => params.live_reload == LiveReloadMode::Poll,
            live_reload_ws => params.live_reload == LiveReloadMode::Ws,
            mermaid_enabled => cfg.markdown.mermaid.enabled,
            dark_enabled => cfg.theme.dark,
        })?;
        let out_path = output_dir.join(page.output_rel_path());
        assets::write_file(&out_path, html.as_bytes())?;
        tracing::debug!(page = %page.rel.display(), out = %out_path.display(), "ページ出力");
    }

    assets::write_theme_assets(output_dir, rc.theme_dir.as_deref())?;

    let syntect_css = css::generate_syntect_css(
        &cfg.markdown.highlight.theme_light,
        &cfg.markdown.highlight.theme_dark,
    )?;
    assets::write_file(
        &output_dir.join("_assets/css/syntect.css"),
        syntect_css.as_bytes(),
    )?;

    assets::copy_public(rc.public_dir.as_deref(), output_dir)?;
    assets::write_build_id(output_dir)?;

    tracing::info!(
        pages = params.site.pages.len(),
        out = %output_dir.display(),
        "ビルド完了"
    );
    Ok(())
}
