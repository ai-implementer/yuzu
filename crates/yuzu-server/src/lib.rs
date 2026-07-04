//! yuzu の配信・監視。
//!
//! - [`serve`] — `dist/` を配信する静的サーバ（axum + `ServeDir`）。
//!   [`ReloadNotifier`] を渡すと `/__livereload` に WebSocket ライブリロードを
//!   生やす（`yuzu dev`）。preview は通知なしの純粋な静的配信
//! - [`watch`] — `content/` / `theme/` の監視（notify + debouncer）。
//!   再ビルドのロジックはコールバックとして呼び出し側（cli）が渡す
//!   （依存方向 `cli → server` を守り、server は render を知らない）

mod error;
mod livereload;
mod serve;
mod watch;

pub use error::ServerError;
pub use livereload::{LIVERELOAD_PATH, ReloadNotifier};
pub use serve::{ServeOptions, base_path, serve};
pub use watch::{WatchHandle, watch};
