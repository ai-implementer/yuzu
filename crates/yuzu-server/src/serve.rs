//! 最小静的サーバ（axum + tower-http `ServeDir`）。
//!
//! pretty URL（`guide/` → `guide/index.html`）の解決は `ServeDir` の既定挙動。
//! baseUrl がサブパス（例: `/docs/`）のときはそのパスへ nest し、
//! `/` からはリダイレクトする。
//! 存在しないパスは `404.html` があればそれを 404 ステータスで返す
//! （GitHub Pages と同じ挙動。無ければ素の 404）。
//! `live_reload` に [`ReloadNotifier`] を渡すと `/__livereload` に
//! WebSocket エンドポイントを生やす（`yuzu dev` 用）。
//!
//! `path_guard` を渡すと、`ServeDir` がファイルを開く・metadata を引く前と
//! 404 フォールバックの読み込み前に述語を呼び、拒否されたパスは**存在しないもの**
//! として扱う（404）。`ServeDir` のパス検証は字句（`..` の拒否）だけで
//! シンボリックリンクを辿るため、書き側（`yuzu_core::output`）と同じ規律を
//! 読み側にも掛けるのが目的。server は core を知らないので、検査の中身は
//! cli が [`PathGuard`] に包んで渡す（`WatchIgnore` と同型）

use std::future::Future;
use std::io;
use std::net::{IpAddr, SocketAddr};
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::Arc;

use axum::Router;
use axum::extract::ws::WebSocketUpgrade;
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Redirect};
use axum::routing::{any, get};
use tower_http::services::ServeDir;
use tower_http::services::fs::{Backend, TokioBackend, TokioFile};

use crate::error::ServerError;
use crate::livereload::{LIVERELOAD_PATH, ReloadNotifier, handle_socket};

/// 配信パスの検査述語。引数は配信ディレクトリ配下の**絶対パス**（`ServeDir` が
/// 要求パスを結合した後の値）。`Err(理由)` なら配信しない（404 扱い・理由は
/// warn ログに出る）
pub type PathGuard = Arc<dyn Fn(&Path) -> Result<(), String> + Send + Sync>;

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
    /// 配信パスの検査（None なら `ServeDir` の既定どおりリンクも辿る）
    pub path_guard: Option<PathGuard>,
}

/// `TokioBackend` を包み、`open` / `metadata` の前に [`PathGuard`] を通す。
/// 拒否は `NotFound` にする = `ServeDir` が 404（`not_found_service` へ）として扱う
#[derive(Clone)]
struct GuardedBackend {
    inner: TokioBackend,
    guard: Option<PathGuard>,
}

impl GuardedBackend {
    fn check(&self, path: &Path) -> io::Result<()> {
        if let Some(guard) = &self.guard {
            if let Err(reason) = guard(path) {
                tracing::warn!(path = %path.display(), reason = %reason, "配信しません");
                return Err(io::Error::new(io::ErrorKind::NotFound, reason));
            }
        }
        Ok(())
    }
}

impl Backend for GuardedBackend {
    type File = TokioFile;
    type Metadata = std::fs::Metadata;
    type OpenFuture = Pin<Box<dyn Future<Output = io::Result<TokioFile>> + Send>>;
    type MetadataFuture = Pin<Box<dyn Future<Output = io::Result<std::fs::Metadata>> + Send>>;

    fn open(&self, path: PathBuf) -> Self::OpenFuture {
        match self.check(&path) {
            Ok(()) => self.inner.open(path),
            Err(e) => Box::pin(std::future::ready(Err(e))),
        }
    }

    fn metadata(&self, path: PathBuf) -> Self::MetadataFuture {
        match self.check(&path) {
            Ok(()) => self.inner.metadata(path),
            Err(e) => Box::pin(std::future::ready(Err(e))),
        }
    }
}

