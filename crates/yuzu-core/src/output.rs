//! インクリメンタルビルドの出力トラッキング。
//!
//! - [`write_if_changed`] — 内容一致なら書き込まない（mtime を汚さない。
//!   `yuzu fmt` の「差分なしなら書き込まない」と同じ思想）
//! - [`OutputTracker`] — このビルドで書き出した dist 相対パスを記録する
//! - [`remove_orphans`] — 前回マニフェストとの差分で、削除ページの古い出力だけ掃除する
//!
//! **出力先への書き込みは [`write_under`]、削除は [`remove_dir_all_under`] を必ず通す。**
//! どちらも [`resolve_output_rel`]（rel の字句検証）と
//! [`ensure_no_symlink_under`]（経路のリンク検査）を内包していて、
//! `root.join(rel)` を直に書くとこの 2 つが抜ける。
//! リンク検査は**出力ツリーの内部**（`dist/guide -> /outside`）まで見る必要があり、
//! 出力ルートまでの検査だけでは書き込み・削除がリンク先へ抜ける

use std::collections::BTreeSet;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::sync::Mutex;

/// dist 相対パスを検証して絶対パスへ結合する。
///
/// 絶対パス・`..`・`.`・空セグメント・`\` を含む rel を拒否する。
/// エイリアス（frontmatter 由来のユーザ入力）と出力マニフェスト（前回ビルドの JSON）の
/// 文字列がそのまま `root.join()` されるため、判定をここ 1 実装に寄せる
/// （`Path::join` は絶対パス引数で左辺を捨て、`.` はファイルシステムが吸収するので、
/// 文字列の一致比較だけでは実ページの上書きを防げない）。
pub fn resolve_output_rel(root: &Path, rel: &str) -> std::io::Result<PathBuf> {
    let invalid = |reason: &str| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("出力先として使えない相対パスです（{reason}）: {rel}"),
        )
    };
    if rel.is_empty() {
        return Err(invalid("空です"));
    }
    if rel.contains('\\') {
        return Err(invalid("区切りは / を使ってください"));
    }
    let mut path = root.to_path_buf();
    for seg in rel.split('/') {
        match seg {
            "" => return Err(invalid("空のセグメントを含みます")),
            "." => return Err(invalid(". セグメントを含みます")),
            ".." => return Err(invalid(".. セグメントを含みます")),
            _ => {}
        }
        // ドライブレター等（Windows の Prefix / RootDir）を弾く。
        // 単一の通常セグメントに解決できるものだけ通す
        let mut comps = Path::new(seg).components();
        match (comps.next(), comps.next()) {
            (Some(Component::Normal(s)), None) => path.push(s),
            _ => return Err(invalid("パス要素として使えません")),
        }
    }
    Ok(path)
}

/// `root` から `target` までの**各パス要素**にシンボリックリンクが無く、
/// `target` が `root` の真の子孫であることを確認する。
/// まだ存在しない末端要素は許容する（これから作るため）。
///
/// 出力先への**書き込み・削除の前に必ず通す**こと。canonicalize による
/// 「ルート配下か」の判定だけでは足りない:
///
/// - `dist -> ../outside` は `output.clean: false` やインクリメンタルビルドだと
///   削除経路を通らないので、書き込みだけが黙ってプロジェクト外へ出る
/// - `alias -> <ルート>` ＋ `output.dir: "alias/content"` は**リンク先がルート内**
///   なので配下判定を通り、原稿ディレクトリを破壊できる
/// - 最終要素だけ見る判定では中間要素のリンクを取りこぼす
///
/// リンクは**向き先に関わらず拒否する**（ルート内を指していても危険なため）。
///
/// ⚠️ 検査範囲は `root` 自身と、そこから `target` までの各要素。
/// **root の祖先は見ない**（macOS の `/tmp -> private/tmp` のように、
/// プロジェクトへ至る経路がリンクなのは正常なため）。
/// まだ存在しない要素は検査対象が無いので `Ok`（これから実体を作る）。
///
/// root 自身を含めるのは、公開 API（`build_search_index` / [`save_manifest`] 等）が
/// 呼び出し側の検証なしに基点を受け取れてしまうため。基点がリンクなら、
/// その下をいくら検査しても書き込みは全部リンク先の中に入る。
///
/// [`save_manifest`]: save_manifest
pub fn ensure_no_symlink_under(root: &Path, target: &Path) -> std::io::Result<()> {
    if target
        .strip_prefix(root)
        .is_ok_and(|rel| rel.as_os_str().is_empty())
    {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("出力先が基準ディレクトリ自身です: {}", root.display()),
        ));
    }
    ensure_symlink_free(root, target)
}

