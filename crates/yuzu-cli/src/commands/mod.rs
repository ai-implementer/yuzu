use anyhow::Context;
use yuzu_config::ResolvedConfig;

use crate::cx::Cx;

pub mod build;
pub mod check;
pub mod completions;
pub mod dev;
pub mod diag;
pub mod extlink;
pub mod fmt;
pub mod lint;
pub mod llms;
pub mod new;
pub mod preview;
pub mod search;

/// プロジェクトルートを確定して設定を読む（全コマンド共通の入口）。
///
/// `--root` があればそこを使い、**上方向探索はしない**（探索すると
/// 「指定したのに親の `yuzu.toml` を拾う」事故になる）。無指定なら従来どおり
/// cwd から上方向に `yuzu.toml` を探す。
///
/// 設定エラー（構文・未知キー・型不一致）は位置付きのエラーで止まる（exit 2）。
/// 読み込みは成功するが注意が要る設定（`input.dir` がルート外など）は警告ログへ出す
/// — yuzu-config はログを出さないので表示はここの責務
/// （`lint` / `check` はこれに加えて診断としても報告する）
pub(crate) fn load_project(cx: &Cx) -> anyhow::Result<ResolvedConfig> {
    // `--root` 側だけ正規化しているように見えるが非対称ではない。
    // `current_dir()` はシンボリックリンク解決済みのパスを返すので、
    // 探索経路のルートは既に正規化済みの絶対パスになっている
    let root = match cx.root() {
        Some(root) => root.to_path_buf(),
        None => {
            let cwd = std::env::current_dir().context("カレントディレクトリを取得できません")?;
            yuzu_config::find_project_root(&cwd)?
        }
    };
    let rc = yuzu_config::load(&root)?;
    warn_config_diagnostics(&rc);
    Ok(rc)
}

/// `ResolvedConfig::diagnostics` を警告ログに出す（`load_project` と watch 中の
/// 設定再読み込みが共有する）
pub(crate) fn warn_config_diagnostics(rc: &ResolvedConfig) {
    for d in &rc.diagnostics {
        tracing::warn!(
            "{}:{}:{}: {}",
            yuzu_config::CONFIG_FILE_NAME,
            d.line,
            d.col,
            d.message
        );
    }
}

/// 原稿（`.md`）を書き換える（`yuzu fmt` と `lint --fix` が通る唯一の書き込み口。Phase 85）。
///
/// - 同じディレクトリの一時ファイルへ書いてから rename する。書き込みの途中で止まっても
///   原稿が半端な内容で残らない。パーミッションは元のファイルから引き継ぐ
///   （シンボリックリンクの `.md` はページにならないので、リンクを実ファイルで置き換える
///   ことは無い。ハードリンクは切れる）
/// - 書く直前に、ディスクの内容が読み込んだとき（`expected`）から変わっていないかを
///   照合する。変わっていたら書かずにエラーにする（エディタの保存と重なったときに、
///   古い内容から作った結果で上書きしない）
pub(crate) fn rewrite_source(
    path: &std::path::Path,
    expected: &str,
    new: &str,
) -> anyhow::Result<()> {
    let current = std::fs::read_to_string(path)
        .with_context(|| format!("{} を読み込めません", path.display()))?;
    if current != expected {
        anyhow::bail!(
            "{} は読み込んだ後に変更されています。書き込みを中止しました（もう一度実行してください）",
            path.display()
        );
    }
    let name = path
        .file_name()
        .with_context(|| format!("{} はファイルではありません", path.display()))?;
    let tmp = path.with_file_name(format!(".{}.yuzu-tmp", name.to_string_lossy()));
    let write = || -> std::io::Result<()> {
        let permissions = std::fs::metadata(path)?.permissions();
        std::fs::write(&tmp, new)?;
        std::fs::set_permissions(&tmp, permissions)?;
        std::fs::rename(&tmp, path)
    };
    write().map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        anyhow::Error::new(e).context(format!("{} に書き込めません", path.display()))
    })
}

#[cfg(test)]
mod tests {
    use super::rewrite_source;

    #[test]
    fn 原稿の書き換えは一時ファイルを残さずパーミッションを引き継ぐ() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.md");
        std::fs::write(&path, "old\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o640)).unwrap();
        }

        rewrite_source(&path, "old\n", "new\n").unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "new\n");
        let names: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(names, ["a.md"], "一時ファイルが残らない");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&path).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o640);
        }
    }

    /// 読み込んだ後にエディタ等で変わった原稿は、古い内容から作った結果で上書きしない
    #[test]
    fn 読み込み後に変わった原稿は書き換えない() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.md");
        std::fs::write(&path, "edited\n").unwrap();

        let err = rewrite_source(&path, "old\n", "new\n").unwrap_err();
        assert!(err.to_string().contains("変更されています"), "{err}");
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "edited\n");
    }
}
