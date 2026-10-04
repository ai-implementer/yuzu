//! yuzu の配信・監視。
//!
//! - [`serve`] — `dist/` を配信する静的サーバ（axum + `ServeDir`）。
//!   [`ReloadNotifier`] を渡すと `/__livereload` に WebSocket ライブリロードを
//!   生やす（`yuzu dev`）。preview は通知なしの純粋な静的配信
//! - [`watch`] — プロジェクトルートの監視（notify。debounce と種類の絞り込みは
//!   自前の監視スレッド）。再ビルドのロジックはコールバックとして呼び出し側
//!   （cli）が渡す（依存方向 `cli → server` を守り、server は render を知らない）

mod error;
mod livereload;
mod serve;
mod watch;

pub use error::ServerError;
pub use livereload::{LIVERELOAD_PATH, ReloadNotifier};
pub use serve::{PathGuard, ServeOptions, base_path, serve};
pub use watch::{WatchFailure, WatchHandle, WatchIgnore, watch};
