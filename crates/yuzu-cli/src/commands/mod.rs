use anyhow::Context;
use yuzu_config::ResolvedConfig;

use crate::cx::Cx;

pub mod build;
pub mod check;
mod collect;
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
    let rc = read_config(&root)?;
    warn_config_diagnostics(&rc);
    Ok(rc)
}

/// `yuzu.toml` を読み、yuzu-config が知らない規則（yuzu-core・yuzu-render が持つ）でも値を
/// 検証する。問題は位置付きの設定エラー（exit 2）になる。
///
/// **設定を読むのはここだけ**（[`load_project`] と watch 中の読み直しの両方が通る）。
/// 以前は不正な値を使う側が黙って捨てていて、`search.page = "../search"` で検索結果
/// ページが出ないのに build も check も成功していた（10-12 レビューの R3）
pub(crate) fn read_config(
    root: &std::path::Path,
) -> Result<ResolvedConfig, yuzu_config::ConfigError> {
    yuzu_config::load_with(root, setting_problems)
}

/// [`read_config`] の値の検証。規則は持たず、それぞれの規則の持ち主を呼ぶ
fn setting_problems(config: &yuzu_config::Config) -> Vec<yuzu_config::SettingProblem> {
    let mut problems = Vec::new();
    let mut check = |key_path: &'static str, raw: &str, problem: Option<&str>| {
        if let Some(reason) = problem {
            problems.push(yuzu_config::SettingProblem {
                key_path,
                message: format!("`{key_path}` の値 `{raw}` は使えません。{reason}"),
            });
        }
    };
    // 合成ページの route（空はページを作らない設定）
    for (key_path, raw) in [
        ("search.page", &config.search.page),
        ("markdown.glossary.page", &config.markdown.glossary.page),
    ] {
        check(key_path, raw, yuzu_core::urlpath::synth_page_problem(raw));
    }
    let levels = &config.theme.toc.levels;
    check(
        "theme.toc.levels",
        levels,
        yuzu_render::toc_levels_problem(levels),
    );
    problems
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
    let permissions = std::fs::metadata(path)
        .with_context(|| format!("{} を読み込めません", path.display()))?
        .permissions();
    let (tmp, file) = create_temp_beside(path)
        .with_context(|| format!("{} の一時ファイルを作れません", path.display()))?;
    // 書き込みも権限の設定も、作ったファイルのハンドルに対して行う（パスを開き直さない）
    let write = || -> std::io::Result<()> {
        use std::io::Write;
        let mut file = file;
        file.write_all(new.as_bytes())?;
        file.set_permissions(permissions)?;
        file.sync_all()?;
        drop(file);
        std::fs::rename(&tmp, path)
    };
    write().map_err(|e| {
        // 消すのは自分で作った一時ファイルだけ
        let _ = std::fs::remove_file(&tmp);
        anyhow::Error::new(e).context(format!("{} に書き込めません", path.display()))
    })
}

/// 原稿と同じディレクトリに一時ファイルを**排他的に**作る（`create_new`。既にある名前は、
/// ファイルでもシンボリックリンクでも開かずに次の名前へ進む）。固定名へ `fs::write` すると、
/// そこに置かれたリンクを辿ってリンク先を上書きし、rename で原稿がそのリンクに置き換わって
/// いた（PR #47 のレビュー指摘）。
///
/// Unix では作った時点から本人だけが読み書きできる権限（600）にする。既定の権限（umask 022 なら
/// 644）で作ると、本文を書いてから元の権限へ戻すまでの間、非公開の原稿を他のユーザが読める
/// （PR #47 の再レビューの指摘）。Windows の権限はディレクトリから引き継ぐ ACL なので扱わない
fn create_temp_beside(
    path: &std::path::Path,
) -> std::io::Result<(std::path::PathBuf, std::fs::File)> {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    for n in 0..100 {
        let tmp = path.with_file_name(format!(".{name}.yuzu-tmp-{n}"));
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
        match options.open(&tmp) {
            Ok(file) => return Ok((tmp, file)),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e),
        }
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::AlreadyExists,
        "使える一時ファイルの名前がありません（同じ名前の一時ファイルが 100 個あります）",
    ))
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

    /// 一時ファイルの名前に置かれたシンボリックリンクを辿らない（リンク先を上書きせず、
    /// 原稿もリンクに置き換わらない。自分が作っていないリンクは消さない）。PR #47 の指摘
    #[cfg(unix)]
    #[test]
    fn 一時ファイルの名前に置かれたリンクを辿らない() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.md");
        let victim = dir.path().join("victim.txt");
        std::fs::write(&path, "old\n").unwrap();
        std::fs::write(&victim, "victim\n").unwrap();
        let planted = dir.path().join(".a.md.yuzu-tmp-0");
        std::os::unix::fs::symlink(&victim, &planted).unwrap();

        rewrite_source(&path, "old\n", "new\n").unwrap();
        assert_eq!(std::fs::read_to_string(&victim).unwrap(), "victim\n");
        let meta = std::fs::symlink_metadata(&path).unwrap();
        assert!(meta.file_type().is_file(), "原稿はリンクに置き換わらない");
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "new\n");
        assert!(
            std::fs::symlink_metadata(&planted)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        assert!(
            !dir.path().join(".a.md.yuzu-tmp-1").exists(),
            "自分の一時ファイルは残さない"
        );
    }

    /// 一時ファイルは作った時点から本人だけが読める（本文を書く前に他のユーザから読めない）。
    /// PR #47 の再レビューの指摘
    #[cfg(unix)]
    #[test]
    fn 一時ファイルは作った時点から本人だけが読める() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.md");
        std::fs::write(&path, "old\n").unwrap();

        let (tmp, _file) = super::create_temp_beside(&path).unwrap();
        let mode = std::fs::metadata(&tmp).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
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

    /// 10-12 レビューの R3: yuzu-core・yuzu-render の規則で不正な値は、位置付きの設定エラー
    /// （全件）になる。以前は黙って捨てられ、build も check も成功していた
    #[test]
    fn 合成ページの_route_と_toc_の範囲の不正値は位置付きの設定エラー() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("yuzu.toml"),
            "[search]\npage = \"../search\"\n\n[markdown.glossary]\npage = \"a//b\"\n\n[theme.toc]\nlevels = \"3-2\"\n",
        )
        .unwrap();
        let err = super::read_config(dir.path()).unwrap_err();
        let yuzu_config::ConfigError::Invalid { issues, .. } = &err else {
            panic!("Invalid を期待: {err}");
        };
        let got: Vec<(&str, usize, usize)> = issues
            .iter()
            .map(|i| (i.key_path.as_str(), i.line, i.col))
            .collect();
        assert_eq!(
            got,
            [
                ("search.page", 2, 1),
                ("markdown.glossary.page", 5, 1),
                ("theme.toc.levels", 8, 1)
            ]
        );
        assert!(
            issues[0]
                .message
                .contains("`search.page` の値 `../search` は使えません"),
            "{}",
            issues[0].message
        );

        // 空（ページを作らない）と正しい値は通る
        std::fs::write(
            dir.path().join("yuzu.toml"),
            "[search]\npage = \"\"\n\n[markdown.glossary]\npage = \"help/terms\"\n\n[theme.toc]\nlevels = \"2-4\"\n",
        )
        .unwrap();
        super::read_config(dir.path()).unwrap();
    }
}
