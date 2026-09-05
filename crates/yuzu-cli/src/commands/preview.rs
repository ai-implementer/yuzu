//! `yuzu preview [--port]`: dist/ の配信

use std::net::IpAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, bail};

use yuzu_config::ResolvedConfig;
use yuzu_server::{PathGuard, ServeOptions};

pub fn run(port: Option<u16>, host: Option<String>) -> anyhow::Result<()> {
    let (_, mut rc) = super::load_project()?;
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
        path_guard: Some(symlink_guard(&rc.output_dir)),
    })?;
    Ok(())
}

/// 配信パスの検査述語（`preview` / `dev` / `build --watch` で共用）。
/// 出力ディレクトリ配下のシンボリックリンクは、書き側（`write_under` / 孤児掃除）が
/// 拒否するのと同じ規律で読み側も辿らない（server は core を知らないので cli が包む）
pub(crate) fn symlink_guard(dir: &Path) -> PathGuard {
    let root: PathBuf = dir.to_path_buf();
    Arc::new(move |path: &Path| {
        yuzu_core::output::ensure_symlink_free(&root, path).map_err(|e| e.to_string())
    })
}
