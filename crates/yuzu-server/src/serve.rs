//! 最小静的サーバ（axum + tower-http `ServeDir`）。
//!
//! pretty URL（`guide/` → `guide/index.html`）の解決は `ServeDir` の既定挙動。
//! baseUrl がサブパス（例: `/docs/`）のときはそのパスへ nest し、
//! `/` からはリダイレクトする。

use std::net::{IpAddr, SocketAddr};
use std::path::PathBuf;

use axum::Router;
use axum::response::Redirect;
use axum::routing::get;
use tower_http::services::ServeDir;

use crate::error::ServerError;

pub struct ServeOptions {
    /// 配信ディレクトリ（通常は `dist/`）
    pub dir: PathBuf,
    pub host: IpAddr,
    pub port: u16,
    /// 正規化済み baseUrl（`/` または `/docs/`。フル URL ならパス部を使う）
    pub base_url: String,
}

/// ブロッキングで配信を開始する（内部で tokio ランタイムを立ち上げる。
/// 呼び出し側の cli を async にしないための設計）。Ctrl+C で終了
pub fn serve(opts: ServeOptions) -> Result<(), ServerError> {
    let runtime = tokio::runtime::Runtime::new()?;
    runtime.block_on(async move {
        let base = base_path(&opts.base_url).to_string();
        let serve_dir = ServeDir::new(&opts.dir);

        let app = if base == "/" {
            Router::new().fallback_service(serve_dir)
        } else {
            // nest_service のパスは末尾スラッシュなし（例: "/docs"）
            let mount = base.trim_end_matches('/').to_string();
            let redirect_to = base.clone();
            Router::new().nest_service(&mount, serve_dir).route(
                "/",
                get(move || {
                    let to = redirect_to.clone();
                    async move { Redirect::temporary(&to) }
                }),
            )
        };

        let addr = SocketAddr::new(opts.host, opts.port);
        let listener = tokio::net::TcpListener::bind(addr).await?;
        tracing::info!("http://{addr}{base} で配信中（Ctrl+C で停止）");
        axum::serve(listener, app).await?;
        Ok(())
    })
}

/// baseUrl からサーバのマウントパスを取り出す。
/// フル URL（`https://example.com/docs/`）はパス部のみを使う
fn base_path(base_url: &str) -> &str {
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
    use super::base_path;

    #[test]
    fn base_path_の取り出し() {
        assert_eq!(base_path("/"), "/");
        assert_eq!(base_path("/docs/"), "/docs/");
        assert_eq!(base_path("https://example.com/docs/"), "/docs/");
        assert_eq!(base_path("https://example.com"), "/");
    }
}
