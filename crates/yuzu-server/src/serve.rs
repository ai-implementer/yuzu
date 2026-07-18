//! 最小静的サーバ（axum + tower-http `ServeDir`）。
//!
//! pretty URL（`guide/` → `guide/index.html`）の解決は `ServeDir` の既定挙動。
//! baseUrl がサブパス（例: `/docs/`）のときはそのパスへ nest し、
//! `/` からはリダイレクトする。
//! 存在しないパスは `404.html` があればそれを 404 ステータスで返す
//! （GitHub Pages と同じ挙動。無ければ素の 404）。
//! `live_reload` に [`ReloadNotifier`] を渡すと `/__livereload` に
//! WebSocket エンドポイントを生やす（`yuzu dev` 用）。

use std::net::{IpAddr, SocketAddr};
use std::path::{Path, PathBuf};

use axum::Router;
use axum::extract::ws::WebSocketUpgrade;
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Redirect};
use axum::routing::{any, get};
use tower_http::services::ServeDir;

use crate::error::ServerError;
use crate::livereload::{LIVERELOAD_PATH, ReloadNotifier, handle_socket};

pub struct ServeOptions {
    /// 配信ディレクトリ（通常は `dist/`）
    pub dir: PathBuf,
    pub host: IpAddr,
    pub port: u16,
    /// 正規化済み baseUrl（`/` または `/docs/`。フル URL ならパス部を使う）
    pub base_url: String,
    /// Some なら `/__livereload` に WS エンドポイントを生やす（`yuzu dev` 用）。
    /// preview / build --watch は None
    pub live_reload: Option<ReloadNotifier>,
}

/// ブロッキングで配信を開始する（内部で tokio ランタイムを立ち上げる。
/// 呼び出し側の cli を async にしないための設計）。Ctrl+C で終了
pub fn serve(opts: ServeOptions) -> Result<(), ServerError> {
    let runtime = tokio::runtime::Runtime::new()?;
    runtime.block_on(async move {
        let base = base_path(&opts.base_url).to_string();
        let app = build_router(&opts.dir, &base, opts.live_reload);

        let addr = SocketAddr::new(opts.host, opts.port);
        let listener = tokio::net::TcpListener::bind(addr).await?;
        tracing::info!("http://{addr}{base} で配信中（Ctrl+C で停止）");
        axum::serve(listener, app).await?;
        Ok(())
    })
}

