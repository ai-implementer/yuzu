//! ファイル監視（notify）。
//!
//! エディタの連続保存を debounce でまとめ、変更があればコールバックを呼ぶ。
//!
//! ⚠️ **出力ディレクトリを必ず除外すること**。`dist/` の変更を拾うと
//! 再ビルド → 変更検知 → 再ビルドの無限ループになる。コンテンツインクルード
//! （`file=` 参照）のためにプロジェクトルート全体を監視する運用になったため、
//! 除外は [`WatchIgnore`] で明示的に渡す（隠しディレクトリ配下は
//! `.git` / `.yuzu` を含めて常に無視する）。
//!
//! ⚠️ **イベントの種類で絞ること**。Linux（inotify）の notify はファイルを
//! 開いた・読んだだけのイベントも届けるため、種類を見ずに「変更」とすると
//! ビルドが原稿を読むたびに再ビルドが起き、止まらなくなる（macOS の FSEvents は
//! 開いただけでは通知しないので気付きにくい）。種類を捨てる debouncer を使わず、
//! 受信・絞り込み・debounce をこのモジュールの監視スレッドで行う。
//!
//! コールバックの panic は監視スレッドで受け止め、[`WatchFailure`] で
//! 呼び出し側へ知らせる。監視スレッドの panic は main の `catch_unwind` に
//! 届かないため、知らせないと監視だけが黙って止まり配信が残る。

use std::any::Any;
use std::collections::BTreeSet;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::time::{Duration, Instant};

use notify::event::{AccessKind, AccessMode};
use notify::{Event, EventKind, RecursiveMode, Watcher};
use tokio::sync::oneshot;

use crate::error::ServerError;

/// 監視ハンドル。drop すると監視が止まるため、watch 中は保持し続けること
pub struct WatchHandle {
    _watcher: notify::RecommendedWatcher,
    failure: Option<WatchFailure>,
}

impl WatchHandle {
    /// 監視スレッドが panic で止まったときに知らせる合図を取り出す
    /// （2 回目以降は None）。[`crate::ServeOptions::watch_failure`] に渡すと、
    /// 監視が止まった時点で配信も止めて `serve` が Err を返す
    pub fn take_failure(&mut self) -> Option<WatchFailure> {
        self.failure.take()
    }
}

/// 監視スレッドがコールバックの panic で止まったことの合図。
/// 中身は panic のメッセージ
pub struct WatchFailure {
    rx: oneshot::Receiver<String>,
}

impl WatchFailure {
    /// 監視が panic で止まるまで待ち、メッセージを返す。監視が正常に終わった
    /// （ハンドルの drop 等で送り手が消えた）ときは完了しない
    pub(crate) async fn wait(self) -> String {
        match self.rx.await {
            Ok(message) => message,
            Err(_) => std::future::pending().await,
        }
    }
}

/// 1 回の debounce で待つ上限（debounce 間隔の倍数）。変更が途切れず届き
/// 続けても、この時間でいったん区切ってコールバックを呼ぶ
const MAX_BATCH_FACTOR: u32 = 10;

/// 監視除外の規則。
///
/// ディレクトリ前置（出力ディレクトリ等）と隠しディレクトリは server 内の固定規則。
/// glob（`build.watch_ignore`）のような追加規則は述語で受け取る — glob の解釈は
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
    /// 監視ルート（[`watch`] が設定する）。隠しディレクトリの判定はここからの
    /// 相対パスで行う
    roots: Vec<PathBuf>,
}

/// 追加の除外述語（true なら除外）
type ExtraRule = Box<dyn Fn(&Path) -> bool + Send>;

impl WatchIgnore {
    /// `dirs` 配下（絶対パス。出力ディレクトリ等）を除外する
    pub fn new(dirs: Vec<PathBuf>) -> Self {
        Self {
            dirs,
            ..Self::default()
        }
    }

    /// 追加の除外述語（true なら除外）。呼び出し側の glob 判定を挿す口
    pub fn with_extra(mut self, extra: impl Fn(&Path) -> bool + Send + 'static) -> Self {
        self.extra = Some(Box::new(extra));
        self
    }