/// ブロッキングで配信を開始する（内部で tokio ランタイムを立ち上げる。
/// 呼び出し側の cli を async にしないための設計）。Ctrl+C で終了
pub fn serve(opts: ServeOptions) -> Result<(), ServerError> {
    let runtime = tokio::runtime::Runtime::new()?;
    runtime.block_on(async move {
        let base = base_path(&opts.base_url).to_string();
        let app = build_router(&opts.dir, &base, opts.live_reload, opts.path_guard);

        let addr = SocketAddr::new(opts.host, opts.port);
        let listener = match tokio::net::TcpListener::bind(addr).await {
            Ok(listener) => listener,
            // 既定ポートで dev を 2 つ立てると必ず踏むので、ここだけ専用の文言にする
            Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => {
                return Err(ServerError::PortInUse {
                    host: opts.host,
                    port: opts.port,
                });
            }
            Err(e) => return Err(e.into()),
        };
        tracing::info!("http://{addr}{base} で配信中（Ctrl+C で停止）");
        axum::serve(listener, app).await?;
        Ok(())
    })
}

/// Router の組み立て（テスト容易性のため分離）
fn build_router(
    dir: &Path,
    base: &str,
    live_reload: Option<ReloadNotifier>,
    path_guard: Option<PathGuard>,
) -> Router {
    let backend = GuardedBackend {
        inner: TokioBackend,
        guard: path_guard,
    };
    // 存在しないパスは dist/404.html を 404 ステータスで返す（毎リクエスト読み直し
    // = watch 中の再ビルドが即反映される。無ければ素の 404）。
    // 404.html 自身も述語を通す（リンクなら読まずに素の 404）
    let not_found_page = dir.join("404.html");
    let not_found_backend = backend.clone();
    let serve_dir = ServeDir::with_backend(dir, backend).not_found_service(any(move || {
        let page = not_found_page.clone();
        let backend = not_found_backend.clone();
        async move {
            let body = match backend.check(&page) {
                Ok(()) => tokio::fs::read(&page).await,
                Err(e) => Err(e),
            };
            match body {
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

    use super::{PathGuard, ReloadNotifier, base_path, build_router};

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
        spawn_server_with(dir, base, live_reload, None).await
    }

    async fn spawn_server_with(
        dir: &std::path::Path,
        base: &str,
        live_reload: Option<ReloadNotifier>,
        path_guard: Option<PathGuard>,
    ) -> std::net::SocketAddr {
        let app = build_router(dir, base, live_reload, path_guard);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        addr
    }

    /// cli が渡す述語の最小再現（`yuzu_core::output::ensure_symlink_free` 相当。
    /// server は core に依存しないのでテスト内に置く）: root から各要素を lstat し、
    /// リンクがあれば Err
    #[cfg(unix)]
    fn symlink_guard(root: std::path::PathBuf) -> PathGuard {
        std::sync::Arc::new(move |path: &std::path::Path| {
            let rel = path
                .strip_prefix(&root)
                .map_err(|_| format!("{} は配信ディレクトリの外", path.display()))?;
            if root
                .symlink_metadata()
                .is_ok_and(|m| m.file_type().is_symlink())
            {
                return Err(format!("基準 {} がシンボリックリンク", root.display()));
            }
            let mut cur = root.clone();
            for comp in rel.components() {
                cur.push(comp);
                match cur.symlink_metadata() {
                    Ok(m) if m.file_type().is_symlink() => {
                        return Err(format!(
                            "経路にシンボリックリンクがあります: {}",
                            cur.display()
                        ));
                    }
                    Ok(_) => {}
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => break,
                    Err(e) => return Err(e.to_string()),
                }
            }
            Ok(())
        })
    }

    /// 配信ディレクトリ**まで**の中間要素がリンク（`root/alias -> outside`、
    /// 配信ディレクトリは `root/alias/site`）でも、述語の起点がその上（プロジェクト
    /// ルート相当）なら配信しない。cli は起点にプロジェクトルートを渡す
    #[cfg(unix)]
    #[tokio::test]
    async fn 配信ディレクトリまでの中間ディレクトリのリンクも辿らない() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("root");
        let outside = tmp.path().join("outside");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::create_dir_all(outside.join("site")).unwrap();
        std::fs::write(outside.join("site/index.html"), "<html>leaked</html>").unwrap();
        std::os::unix::fs::symlink(&outside, root.join("alias")).unwrap();
        let dist = root.join("alias/site");

        let guarded = spawn_server_with(&dist, "/", None, Some(symlink_guard(root.clone()))).await;
        for path in ["/", "/index.html"] {
            let resp = reqwest_lite(guarded, path).await;
            assert!(resp.starts_with("HTTP/1.0 404"), "{path}: {resp}");
            assert!(!resp.contains("leaked"), "{path}: {resp}");
        }
        // 起点を配信ディレクトリにすると見逃す（= 起点をルートにする理由）
        let narrow = spawn_server_with(&dist, "/", None, Some(symlink_guard(dist.clone()))).await;
        let resp = reqwest_lite(narrow, "/index.html").await;
        assert!(resp.starts_with("HTTP/1.0 200"), "{resp}");
    }

    /// dist 配下のリンク（`dist/link -> outside/`）は、書き側が拒否するのと同じく
    /// 読み側も辿らない。述語なし（`ServeDir` 素）だと辿ってしまうことも併記して、
    /// 遮断が述語で実現されていることを固定する
    #[cfg(unix)]
    #[tokio::test]
    async fn シンボリックリンク配下は配信せず_404_html_に乗る() {
        let tmp = tempfile::tempdir().unwrap();
        let outside = tmp.path().join("outside");
        let dist = tmp.path().join("dist");
        std::fs::create_dir_all(&outside).unwrap();
        std::fs::create_dir_all(&dist).unwrap();
        std::fs::write(outside.join("secret.html"), "<html>secret</html>").unwrap();
        std::fs::write(dist.join("index.html"), "<html>home</html>").unwrap();
        std::fs::write(dist.join("404.html"), "<html>見つかりません</html>").unwrap();
        std::os::unix::fs::symlink(&outside, dist.join("link")).unwrap();
        std::os::unix::fs::symlink(outside.join("secret.html"), dist.join("leaf.html")).unwrap();

        let guarded = spawn_server_with(&dist, "/", None, Some(symlink_guard(dist.clone()))).await;
        for path in ["/link/secret.html", "/link/", "/leaf.html"] {
            let resp = reqwest_lite(guarded, path).await;
            assert!(resp.starts_with("HTTP/1.0 404"), "{path}: {resp}");
            assert!(
                resp.contains("見つかりません"),
                "{path}: 404.html に乗る: {resp}"
            );
            assert!(
                !resp.contains("secret"),
                "{path}: リンク先が漏れない: {resp}"
            );
        }
        // 実体は従来どおり
        let ok = reqwest_lite(guarded, "/").await;
        assert!(
            ok.starts_with("HTTP/1.0 200") && ok.contains("home"),
            "{ok}"
        );

        // 述語なしの ServeDir はリンクを辿る（= 遮断は述語の責務）
        let plain = spawn_server(&dist, "/", None).await;
        let resp = reqwest_lite(plain, "/link/secret.html").await;
        assert!(
            resp.starts_with("HTTP/1.0 200") && resp.contains("secret"),
            "{resp}"
        );
    }

    /// 404.html 自身がリンクなら読まずに素の 404 を返す
    #[cfg(unix)]
    #[tokio::test]
    async fn フォールバックの_404_html_がリンクなら素の_404() {
        let tmp = tempfile::tempdir().unwrap();
        let outside = tmp.path().join("outside");
        let dist = tmp.path().join("dist");
        std::fs::create_dir_all(&outside).unwrap();
        std::fs::create_dir_all(&dist).unwrap();
        std::fs::write(outside.join("404.html"), "<html>leaked</html>").unwrap();
        std::os::unix::fs::symlink(outside.join("404.html"), dist.join("404.html")).unwrap();

        let addr = spawn_server_with(&dist, "/", None, Some(symlink_guard(dist.clone()))).await;
        let resp = reqwest_lite(addr, "/no-such-page/").await;
        assert!(resp.starts_with("HTTP/1.0 404"), "{resp}");
        assert!(!resp.contains("leaked"), "{resp}");
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
