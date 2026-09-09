//! CLI の実行文脈（グローバル引数の解決結果）。
//!
//! サブコマンドをまたいで効く指定（現在は `--root`）を 1 つの器にまとめ、
//! 各 `run()` へ第 1 引数で渡す。**受け口をここ 1 箇所にする**のが目的で、
//! オプションを足すときに 9 つの `run()` を個別に触らずに済む。

use std::path::{Path, PathBuf};

use anyhow::{Context, bail};

/// 実行文脈。**`Cx` を持っている = `root` は正規化済みの絶対パス**という不変条件を
/// 型で保証する（正規化は [`Cx::new`] だけが行う）。
///
/// 相対パスのまま流すと `ResolvedConfig` の `root` / `content_dir` / `output_dir` から
/// 診断のパス表示まで相対のまま伝播し、さらに `root` 自身がシンボリックリンクだと
/// `.yuzu` のリンク検査を通る build・dev だけが落ちる非対称になる
#[derive(Debug)]
pub(crate) struct Cx {
    root: Option<PathBuf>,
}

impl Cx {
    /// `--root` の唯一の受け口。ここで 1 回だけ正規化する
    pub(crate) fn new(root: Option<PathBuf>) -> anyhow::Result<Self> {
        let Some(raw) = root else {
            return Ok(Self { root: None });
        };
        // エラー文言には利用者が打った綴り（`raw`）を出す。
        // canonicalize は Windows で `\\?\C:\...` の UNC 形式を返すため
        let resolved = raw
            .canonicalize()
            .with_context(|| format!("--root に指定した {} を開けません", raw.display()))?;
        // canonicalize はファイルでも成功する
        if !resolved.is_dir() {
            bail!(
                "--root にはディレクトリを指定してください: {}",
                raw.display()
            );
        }
        Ok(Self {
            root: Some(resolved),
        })
    }

    /// `--root` の指定（正規化済み絶対パス）。無指定なら `None`
    pub(crate) fn root(&self) -> Option<&Path> {
        self.root.as_deref()
    }
}

#[cfg(test)]
mod tests {
    use super::Cx;

    /// `--root` 無指定は従来どおり（cwd からの上方向探索へ回る）
    #[test]
    fn 無指定なら_none() {
        assert!(Cx::new(None).unwrap().root().is_none());
    }

    /// 正規化済み絶対パスになる。
    /// cwd を変える相対パスのテストは並列実行の他テストへ影響するので行わず、
    /// `..` を含むパスの正規化と絶対性で不変条件を確かめる
    #[test]
    fn 正規化済みの絶対パスになる() {
        let tmp = tempfile::tempdir().unwrap();
        let sub = tmp.path().join("sub");
        std::fs::create_dir(&sub).unwrap();

        let cx = Cx::new(Some(tmp.path().join("sub/../sub"))).unwrap();
        let root = cx.root().unwrap();
        assert!(root.is_absolute(), "{}", root.display());
        assert_eq!(root, sub.canonicalize().unwrap());
    }

    /// 存在しないディレクトリは実行エラー。文言に打った綴りが出る
    #[test]
    fn 存在しないディレクトリはエラーになる() {
        let tmp = tempfile::tempdir().unwrap();
        let missing = tmp.path().join("no-such-dir");
        let err = Cx::new(Some(missing)).unwrap_err();
        assert!(err.to_string().contains("no-such-dir"), "{err}");
    }

    /// canonicalize はファイルでも成功するので、ディレクトリ検査が要る
    #[test]
    fn ファイルを指定するとエラーになる() {
        let tmp = tempfile::tempdir().unwrap();
        let file = tmp.path().join("yuzu.toml");
        std::fs::write(&file, "").unwrap();
        let err = Cx::new(Some(file)).unwrap_err();
        assert!(err.to_string().contains("ディレクトリ"), "{err}");
    }

    /// ルート自身がシンボリックリンクでも実体へ解決する。
    /// 解決しないと `.yuzu` のリンク検査を通る build・dev だけが落ちる
    #[cfg(unix)]
    #[test]
    fn ルート自身がリンクでも実体へ解決する() {
        let tmp = tempfile::tempdir().unwrap();
        let real = tmp.path().join("real");
        let link = tmp.path().join("link");
        std::fs::create_dir(&real).unwrap();
        std::os::unix::fs::symlink(&real, &link).unwrap();

        let cx = Cx::new(Some(link)).unwrap();
        assert_eq!(cx.root().unwrap(), real.canonicalize().unwrap());
    }
}