/// Router の組み立て（テスト容易性のため分離）
fn build_router(dir: &Path, base: &str, live_reload: Option<ReloadNotifier>) -> Router {
    // 存在しないパスは dist/404.html を 404 ステータスで返す（毎リクエスト読み直し
    // = watch 中の再ビルドが即反映される。無ければ素の 404）
    let not_found_page = dir.join("404.html");
    let serve_dir = ServeDir::new(dir).not_found_service(any(move || {
        let page = not_found_page.clone();
        async move {
            match tokio::fs::read(&page).await {
                Ok(body) => (
                    StatusCode::NOT_FOUND,
                    [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
                    body,
                )
                    .into_response(),
                Err(_) => StatusCode::NOT_FOUND.into_response(),
            }
        }
    }));

    let mut app = if base == "/" {
        Router::new().fallback_service(serve_dir)
    } else {
        // nest_service のパスは末尾スラッシュなし（例: "/docs"）。"/" を渡すと panic
        let mount = base.trim_end_matches('/').to_string();
        let redirect_to = base.to_string();
        Router::new().nest_service(&mount, serve_dir).route(
            "/",
            get(move || {
                let to = redirect_to.clone();
                async move { Redirect::temporary(&to) }
            }),
        )
    };

    if let Some(notifier) = live_reload {
        // State は使わず Clone クロージャに notifier を捕捉する。
        // subscribe はハンドシェイク前（handler 冒頭）に行い、
        // upgrade 中に発生した通知も Receiver にバッファさせる
        app = app.route(
            LIVERELOAD_PATH,
            any(move |ws: WebSocketUpgrade| {
                let rx = notifier.subscribe();
                async move { ws.on_upgrade(move |socket| handle_socket(socket, rx)) }
            }),
        );
    }

    app
}

/// baseUrl からサーバのマウントパスを取り出す。
/// フル URL（`https://example.com/docs/`）はパス部のみを使う
pub fn base_path(base_url: &str) -> &str {
    match base_url.find("://") {
        Some(pos) => {
            let after = &base_url[pos + 3..];
            match after.find('/') {
                Some(i) => &after[i..],
                None => "/",
            }
        }
        None => base_url,
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use futures_util::StreamExt;

    use super::{ReloadNotifier, base_path, build_router};

    #[test]
    fn base_path_の取り出し() {
        assert_eq!(base_path("/"), "/");
        assert_eq!(base_path("/docs/"), "/docs/");
        assert_eq!(base_path("https://example.com/docs/"), "/docs/");
        assert_eq!(base_path("https://example.com"), "/");
    }

    /// テスト用サーバをエフェメラルポートで起動し、アドレスを返す
    async fn spawn_server(
        dir: &std::path::Path,
        base: &str,
        live_reload: Option<ReloadNotifier>,
    ) -> std::net::SocketAddr {
        let app = build_router(dir, base, live_reload);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        addr
    }

    #[tokio::test]
    async fn ws_で_reload_通知を受信できる() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("index.html"), "<html></html>").unwrap();

        let notifier = ReloadNotifier::new();
        let addr = spawn_server(dir.path(), "/", Some(notifier.clone())).await;

        let (mut ws, _resp) = tokio_tungstenite::connect_async(format!("ws://{addr}/__livereload"))
            .await
            .expect("WS ハンドシェイクが成功する");

        // 監視スレッド相当（非 async スレッド）から notify できることも同時に検証
        let n = notifier.clone();
        std::thread::spawn(move || n.notify()).join().unwrap();

        let msg = tokio::time::timeout(Duration::from_secs(2), ws.next())
            .await
            .expect("2 秒以内に受信")
            .expect("ストリームが閉じていない")
            .expect("受信エラーなし");
        assert_eq!(msg.into_text().unwrap(), "reload");
    }

    #[tokio::test]
    async fn base_付きでも_ws_はルート直下で配信と共存する() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("index.html"), "<html>docs</html>").unwrap();

        let notifier = ReloadNotifier::new();
        let addr = spawn_server(dir.path(), "/docs/", Some(notifier)).await;

        // WS はルート直下
        let (_ws, resp) = tokio_tungstenite::connect_async(format!("ws://{addr}/__livereload"))
            .await
            .expect("base 付きでも WS はルートで繋がる");
        assert_eq!(resp.status().as_u16(), 101);

        // 静的配信は /docs/ 配下
        let body = reqwest_lite(addr, "/docs/index.html").await;
        assert!(body.contains("docs"));
    }

    #[tokio::test]
    async fn live_reload_なしでは_ws_エンドポイントが存在しない() {
        let dir = tempfile::tempdir().unwrap();
        let addr = spawn_server(dir.path(), "/", None).await;

        let result = tokio_tungstenite::connect_async(format!("ws://{addr}/__livereload")).await;
        assert!(result.is_err(), "preview では WS が生えない");
    }

    #[tokio::test]
    async fn 存在しないパスは_404_html_を_404_ステータスで返す() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("index.html"), "<html>home</html>").unwrap();
        std::fs::write(dir.path().join("404.html"), "<html>見つかりません</html>").unwrap();
        let addr = spawn_server(dir.path(), "/", None).await;

        let resp = reqwest_lite(addr, "/no-such-page/").await;
        assert!(resp.starts_with("HTTP/1.0 404"), "resp:\n{resp}");
        assert!(resp.contains("見つかりません"), "resp:\n{resp}");

        // 実在パスは従来どおり 200
        let ok = reqwest_lite(addr, "/index.html").await;
        assert!(ok.starts_with("HTTP/1.0 200"), "resp:\n{ok}");
        assert!(ok.contains("home"));
    }

    #[tokio::test]
    async fn フォールバック用の_404_html_が無ければ素の_404_を返す() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("index.html"), "<html>home</html>").unwrap();
        let addr = spawn_server(dir.path(), "/", None).await;

        let resp = reqwest_lite(addr, "/no-such-page/").await;
        assert!(resp.starts_with("HTTP/1.0 404"), "resp:\n{resp}");
    }

    /// 依存を増やさない最小 HTTP GET（テスト専用）
    async fn reqwest_lite(addr: std::net::SocketAddr, path: &str) -> String {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let mut stream = tokio::net::TcpStream::connect(addr).await.unwrap();
        stream
            .write_all(format!("GET {path} HTTP/1.0\r\nHost: {addr}\r\n\r\n").as_bytes())
            .await
            .unwrap();
        let mut buf = String::new();
        stream.read_to_string(&mut buf).await.unwrap();
        buf
    }
}
