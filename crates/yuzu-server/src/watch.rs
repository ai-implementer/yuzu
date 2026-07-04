//! ファイル監視（notify + notify-debouncer-mini）。
//!
//! エディタの連続保存を debounce でまとめ、変更があればコールバックを呼ぶ。
//! 監視対象は `content/`・`theme/` のみにすること（`dist/` を入れると
//! 再ビルド → 変更検知 → 再ビルドの無限ループになる）。

use std::path::PathBuf;
use std::time::Duration;

use notify::RecursiveMode;
use notify_debouncer_mini::{DebounceEventResult, Debouncer, new_debouncer};

use crate::error::ServerError;

/// 監視ハンドル。drop すると監視が止まるため、watch 中は保持し続けること
pub struct WatchHandle {
    _debouncer: Debouncer<notify::RecommendedWatcher>,
}

/// `paths` を再帰監視し、変更が落ち着いたら `on_change` を呼ぶ。
/// コールバックは監視スレッド上で実行される
pub fn watch(
    paths: &[PathBuf],
    debounce: Duration,
    mut on_change: impl FnMut() + Send + 'static,
) -> Result<WatchHandle, ServerError> {
    let mut debouncer = new_debouncer(debounce, move |result: DebounceEventResult| match result {
        Ok(events) if !events.is_empty() => on_change(),
        Ok(_) => {}
        Err(e) => tracing::warn!("ファイル監視エラー: {e}"),
    })?;

    for path in paths {
        debouncer.watcher().watch(path, RecursiveMode::Recursive)?;
        tracing::info!("監視中: {}", path.display());
    }

    Ok(WatchHandle {
        _debouncer: debouncer,
    })
}
