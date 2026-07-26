//! ファイル監視（notify + notify-debouncer-mini）。
//!
//! エディタの連続保存を debounce でまとめ、変更があればコールバックを呼ぶ。
//!
//! ⚠️ **出力ディレクトリを必ず除外すること**。`dist/` の変更を拾うと
//! 再ビルド → 変更検知 → 再ビルドの無限ループになる。コンテンツインクルード
//! （`file=` 参照）のためにプロジェクトルート全体を監視する運用になったため、
//! 除外は [`WatchIgnore`] で明示的に渡す（隠しディレクトリ配下は
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

/// 監視除外の規則。
///
/// ディレクトリ前置（出力ディレクトリ等）と隠しディレクトリは server 内の固定規則。
/// glob（`build.watchIgnore`）のような追加規則は述語で受け取る — glob の解釈は
/// yuzu-core にあり、依存方向 `cli → server` を守って server は yuzu-core を
/// 知らないため。
///
/// なお除外は**イベントのフィルタ**であって監視の登録自体は減らない
/// （notify にパス単位の除外が無い）。再ビルドの暴発は防げるが、
/// OS の監視資源は `target/` 配下にも使われる
#[derive(Default)]
pub struct WatchIgnore {
    dirs: Vec<PathBuf>,
    extra: Option<ExtraRule>,
}

/// 追加の除外述語（true なら除外）
type ExtraRule = Box<dyn Fn(&Path) -> bool + Send>;

impl WatchIgnore {
    /// `dirs` 配下（絶対パス。出力ディレクトリ等）を除外する
    pub fn new(dirs: Vec<PathBuf>) -> Self {
        Self { dirs, extra: None }
    }

    /// 追加の除外述語（true なら除外）。呼び出し側の glob 判定を挿す口
    pub fn with_extra(mut self, extra: impl Fn(&Path) -> bool + Send + 'static) -> Self {
        self.extra = Some(Box::new(extra));
        self
    }

    /// 監視対象外のパスか。`dirs` 配下・構成要素に隠しディレクトリ
    /// （`.` 始まり）を含むパス・追加述語に当たるパスを無視する
    pub fn is_ignored(&self, path: &Path) -> bool {
        if self.dirs.iter().any(|dir| path.starts_with(dir)) {
            return true;
        }
        if path.components().any(|c| {
            c.as_os_str()
                .to_str()
                .is_some_and(|name| name.starts_with('.') && name.len() > 1)
        }) {
            return true;
        }
        self.extra.as_ref().is_some_and(|f| f(path))
    }
}

/// `paths` を再帰監視し、変更が落ち着いたら `on_change` を呼ぶ。
/// `ignore` に当たる変更では呼ばない（出力ディレクトリの自己検知を防ぐ）。
/// コールバックは監視スレッド上で実行される
pub fn watch(
    paths: &[PathBuf],
    ignore: WatchIgnore,
    debounce: Duration,
    mut on_change: impl FnMut() + Send + 'static,
) -> Result<WatchHandle, ServerError> {
    for dir in &ignore.dirs {
        tracing::debug!("監視除外: {}", dir.display());
    }
    let mut debouncer = new_debouncer(debounce, move |result: DebounceEventResult| match result {
        Ok(events) => {
            // 1 つでも対象内の変更があれば再ビルド（全部が除外対象なら何もしない）
            if events.iter().any(|e| !ignore.is_ignored(&e.path)) {
                on_change();
            }
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 出力ディレクトリ配下は無視する() {
        let ignore = WatchIgnore::new(vec![PathBuf::from("/proj/dist")]);
        assert!(ignore.is_ignored(Path::new("/proj/dist/index.html")));
        assert!(ignore.is_ignored(Path::new("/proj/dist")));
        assert!(!ignore.is_ignored(Path::new("/proj/content/a.md")));
        assert!(!ignore.is_ignored(Path::new("/proj/src/lib.rs")));
    }

    #[test]
    fn 隠しディレクトリ配下は常に無視する() {
        let ignore = WatchIgnore::default();
        assert!(ignore.is_ignored(Path::new("/proj/.git/index")));
        assert!(ignore.is_ignored(Path::new("/proj/.yuzu/cache/x.json")));
        // ファイル名が . 始まりでも無視（エディタの一時ファイル等）
        assert!(ignore.is_ignored(Path::new("/proj/content/.swp")));
        // カレント表記（"."）は無視対象にしない
        assert!(!ignore.is_ignored(Path::new("content/a.md")));
    }

    #[test]
    fn 追加述語の除外も効く() {
        let ignore = WatchIgnore::new(vec![PathBuf::from("/proj/dist")])
            .with_extra(|path| path.to_string_lossy().contains("/target/"));
        assert!(ignore.is_ignored(Path::new("/proj/target/debug/yuzu")));
        assert!(!ignore.is_ignored(Path::new("/proj/content/target.md")));
    }
}
