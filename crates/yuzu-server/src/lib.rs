//! yuzu の配信・監視（v0.1 は最小構成）。
//!
//! - [`serve`] — `dist/` を配信する最小静的サーバ（axum + `ServeDir`）。
//!   Phase 2 でここに WebSocket ライブリロードを足す（凍結方針）
//! - [`watch`] — `content/` / `theme/` の監視（notify + debouncer）。
//!   再ビルドのロジックはコールバックとして呼び出し側（cli）が渡す
//!   （依存方向 `cli → server` を守り、server は render を知らない）

mod error;
mod serve;
mod watch;

pub use error::ServerError;
pub use serve::{ServeOptions, serve};
pub use watch::{WatchHandle, watch};
