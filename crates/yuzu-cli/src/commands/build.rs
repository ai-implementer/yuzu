//! `yuzu build [--watch]`: サイトのビルド（と監視・配信）

use std::time::Duration;

use anyhow::Context;

use yuzu_config::ResolvedConfig;
use yuzu_core::MarkdownOptions;
use yuzu_render::{LiveReloadMode, RenderParams};

use crate::commands::preview;

/// エディタの連続保存をまとめる debounce 幅（build --watch / dev 共通）
pub(crate) const DEBOUNCE: Duration = Duration::from_millis(300);

pub fn run(watch: bool) -> anyhow::Result<()> {
    let cwd = std::env::current_dir().context("カレントディレクトリを取得できません")?;
    let root = yuzu_config::find_project_root(&cwd)?;
    let rc = yuzu_config::load(&root)?;
    yuzu_config::write_resolved(&rc)?;

    // --watch のときだけオートリフレッシュ JS（ポーリング式）を注入する
    let mode = if watch {
        LiveReloadMode::Poll
    } else {
        LiveReloadMode::None
    };
    build_once(&rc, mode)?;

    if !watch {
        return Ok(());
    }

    // 監視対象は content/ と theme/ のみ（dist/ を見ると無限ループ）。
    // 設定は起動時のもので固定（yuzu.jsonc の変更は再起動で反映）
    let mut paths = vec![rc.content_dir.clone()];
    if let Some(theme_dir) = &rc.theme_dir {
        paths.push(theme_dir.clone());
    }
    let rc_for_watch = rc.clone();
    let _watch_handle = yuzu_server::watch(&paths, DEBOUNCE, move || {
        tracing::info!("変更を検知 → 再ビルド");
        if let Err(e) = build_once(&rc_for_watch, LiveReloadMode::Poll) {
            // 執筆中の一時的な構文エラー等でプロセスは落とさない
            tracing::error!("再ビルドに失敗しました: {e:#}");
        }
    })?;

    // 受け入れ条件「編集 → ブラウザ自動更新」を 1 コマンドで満たすため、
    // preview と同じ静的サーバも起動する（ブロッキング）
    preview::serve_dist(&rc, None)
}

pub(crate) fn build_once(rc: &ResolvedConfig, live_reload: LiveReloadMode) -> anyhow::Result<()> {
    let site = yuzu_core::build_site_model(
        &rc.content_dir,
        &rc.config.input.ignore,
        &MarkdownOptions {
            gfm: rc.config.markdown.gfm,
        },
    )?;
    yuzu_render::render_site(&RenderParams {
        config: rc,
        site: &site,
        live_reload,
    })?;
    Ok(())
}