    /// 監視対象外のパスか。`dirs` 配下・構成要素に隠しディレクトリ
    /// （`.` 始まり）を含むパス・追加述語に当たるパスを無視する。
    ///
    /// 隠しディレクトリは**監視ルートからの相対パス**で判定する。イベントは絶対パスで
    /// 届くので、全構成要素を見るとルートの祖先（`~/.config/notes/` や
    /// `.claude/worktrees/…`）に `.` 始まりがあるだけで全イベントを捨て、
    /// 再ビルドが黙って止まる。ルート外のパスは全構成要素で判定する
    pub fn is_ignored(&self, path: &Path) -> bool {
        if self.dirs.iter().any(|dir| path.starts_with(dir)) {
            return true;
        }
        let rel = self
            .roots
            .iter()
            .find_map(|root| path.strip_prefix(root).ok())
            .unwrap_or(path);
        if rel.components().any(|c| {
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
/// `ignore` に当たる変更と、内容を変えない種類のイベント（開いた・読んだだけ）では
/// 呼ばない。コールバックは監視スレッド上で実行され、除外を除いた変更パス
/// （重複除去・ソート済み）を受け取る。
///
/// コールバックが panic したら監視を止め、[`WatchHandle::take_failure`] の合図へ
/// メッセージを送る
pub fn watch(
    paths: &[PathBuf],
    mut ignore: WatchIgnore,
    debounce: Duration,
    on_change: impl FnMut(&[PathBuf]) + Send + 'static,
) -> Result<WatchHandle, ServerError> {
    ignore.roots = paths.to_vec();
    for dir in &ignore.dirs {
        tracing::debug!("監視除外: {}", dir.display());
    }
    let (event_tx, event_rx) = mpsc::channel();
    let mut watcher = notify::recommended_watcher(event_tx)?;
    for path in paths {
        watcher.watch(path, RecursiveMode::Recursive)?;
        tracing::info!("監視中: {}", path.display());
    }

    let (failure_tx, failure_rx) = oneshot::channel();
    std::thread::Builder::new()
        .name("yuzu-watch".to_string())
        .spawn(move || run_loop(&event_rx, &ignore, debounce, on_change, failure_tx))?;

    Ok(WatchHandle {
        _watcher: watcher,
        failure: Some(WatchFailure { rx: failure_rx }),
    })
}

/// 監視スレッドの本体。最初のイベントを待ち、静かになるまで（`debounce` の間
/// 次が来なくなるまで）集めてからコールバックを呼ぶ。送り手（watcher）が
/// drop されたら抜ける
fn run_loop(
    events: &mpsc::Receiver<notify::Result<Event>>,
    ignore: &WatchIgnore,
    debounce: Duration,
    mut on_change: impl FnMut(&[PathBuf]),
    failure: oneshot::Sender<String>,
) {
    let max_batch = debounce * MAX_BATCH_FACTOR;
    loop {
        let Ok(first) = events.recv() else {
            return;
        };
        // BTreeSet = 決定的順序と、同一パスの連続保存の重複除去
        let mut changed = BTreeSet::new();
        collect(first, ignore, &mut changed);
        let started = Instant::now();
        let mut disconnected = false;
        while started.elapsed() < max_batch {
            match events.recv_timeout(debounce) {
                Ok(event) => collect(event, ignore, &mut changed),
                Err(RecvTimeoutError::Timeout) => break,
                Err(RecvTimeoutError::Disconnected) => {
                    disconnected = true;
                    break;
                }
            }
        }
        if !changed.is_empty() {
            let changed: Vec<PathBuf> = changed.into_iter().collect();
            if let Err(payload) = catch_unwind(AssertUnwindSafe(|| on_change(&changed))) {
                let message = panic_message(payload.as_ref()).to_string();
                tracing::error!(
                    "監視中の再ビルドで内部エラーが起きたため、監視を止めます: {message}"
                );
                // 受け手が既に無い（配信していない）ならそのまま終わる
                let _ = failure.send(message);
                return;
            }
        }
        if disconnected {
            return;
        }
    }
}

/// イベント 1 件から、対象になる変更パスを `changed` へ足す
fn collect(event: notify::Result<Event>, ignore: &WatchIgnore, changed: &mut BTreeSet<PathBuf>) {
    match event {
        Ok(event) if is_content_change(&event.kind) => {
            changed.extend(event.paths.into_iter().filter(|p| !ignore.is_ignored(p)));
        }
        Ok(_) => {}
        Err(e) => tracing::warn!("ファイル監視エラー: {e}"),
    }
}

/// 内容が変わりうるイベントか。アクセス系（開いた・読んだ・読み取りで閉じた）は
/// 内容を変えないので捨てる。ただし書き込みを終えて閉じた（inotify の
/// `CLOSE_WRITE`）は保存の完了なので残す
fn is_content_change(kind: &EventKind) -> bool {
    match kind {
        EventKind::Access(AccessKind::Close(AccessMode::Write)) => true,
        EventKind::Access(_) => false,
        _ => true,
    }
}

/// panic のペイロードからメッセージを取り出す（`panic!` の引数は `&str` か `String`）
fn panic_message(payload: &(dyn Any + Send)) -> &str {
    if let Some(s) = payload.downcast_ref::<&str>() {
        s
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s
    } else {
        "（メッセージなし）"
    }
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
    fn 監視ルートの祖先にある隠しディレクトリでは無視しない() {
        // ルートが `~/.config/notes` のような隠しディレクトリ配下にあっても、
        // ルートより下に `.` 始まりが無ければ変更として扱う
        let ignore = WatchIgnore {
            roots: vec![PathBuf::from("/home/u/.config/notes")],
            ..WatchIgnore::default()
        };
        assert!(!ignore.is_ignored(Path::new("/home/u/.config/notes/content/a.md")));
        // ルートより下の隠しディレクトリ・隠しファイルは従来どおり無視する
        assert!(ignore.is_ignored(Path::new("/home/u/.config/notes/.git/index")));
        assert!(ignore.is_ignored(Path::new("/home/u/.config/notes/content/.swp")));
        // ルート外（シンボリックリンク先など）は全構成要素で判定する
        assert!(ignore.is_ignored(Path::new("/other/.cache/x.md")));
    }

    #[test]
    fn 開いた_読んだだけのイベントは変更として扱わない() {
        use notify::event::{CreateKind, DataChange, ModifyKind, RemoveKind};
        // Linux の inotify は open / close(read) も届ける。これを変更とすると
        // ビルドが原稿を読むたびに再ビルドが起きて止まらない
        assert!(!is_content_change(&EventKind::Access(AccessKind::Open(
            AccessMode::Any
        ))));
        assert!(!is_content_change(&EventKind::Access(AccessKind::Close(
            AccessMode::Read
        ))));
        assert!(!is_content_change(&EventKind::Access(AccessKind::Read)));
        // 書き込みを終えて閉じた（CLOSE_WRITE）は保存の完了なので残す
        assert!(is_content_change(&EventKind::Access(AccessKind::Close(
            AccessMode::Write
        ))));
        assert!(is_content_change(&EventKind::Modify(ModifyKind::Data(
            DataChange::Any
        ))));
        assert!(is_content_change(&EventKind::Create(CreateKind::File)));
        assert!(is_content_change(&EventKind::Remove(RemoveKind::File)));
        assert!(is_content_change(&EventKind::Any));
    }

    /// テスト用の監視ループ。イベントの送り口と、コールバックが受け取った
    /// 変更パスの受け口、panic の合図を返す
    fn spawn_loop(
        panic_on_call: bool,
    ) -> (
        mpsc::Sender<notify::Result<Event>>,
        mpsc::Receiver<Vec<PathBuf>>,
        WatchFailure,
    ) {
        let (event_tx, event_rx) = mpsc::channel();
        let (called_tx, called_rx) = mpsc::channel();
        let (failure_tx, failure_rx) = oneshot::channel();
        let ignore = WatchIgnore::new(vec![PathBuf::from("/proj/dist")]);
        std::thread::spawn(move || {
            run_loop(
                &event_rx,
                &ignore,
                Duration::from_millis(50),
                move |changed| {
                    if panic_on_call {
                        panic!("テスト用の panic");
                    }
                    called_tx.send(changed.to_vec()).unwrap();
                },
                failure_tx,
            );
        });
        (event_tx, called_rx, WatchFailure { rx: failure_rx })
    }

    fn event(kind: EventKind, path: &str) -> notify::Result<Event> {
        Ok(Event::new(kind).add_path(PathBuf::from(path)))
    }

    #[test]
    fn 読んだだけのイベントではコールバックを呼ばない() {
        let (tx, called, _failure) = spawn_loop(false);
        tx.send(event(
            EventKind::Access(AccessKind::Open(AccessMode::Any)),
            "/proj/content/a.md",
        ))
        .unwrap();
        tx.send(event(
            EventKind::Access(AccessKind::Close(AccessMode::Read)),
            "/proj/content/a.md",
        ))
        .unwrap();
        assert!(called.recv_timeout(Duration::from_millis(300)).is_err());
    }

    #[test]
    fn 連続した変更は_1_回にまとめ_除外パスを落として渡す() {
        use notify::event::{DataChange, ModifyKind};
        let (tx, called, _failure) = spawn_loop(false);
        let modify = || EventKind::Modify(ModifyKind::Data(DataChange::Any));
        tx.send(event(modify(), "/proj/content/b.md")).unwrap();
        tx.send(event(modify(), "/proj/content/a.md")).unwrap();
        tx.send(event(modify(), "/proj/content/a.md")).unwrap();
        tx.send(event(modify(), "/proj/dist/index.html")).unwrap();
        let changed = called.recv_timeout(Duration::from_secs(5)).unwrap();
        assert_eq!(
            changed,
            vec![
                PathBuf::from("/proj/content/a.md"),
                PathBuf::from("/proj/content/b.md"),
            ]
        );
        assert!(called.recv_timeout(Duration::from_millis(300)).is_err());
    }

    #[test]
    fn コールバックの_panic_は合図で知らせて監視を止める() {
        use notify::event::{DataChange, ModifyKind};
        let (tx, _called, failure) = spawn_loop(true);
        tx.send(event(
            EventKind::Modify(ModifyKind::Data(DataChange::Any)),
            "/proj/content/a.md",
        ))
        .unwrap();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .unwrap();
        // timeout はランタイムの中で作る（外で作ると reactor が無く panic する）
        let message = runtime
            .block_on(async { tokio::time::timeout(Duration::from_secs(5), failure.wait()).await })
            .expect("panic の合図が届かない");
        assert!(message.contains("テスト用の panic"), "{message}");
    }

    #[test]
    fn 監視が正常に終わったときは合図を出さない() {
        let (tx, _called, failure) = spawn_loop(false);
        // 送り手が消える = watcher の drop と同じ。監視スレッドは抜けるが panic ではない
        drop(tx);
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .unwrap();
        let waited = runtime.block_on(async {
            tokio::time::timeout(Duration::from_millis(300), failure.wait()).await
        });
        assert!(waited.is_err(), "正常終了で合図が出た: {waited:?}");
    }

    /// 実際のファイルシステムで、読むだけでは呼ばれず書けば呼ばれることを確かめる
    /// （Linux では inotify の open イベントが届く経路そのもの）。
    /// `tempfile::tempdir()` の既定名は `.tmpXXXX` なので、ルートの祖先に隠し
    /// ディレクトリがある配置の確認も兼ねる
    #[test]
    fn 実ファイルを読むだけでは呼ばれず_書けば呼ばれる() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("proj");
        std::fs::create_dir(&root).unwrap();
        let root = root.canonicalize().unwrap();
        let file = root.join("a.md");
        std::fs::write(&file, "# a\n").unwrap();
        // 監視開始より前の作成イベントを拾わないよう間を置く（FSEvents の遅延対策）
        std::thread::sleep(Duration::from_millis(200));

        let (called_tx, called_rx) = mpsc::channel();
        let _handle = watch(
            std::slice::from_ref(&root),
            WatchIgnore::default(),
            Duration::from_millis(50),
            move |changed| {
                let _ = called_tx.send(changed.to_vec());
            },
        )
        .unwrap();

        for _ in 0..5 {
            let _ = std::fs::read_to_string(&file).unwrap();
        }
        assert!(
            called_rx.recv_timeout(Duration::from_millis(500)).is_err(),
            "読んだだけで呼ばれた"
        );

        std::fs::write(&file, "# a\n\n本文\n").unwrap();
        let changed = called_rx
            .recv_timeout(Duration::from_secs(5))
            .expect("書いても呼ばれない");
        assert!(changed.iter().any(|p| p.ends_with("a.md")), "{changed:?}");
    }

    #[test]
    fn 追加述語の除外も効く() {
        let ignore = WatchIgnore::new(vec![PathBuf::from("/proj/dist")])
            .with_extra(|path| path.to_string_lossy().contains("/target/"));
        assert!(ignore.is_ignored(Path::new("/proj/target/debug/yuzu")));
        assert!(!ignore.is_ignored(Path::new("/proj/content/target.md")));
    }
}
