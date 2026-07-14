//! `yuzu preview [--port]`: dist/ の配信

use std::net::IpAddr;

use anyhow::{Context, bail};

use yuzu_config::ResolvedConfig;
use yuzu_server::ServeOptions;

pub fn run(port: Option<u16>, host: Option<String>) -> anyhow::Result<()> {
    let cwd = std::env::current_dir().context("カレントディレクトリを取得できません")?;
    let root = yuzu_config::find_project_root(&cwd)?;
    let mut rc = yuzu_config::load(&root)?;
    // --host は dev.host の設定より優先（コンテナ内から 0.0.0.0 で配信する用途）
    if let Some(host) = host {
        rc.config.dev.host = host;
    }

    if !rc.output_dir.is_dir() {
        bail!(
            "{} がありません。先に `yuzu build` を実行してください",
            rc.output_dir.display()
        );
    }
    serve_dist(&rc, port)
}

/// dist/ を配信する（`preview` と `build --watch` で共用。ブロッキング）
pub(crate) fn serve_dist(rc: &ResolvedConfig, port: Option<u16>) -> anyhow::Result<()> {
    let host: IpAddr = rc
        .config
        .dev
        .host
        .parse()
        .with_context(|| format!("dev.host が不正です: {}", rc.config.dev.host))?;

    yuzu_server::serve(ServeOptions {
        dir: rc.output_dir.clone(),
        host,
        port: port.unwrap_or(rc.config.dev.port),
        base_url: rc.base_url.clone(),
        live_reload: None,
    })?;
    Ok(())
}
