use std::path::PathBuf;

use anyhow::Context;
use yuzu_config::ResolvedConfig;

pub mod build;
pub mod check;
pub mod dev;
pub mod diag;
pub mod extlink;
pub mod fmt;
pub mod lint;
pub mod llms;
pub mod new;
pub mod preview;
pub mod search;

/// cwd から上方向に `yuzu.toml` を探してプロジェクトルートを確定し、設定を読む
/// （全コマンド共通の入口）。設定エラー（構文・未知キー・型不一致）は位置付きの
/// エラーで止まる（exit 2）。読み込みは成功するが注意が要る設定（`input.dir` が
/// ルート外など）は警告ログへ出す — yuzu-config はログを出さないので表示はここの
/// 責務（`lint` / `check` はこれに加えて診断としても報告する）
pub(crate) fn load_project() -> anyhow::Result<(PathBuf, ResolvedConfig)> {
    let cwd = std::env::current_dir().context("カレントディレクトリを取得できません")?;
    let root = yuzu_config::find_project_root(&cwd)?;
    let rc = yuzu_config::load(&root)?;
    warn_config_diagnostics(&rc);
    Ok((root, rc))
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
