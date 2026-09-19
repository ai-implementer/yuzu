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
