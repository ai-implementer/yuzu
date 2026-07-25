//! ファイル監視（notify + notify-debouncer-mini）。
//!
//! エディタの連続保存を debounce でまとめ、変更があればコールバックを呼ぶ。
//!
//! ⚠️ **出力ディレクトリを必ず除外すること**。`dist/` の変更を拾うと
//! 再ビルド → 変更検知 → 再ビルドの無限ループになる。コンテンツインクルード
//! （`file=` 参照）のためにプロジェクトルート全体を監視する運用になったため、
//! 除外は [`watch`] の `ignore` 引数で明示的に渡す（隠しディレクトリ配下は
//! `.git` / `.yuzu` を含めて常に無視する）。

use std::path::{Path, PathBuf};
use std::time::Duration;

use notify::RecursiveMode;
use notify_debouncer_mini::{DebounceEventResult, Debouncer, new_debouncer};

use crate::error::ServerError;

/// 監視ハンドル。drop すると監視が止まるため、watch 中は保持し続けること
pub struct WatchHandle {
    _debouncer: Debouncer<notify::RecommendedWatcher>,
}

/// 監視対象外のパスか。`ignore` 配下（出力ディレクトリ等）と、
/// 構成要素に隠しディレクトリ（`.` 始まり）を含むパスを無視する
pub fn should_ignore(path: &Path, ignore: &[PathBuf]) -> bool {
    if ignore.iter().any(|dir| path.starts_with(dir)) {
        return true;
    }
    path.components().any(|c| {
        c.as_os_str()
            .to_str()
            .is_some_and(|name| name.starts_with('.') && name.len() > 1)
    })
}

/// `paths` を再帰監視し、変更が落ち着いたら `on_change` を呼ぶ。
/// `ignore` 配下だけの変更では呼ばない（出力ディレクトリの自己検知を防ぐ）。
/// コールバックは監視スレッド上で実行される
pub fn watch(
    paths: &[PathBuf],
    ignore: &[PathBuf],
    debounce: Duration,
    mut on_change: impl FnMut() + Send + 'static,
) -> Result<WatchHandle, ServerError> {
    let ignore_owned: Vec<PathBuf> = ignore.to_vec();
    let mut debouncer = new_debouncer(debounce, move |result: DebounceEventResult| match result {
        Ok(events) => {
            // 1 つでも対象内の変更があれば再ビルド（全部が除外対象なら何もしない）
            if events
                .iter()
                .any(|e| !should_ignore(&e.path, &ignore_owned))
            {
                on_change();
            }
        }
        Err(e) => tracing::warn!("ファイル監視エラー: {e}"),
    })?;

    for path in paths {
        debouncer.watcher().watch(path, RecursiveMode::Recursive)?;
        tracing::info!("監視中: {}", path.display());
    }
    for dir in ignore {
        tracing::debug!("監視除外: {}", dir.display());
    }

    Ok(WatchHandle {
        _debouncer: debouncer,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 出力ディレクトリ配下は無視する() {
        let ignore = vec![PathBuf::from("/proj/dist")];
        assert!(should_ignore(Path::new("/proj/dist/index.html"), &ignore));
        assert!(should_ignore(Path::new("/proj/dist"), &ignore));
        assert!(!should_ignore(Path::new("/proj/content/a.md"), &ignore));
        assert!(!should_ignore(Path::new("/proj/src/lib.rs"), &ignore));
    }

    #[test]
    fn 隠しディレクトリ配下は常に無視する() {
        let ignore: Vec<PathBuf> = Vec::new();
        assert!(should_ignore(Path::new("/proj/.git/index"), &ignore));
        assert!(should_ignore(
            Path::new("/proj/.yuzu/cache/x.json"),
            &ignore
        ));
        // ファイル名が . 始まりでも無視（エディタの一時ファイル等）
        assert!(should_ignore(Path::new("/proj/content/.swp"), &ignore));
        // カレント表記（"."）は無視対象にしない
        assert!(!should_ignore(Path::new("content/a.md"), &ignore));
    }
}