/// `target` が `root` 自身またはその配下で、`root` から `target` までの経路に
/// シンボリックリンクが無いことを確認する。
///
/// [`ensure_no_symlink_under`] の読み側（配信）版で、違いは **`target == root` を
/// 許す**こと（`preview` / `dev` は `/` の要求で基点ディレクトリ自身の metadata を
/// 引く）。検査範囲と「まだ存在しない要素は Ok」の規律は書き側と同じ = 書き側が
/// 拒否したリンクを読み側だけが辿ることがない。
/// `root` 自身がリンクなら `Ok` になる経路は無い（配下をいくら検査しても無意味なため）
pub fn ensure_symlink_free(root: &Path, target: &Path) -> std::io::Result<()> {
    let invalid = |message: String| std::io::Error::new(std::io::ErrorKind::InvalidInput, message);

    let rel = target.strip_prefix(root).map_err(|_| {
        invalid(format!(
            "{} が {} の外を指しています",
            target.display(),
            root.display()
        ))
    })?;
    if root
        .symlink_metadata()
        .is_ok_and(|m| m.file_type().is_symlink())
    {
        return Err(invalid(format!(
            "基準ディレクトリ {} がシンボリックリンクです（リンク先へは読み書きしません）",
            root.display()
        )));
    }

    let mut cur = root.to_path_buf();
    for comp in rel.components() {
        cur.push(comp);
        match cur.symlink_metadata() {
            Ok(meta) if meta.file_type().is_symlink() => {
                return Err(invalid(format!(
                    "経路にシンボリックリンクがあります: {}（リンク先へは読み書きしません）",
                    cur.display()
                )));
            }
            Ok(_) => {}
            // ここから下はまだ存在しない = 書き側はこれから実ディレクトリを作り、
            // 読み側は素の NotFound になる
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => break,
            Err(e) => return Err(e),
        }
    }
    Ok(())
}

/// [`ensure_no_symlink_under`] で安全を確認してからディレクトリを再帰削除する。
/// 削除したら `Ok(true)`、`dir` が無ければ何もせず `Ok(false)`。
///
/// 設定検証（yuzu-config が `output.dir` の絶対パス・`..`・入力ディレクトリとの
/// 重なりを拒否する）をすり抜けた場合の最後の防波堤
/// （設定の字句検証はシンボリックリンクの存在を見られない）。
pub fn remove_dir_all_under(root: &Path, dir: &Path) -> std::io::Result<bool> {
    ensure_no_symlink_under(root, dir)?;
    if !dir.exists() {
        return Ok(false);
    }
    fs::remove_dir_all(dir)?;
    Ok(true)
}

/// 書き込み結果（Unchanged = 内容一致でスキップ）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WriteOutcome {
    Written,
    Unchanged,
}

/// 内容が一致していれば書き込みをスキップする（mtime 温存）。
///
/// ⚠️ 経路検証をしないので、**出力ディレクトリへ書くときは直接使わない**
/// （[`write_under`] を通すこと）。単体で使ってよいのは出力先の外
/// （原稿の整形結果など、パスが利用者指定でない書き込み）だけ
pub fn write_if_changed(path: &Path, data: &[u8]) -> std::io::Result<WriteOutcome> {
    if let Ok(meta) = fs::metadata(path) {
        if meta.len() == data.len() as u64 && fs::read(path)? == data {
            return Ok(WriteOutcome::Unchanged);
        }
    }
    fs::write(path, data)?;
    Ok(WriteOutcome::Written)
}

