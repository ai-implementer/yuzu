//! `yuzu dev`: 監視ビルド＋配信＋WebSocket ライブリロード（Phase 2）

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::Duration;

use anyhow::Context;

use yuzu_config::ResolvedConfig;
use yuzu_render::LiveReloadMode;
use yuzu_server::{ReloadNotifier, ServeOptions};

use crate::commands::build;

pub fn run(port: Option<u16>, force: bool, drafts: bool) -> anyhow::Result<()> {
    let cwd = std::env::current_dir().context("カレントディレクトリを取得できません")?;
    let root = yuzu_config::find_project_root(&cwd)?;
    let rc = yuzu_config::load(&root)?;
    yuzu_config::write_resolved(&rc)?;

    // dev.liveReload=false は「WS 注入なしの監視ビルド＋配信のみ」
    let mode = if rc.config.dev.live_reload {
        LiveReloadMode::Ws
    } else {
        LiveReloadMode::None
    };
    let mut session = build::BuildSession::new(&rc, force)?;
    build::build_once(&rc, mode, &mut session, drafts)?;

    let notifier = rc.config.dev.live_reload.then(ReloadNotifier::new);

    // 監視対象は content/ と theme/ のみ（dist/ を見ると無限ループ）。
    // 設定は起動時のもので固定（yuzu.jsonc の変更は再起動で反映）
    let mut paths = vec![rc.content_dir.clone()];
    if let Some(theme_dir) = &rc.theme_dir {
        paths.push(theme_dir.clone());
    }
    let rc_for_watch = rc.clone();
    let notifier_for_watch = notifier.clone();
    // session はクロージャへ move してセッション全体で再利用する
    let _watch_handle = yuzu_server::watch(&paths, build::DEBOUNCE, move || {
        tracing::info!("変更を検知 → 再ビルド");
        match build::build_once(&rc_for_watch, mode, &mut session, drafts) {
            // 通知は必ず再ビルド成功後（失敗時に通知すると壊れた dist を読ませる）
            Ok(()) => {
                if let Some(n) = &notifier_for_watch {
                    n.notify();
                }
            }
            // 執筆中の一時的なエラーでプロセスは落とさない
            Err(e) => tracing::error!("再ビルドに失敗しました: {e:#}"),
        }
    })?;

    let host: IpAddr = rc
        .config
        .dev
        .host
        .parse()
        .with_context(|| format!("dev.host が不正です: {}", rc.config.dev.host))?;
    let port = port.unwrap_or(rc.config.dev.port);

    if rc.config.dev.open {
        open_browser_when_ready(SocketAddr::new(host, port), open_url(&rc, host, port));
    }

    // ブロッキング配信（Ctrl+C で終了）
    yuzu_server::serve(ServeOptions {
        dir: rc.output_dir.clone(),
        host,
        port,
        base_url: rc.base_url.clone(),
        live_reload: notifier,
    })?;
    Ok(())
}

/// ブラウザで開く URL。bind アドレスが `0.0.0.0` / `::` のときは
/// 127.0.0.1 に読み替える（bind アドレスにはブラウザで繋げない）
fn open_url(rc: &ResolvedConfig, host: IpAddr, port: u16) -> String {
    let open_host: IpAddr = if host.is_unspecified() {
        IpAddr::V4(Ipv4Addr::LOCALHOST)
    } else {
        host
    };
    format!(
        "http://{open_host}:{port}{}",
        yuzu_server::base_path(&rc.base_url)
    )
}

/// serve() が bind を終えるのを TCP 接続で確認してから既定ブラウザを開く。
/// serve はブロッキング API のため別スレッドでポーリングする
fn open_browser_when_ready(addr: SocketAddr, url: String) {
    std::thread::spawn(move || {
        for _ in 0..50 {
            if std::net::TcpStream::connect_timeout(&addr, Duration::from_millis(100)).is_ok() {
                tracing::info!("ブラウザで {url} を開きます");
                if let Err(e) = open::that_detached(&url) {
                    tracing::warn!("ブラウザを開けませんでした: {e}");
                }
                return;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        tracing::warn!("サーバ起動を確認できず、ブラウザは開きませんでした");
    });
}
