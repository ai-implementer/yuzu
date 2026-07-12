//! インクリメンタルビルドの出力トラッキング。
//!
//! - [`write_if_changed`] — 内容一致なら書き込まない（mtime を汚さない。
//!   `yuzu fmt` の「差分なしなら書き込まない」と同じ思想）
//! - [`OutputTracker`] — このビルドで書き出した dist 相対パスを記録する
//! - [`remove_orphans`] — 前回マニフェストとの差分で、削除ページの古い出力だけ掃除する

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// 書き込み結果（Unchanged = 内容一致でスキップ）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WriteOutcome {
    Written,
    Unchanged,
}

/// 内容が一致していれば書き込みをスキップする（mtime 温存）
pub fn write_if_changed(path: &Path, data: &[u8]) -> std::io::Result<WriteOutcome> {
    if let Ok(meta) = fs::metadata(path) {
        if meta.len() == data.len() as u64 && fs::read(path)? == data {
            return Ok(WriteOutcome::Unchanged);
        }
    }
    fs::write(path, data)?;
    Ok(WriteOutcome::Written)
}

/// このビルドで書き出した dist 相対パスの記録（孤児掃除マニフェストの元）
pub struct OutputTracker {
    root: PathBuf,
    written: Mutex<BTreeSet<String>>,
}

impl OutputTracker {
    pub fn new(root: &Path) -> Self {
        Self {
            root: root.to_path_buf(),
            written: Mutex::new(BTreeSet::new()),
        }
    }

    /// `create_dir_all` ＋ compare-before-write ＋ 記録
    pub fn write(&self, rel: &str, data: &[u8]) -> std::io::Result<WriteOutcome> {
        let path = self.root.join(rel);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let outcome = write_if_changed(&path, data)?;
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
        let path = root.join(rel);
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

pub fn save_manifest(path: &Path, files: &BTreeSet<String>) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let manifest = OutputManifest {
        format_version: MANIFEST_FORMAT_VERSION,
        files: files.clone(),
    };
    fs::write(path, serde_json::to_vec(&manifest)?)
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
        let tracker = OutputTracker::new(dir.path());
        tracker.write("index.html", b"a").unwrap();
        tracker.write("old/index.html", b"b").unwrap();
        tracker.write("keep/index.html", b"c").unwrap();
        let previous = tracker.into_written();

        let tracker = OutputTracker::new(dir.path());
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
}