/// `root` 配下の `rel`（`/` 区切り）へ安全に書き出す。解決したパスを返す。
///
/// **出力ディレクトリへ書くコードはすべてこれを通すこと。** 3 つの防御を
/// 1 実装に寄せている:
///
/// 1. `rel` の字句検証（[`resolve_output_rel`]。`..` や絶対パスを拒否）
/// 2. **最終パスまでの経路のリンク検査**（[`ensure_no_symlink_under`]）。
///    出力ルートまでを検査しても、`dist/guide -> /outside` のような
///    **出力ツリー内部**のリンクは別途見ないと書き込みが外へ抜ける
/// 3. 親ディレクトリ作成 ＋ compare-before-write（mtime 温存）
pub fn write_under(
    root: &Path,
    rel: &str,
    data: &[u8],
) -> std::io::Result<(PathBuf, WriteOutcome)> {
    let path = resolve_output_rel(root, rel)?;
    ensure_no_symlink_under(root, &path)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let outcome = write_if_changed(&path, data)?;
    Ok((path, outcome))
}

/// [`write_under`] の原子的版（tmp へ書いて rename）。
///
/// 使いどころは「**書き込み途中で中断されると、次回の読み手が壊れた内容を
/// 掴む**」ファイルだけ。`.yuzu/cache/global.json` は事実上のコミットレコードで、
/// これが健全なら配下のページキャッシュも整合する、という構造になっている。
///
/// 3 つの防御（字句検証・経路のリンク検査・親ディレクトリ作成）は [`write_under`]
/// と同じものを通し、**tmp 側のパスにもリンク検査をかける**。
/// compare-before-write（mtime 温存）はしない — 原子性のために必ず書き直すので、
/// mtime を温存したい出力（dist 配下）にはこちらを使わないこと
pub fn write_atomic_under(root: &Path, rel: &str, data: &[u8]) -> std::io::Result<PathBuf> {
    let path = resolve_output_rel(root, rel)?;
    ensure_no_symlink_under(root, &path)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    // 同一ディレクトリへ置く（rename が同一ファイルシステム内で完結する）。
    // 名前は固定でよい: 同じ出力先への同時ビルドは元々想定していない
    let tmp_rel = format!("{rel}.tmp");
    let tmp = resolve_output_rel(root, &tmp_rel)?;
    ensure_no_symlink_under(root, &tmp)?;
    fs::write(&tmp, data)?;
    fs::rename(&tmp, &path)?;
    Ok(path)
}

/// このビルドで書き出した dist 相対パスの記録（孤児掃除マニフェストの元）
pub struct OutputTracker {
    root: PathBuf,
    written: Mutex<BTreeSet<String>>,
}

impl OutputTracker {
    /// 出力先そのものがシンボリックリンクなら作れない
    /// （書き込みが全部リンク先へ素通りするため。呼び出し側の検証と重なるが、
    /// tracker を作れば必ず書けてしまう以上ここでも止める二重防御）
    pub fn new(root: &Path) -> std::io::Result<Self> {
        if root
            .symlink_metadata()
            .is_ok_and(|m| m.file_type().is_symlink())
        {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!(
                    "出力ディレクトリ {} はシンボリックリンクです",
                    root.display()
                ),
            ));
        }
        Ok(Self {
            root: root.to_path_buf(),
            written: Mutex::new(BTreeSet::new()),
        })
    }

    /// [`write_under`] ＋ 孤児掃除マニフェストへの記録
    pub fn write(&self, rel: &str, data: &[u8]) -> std::io::Result<WriteOutcome> {
        let (_, outcome) = write_under(&self.root, rel, data)?;
        self.written.lock().unwrap().insert(rel.to_string());
        Ok(outcome)
    }

    pub fn into_written(self) -> BTreeSet<String> {
        self.written.into_inner().unwrap()
    }
}

/// 前回書き出したが今回書き出さなかったファイルを削除し、
/// 空になったディレクトリを root 直前まで剪定する。削除件数を返す
pub fn remove_orphans(
    root: &Path,
    previous: &BTreeSet<String>,
    current: &BTreeSet<String>,
) -> std::io::Result<usize> {
    let mut removed = 0usize;
    let mut dirs: BTreeSet<PathBuf> = BTreeSet::new();
    for rel in previous.difference(current) {
        // 前回マニフェスト（JSON）は書き換えられうるので、削除前に検証する。
        // 経路のリンク検査も込み（`dist/guide -> /outside` を経由して
        // リンク先のファイルを消さないため）。
        // 不正な rel はビルドを落とさず読み飛ばす（掃除できないだけで害はない）
        let path = match resolve_output_rel(root, rel)
            .and_then(|path| ensure_no_symlink_under(root, &path).map(|()| path))
        {
            Ok(path) => path,
            Err(e) => {
                tracing::warn!(rel, "孤児掃除をスキップします: {e}");
                continue;
            }
        };
        match fs::remove_file(&path) {
            Ok(()) => removed += 1,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e),
        }
        let mut dir = path.parent();
        while let Some(d) = dir {
            if d == root {
                break;
            }
            dirs.insert(d.to_path_buf());
            dir = d.parent();
        }
    }
    // 深い順に空ディレクトリを剪定（非空は remove_dir が失敗するだけなので無視）
    for dir in dirs.iter().rev() {
        let _ = fs::remove_dir(dir);
    }
    Ok(removed)
}

/// 出力マニフェスト（前回書き出した dist 相対パス一覧）を読む。破損・不在は None
pub fn load_manifest(path: &Path) -> Option<BTreeSet<String>> {
    let bytes = fs::read(path).ok()?;
    let manifest: OutputManifest = serde_json::from_slice(&bytes).ok()?;
    (manifest.format_version == MANIFEST_FORMAT_VERSION).then_some(manifest.files)
}

/// マニフェストを書き出す。`path` の親（`.yuzu/cache`）を基点に
/// ファイル名だけを [`write_under`] へ渡し、リンク経由の書き込みを防ぐ
pub fn save_manifest(path: &Path, files: &BTreeSet<String>) -> std::io::Result<()> {
    let invalid = |m: &str| std::io::Error::new(std::io::ErrorKind::InvalidInput, m.to_string());
    let parent = path
        .parent()
        .ok_or_else(|| invalid("親ディレクトリがありません"))?;
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| invalid("ファイル名を解決できません"))?;
    fs::create_dir_all(parent)?;

    let manifest = OutputManifest {
        format_version: MANIFEST_FORMAT_VERSION,
        files: files.clone(),
    };
    write_under(parent, name, &serde_json::to_vec(&manifest)?)?;
    Ok(())
}

const MANIFEST_FORMAT_VERSION: u32 = 1;

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct OutputManifest {
    format_version: u32,
    files: BTreeSet<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn write_if_changed_は同一内容でスキップし_mtime_を温存する() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.txt");
        assert_eq!(
            write_if_changed(&path, b"hello").unwrap(),
            WriteOutcome::Written
        );
        let mtime1 = fs::metadata(&path).unwrap().modified().unwrap();

        std::thread::sleep(std::time::Duration::from_millis(20));
        assert_eq!(
            write_if_changed(&path, b"hello").unwrap(),
            WriteOutcome::Unchanged
        );
        let mtime2 = fs::metadata(&path).unwrap().modified().unwrap();
        assert_eq!(mtime1, mtime2, "スキップ時は mtime が変わらない");

        assert_eq!(
            write_if_changed(&path, b"world").unwrap(),
            WriteOutcome::Written
        );
        assert_eq!(fs::read(&path).unwrap(), b"world");
    }

    #[test]
    fn remove_orphans_は差分削除と空ディレクトリ剪定をする() {
        let dir = tempfile::tempdir().unwrap();
        let tracker = OutputTracker::new(dir.path()).unwrap();
        tracker.write("index.html", b"a").unwrap();
        tracker.write("old/index.html", b"b").unwrap();
        tracker.write("keep/index.html", b"c").unwrap();
        let previous = tracker.into_written();

        let tracker = OutputTracker::new(dir.path()).unwrap();
        tracker.write("index.html", b"a").unwrap();
        tracker.write("keep/other.html", b"d").unwrap();
        let current = tracker.into_written();

        let removed = remove_orphans(dir.path(), &previous, &current).unwrap();
        assert_eq!(removed, 2);
        assert!(!dir.path().join("old").exists(), "空ディレクトリは剪定");
        assert!(dir.path().join("keep").exists(), "非空ディレクトリは残す");
        assert!(dir.path().join("index.html").exists());
    }

    #[test]
    fn output_tracker_は危険な相対パスを拒否する() {
        let dir = tempfile::tempdir().unwrap();
        let tracker = OutputTracker::new(dir.path()).unwrap();
        // `.` はファイルシステムが吸収するため、文字列比較では実ページと
        // 別物に見えるのに同じファイルを指す（エイリアス `"."` の再現）
        for bad in [
            "./index.html",
            "../outside.html",
            "a/./b.html",
            "a/../b.html",
            "/etc/passwd",
            "a//b.html",
            "a\\b.html",
            "",
        ] {
            assert!(tracker.write(bad, b"x").is_err(), "拒否されるべき: {bad:?}");
        }
        assert!(tracker.write("a/b.html", b"x").is_ok());
        assert!(dir.path().join("a/b.html").exists());
    }

    #[test]
    fn remove_orphans_は危険な相対パスを飛ばして続行する() {
        let dir = tempfile::tempdir().unwrap();
        let outside = dir.path().join("outside.html");
        let root = dir.path().join("dist");
        fs::create_dir_all(&root).unwrap();
        fs::write(&outside, "守られるべきファイル".as_bytes()).unwrap();
        fs::write(root.join("stale.html"), b"x").unwrap();

        let previous: BTreeSet<String> =
            ["../outside.html".to_string(), "stale.html".to_string()].into();
        let removed = remove_orphans(&root, &previous, &BTreeSet::new()).unwrap();

        assert_eq!(removed, 1, "不正な rel は数えない");
        assert!(outside.exists(), "出力先の外は消さない");
        assert!(!root.join("stale.html").exists());
    }

    #[cfg(unix)]
    #[test]
    fn ensure_no_symlink_under_は末端のリンクを拒否する() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("proj");
        let outside = dir.path().join("outside");
        fs::create_dir_all(&root).unwrap();
        fs::create_dir_all(&outside).unwrap();
        std::os::unix::fs::symlink(&outside, root.join("dist")).unwrap();

        let err = ensure_no_symlink_under(&root, &root.join("dist")).unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
    }

    /// 最終要素だけ見る判定では取りこぼす形。リンク先がルート**内**でも拒否する
    #[cfg(unix)]
    #[test]
    fn ensure_no_symlink_under_は中間要素のリンクを拒否する() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fs::create_dir_all(root.join("content")).unwrap();
        std::os::unix::fs::symlink(root, root.join("alias")).unwrap();

        let err = ensure_no_symlink_under(root, &root.join("alias/content")).unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
    }

    #[test]
    fn ensure_no_symlink_under_は未作成の末端を許容する() {
        let dir = tempfile::tempdir().unwrap();
        // dist も その下も まだ無い（これから作る）
        ensure_no_symlink_under(dir.path(), &dir.path().join("dist")).unwrap();
        ensure_no_symlink_under(dir.path(), &dir.path().join("a/b/c")).unwrap();
    }

    #[test]
    fn ensure_no_symlink_under_はルート自身とルート外を拒否する() {
        let dir = tempfile::tempdir().unwrap();
        assert!(ensure_no_symlink_under(dir.path(), dir.path()).is_err());
        assert!(ensure_no_symlink_under(dir.path(), Path::new("/etc")).is_err());
    }

    /// macOS の `/tmp -> private/tmp` 相当。ルートへ至る経路のリンクは
    /// 検査対象外にしないと、tempdir 上の全テストと実プロジェクトが落ちる
    #[cfg(unix)]
    #[test]
    fn ensure_no_symlink_under_は祖先のリンクを問題にしない() {
        // macOS の `/tmp -> private/tmp` 相当。リンクは**基点の祖先**であって
        // 基点自身ではない（cwd は getcwd で解決済みなので root は常に実体）。
        // 祖先まで検査すると tempdir 上の全テストと実プロジェクトが落ちる
        let dir = tempfile::tempdir().unwrap();
        let real = dir.path().join("real");
        fs::create_dir_all(real.join("proj/dist")).unwrap();
        let ancestor_link = dir.path().join("link");
        std::os::unix::fs::symlink(&real, &ancestor_link).unwrap();

        let root = ancestor_link.join("proj");
        ensure_no_symlink_under(&root, &root.join("dist")).unwrap();
    }

    #[test]
    fn ensure_no_symlink_under_はルート未作成でも通る() {
        // save_manifest / キャッシュ保存は「作る前」に検証するため
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("not-yet");
        ensure_no_symlink_under(&missing, &missing.join("a/b.json")).unwrap();
    }

    /// 出力ツリー**内部**のリンク。ルートまでを検査しても、
    /// `dist/guide -> /outside` があると書き込みがリンク先へ抜ける
    #[cfg(unix)]
    #[test]
    fn write_under_は出力ツリー内部のリンクを拒否する() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("dist");
        let outside = dir.path().join("outside");
        fs::create_dir_all(&root).unwrap();
        fs::create_dir_all(&outside).unwrap();
        std::os::unix::fs::symlink(&outside, root.join("guide")).unwrap();

        let err = write_under(&root, "guide/index.html", b"x").unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
        assert!(!outside.join("index.html").exists(), "リンク先へ書かない");

        // 出力ファイル自体がリンクの場合も同じ
        std::os::unix::fs::symlink(outside.join("t.html"), root.join("t.html")).unwrap();
        assert!(write_under(&root, "t.html", b"x").is_err());
        assert!(!outside.join("t.html").exists());
    }

    #[cfg(unix)]
    #[test]
    fn write_under_は基点自身がリンクなら拒否する() {
        let dir = tempfile::tempdir().unwrap();
        let outside = dir.path().join("outside");
        std::fs::create_dir_all(&outside).unwrap();
        let root = dir.path().join("dist");
        std::os::unix::fs::symlink(&outside, &root).unwrap();

        assert!(write_under(&root, "index.html", b"x").is_err());
        assert!(!outside.join("index.html").exists(), "リンク先へ書かない");
    }

    /// 孤児掃除もリンク経由でリンク先のファイルを消してはいけない
    #[cfg(unix)]
    #[test]
    fn remove_orphans_は内部リンク経由の削除をしない() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("dist");
        let outside = dir.path().join("outside");
        fs::create_dir_all(&root).unwrap();
        fs::create_dir_all(&outside).unwrap();
        fs::write(outside.join("keep.html"), "守られるべきファイル".as_bytes()).unwrap();
        std::os::unix::fs::symlink(&outside, root.join("guide")).unwrap();

        let previous: BTreeSet<String> = ["guide/keep.html".to_string()].into();
        let removed = remove_orphans(&root, &previous, &BTreeSet::new()).unwrap();

        assert_eq!(removed, 0);
        assert!(outside.join("keep.html").exists(), "リンク先は消さない");
    }

    #[cfg(unix)]
    #[test]
    fn output_tracker_は出力先がリンクなら作れない() {
        let dir = tempfile::tempdir().unwrap();
        let outside = dir.path().join("outside");
        fs::create_dir_all(&outside).unwrap();
        let link = dir.path().join("dist");
        std::os::unix::fs::symlink(&outside, &link).unwrap();

        assert!(OutputTracker::new(&link).is_err());
    }

    #[test]
    fn remove_dir_all_under_はルート配下だけ削除する() {
        let dir = tempfile::tempdir().unwrap();
        let out = dir.path().join("dist");
        fs::create_dir_all(out.join("nested")).unwrap();
        fs::write(out.join("nested/a.html"), b"x").unwrap();

        assert!(remove_dir_all_under(dir.path(), &out).unwrap());
        assert!(!out.exists());
        // 不在なら何もせず false
        assert!(!remove_dir_all_under(dir.path(), &out).unwrap());
    }

    #[test]
    fn remove_dir_all_under_はルート自身を拒否する() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("yuzu.toml"), b"").unwrap();

        let err = remove_dir_all_under(dir.path(), dir.path()).unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
        assert!(dir.path().join("yuzu.toml").exists());
    }

    /// canonicalize はリンクを追うため、ルート**内**へ向いたリンクは
    /// 「配下」の判定を通ってしまう（`dist -> content` で原稿が消える）
    #[cfg(unix)]
    #[test]
    fn remove_dir_all_under_はルート内を指すシンボリックリンクも拒否する() {
        let dir = tempfile::tempdir().unwrap();
        let content = dir.path().join("content");
        fs::create_dir_all(&content).unwrap();
        fs::write(content.join("index.md"), "守られるべきファイル".as_bytes()).unwrap();

        let link = dir.path().join("dist");
        std::os::unix::fs::symlink(&content, &link).unwrap();

        let err = remove_dir_all_under(dir.path(), &link).unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
        assert!(content.join("index.md").exists(), "原稿が残っている");
    }

    #[cfg(unix)]
    #[test]
    fn remove_dir_all_under_はシンボリックリンクの脱出を拒否する() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("proj");
        let outside = dir.path().join("outside");
        fs::create_dir_all(&root).unwrap();
        fs::create_dir_all(&outside).unwrap();
        fs::write(outside.join("keep.txt"), "守られるべきファイル".as_bytes()).unwrap();

        let link = root.join("dist");
        std::os::unix::fs::symlink(&outside, &link).unwrap();

        let err = remove_dir_all_under(&root, &link).unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
        assert!(outside.join("keep.txt").exists());
    }

    #[test]
    fn マニフェストのラウンドトリップと破損フォールバック() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("manifest.json");
        let files: BTreeSet<String> = ["a.html".to_string(), "b/c.html".to_string()].into();
        save_manifest(&path, &files).unwrap();
        assert_eq!(load_manifest(&path).unwrap(), files);

        fs::write(&path, b"{ broken").unwrap();
        assert!(load_manifest(&path).is_none());
        assert!(load_manifest(&dir.path().join("nothing.json")).is_none());
    }

    #[test]
    fn write_atomic_under_は_tmp_を残さない() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_atomic_under(dir.path(), "cache/global.json", b"{}").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"{}");
        // 中間ファイルが残っていない（次回の load や孤児掃除が拾わない）
        assert!(!dir.path().join("cache/global.json.tmp").exists());
    }

    #[test]
    fn write_atomic_under_は既存を置き換える() {
        let dir = tempfile::tempdir().unwrap();
        write_atomic_under(dir.path(), "a.json", b"old").unwrap();
        let path = write_atomic_under(dir.path(), "a.json", b"new").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"new");
    }

    #[test]
    fn write_atomic_under_も経路を検証する() {
        let dir = tempfile::tempdir().unwrap();
        // ルート外・絶対パスは write_under と同じ規律で拒否する
        assert!(write_atomic_under(dir.path(), "../outside.json", b"x").is_err());
        assert!(write_atomic_under(dir.path(), "/abs.json", b"x").is_err());
    }
}
