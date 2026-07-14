//! yuzu CLI のエントリポイント

mod cli;
mod commands;

// 依存方向（cli → index）の配線。Phase 3 で実体を使う
use yuzu_index as _;

use std::process::ExitCode;

use clap::Parser;
use tracing_subscriber::EnvFilter;

/// 終了コード規約（grep 流）:
/// 0 = 成功（違反なし）/ 1 = fmt・lint・check の違反あり / 2 = 実行エラー
fn main() -> ExitCode {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .with_target(false)
        .init();

    match run(cli::Cli::parse()) {
        Ok(code) => code,
        Err(err) => {
            eprintln!("Error: {err:?}");
            ExitCode::from(2)
        }
    }
}

fn run(cli: cli::Cli) -> anyhow::Result<ExitCode> {
    let ok = |()| ExitCode::SUCCESS;
    match cli.command {
        cli::Command::New { dir } => commands::new::run(&dir).map(ok),
        cli::Command::Build {
            watch,
            base_url,
            force,
            drafts,
        } => commands::build::run(watch, base_url, force, drafts).map(ok),
        cli::Command::Preview { port, host } => commands::preview::run(port, host).map(ok),
        cli::Command::Dev {
            port,
            host,
            force,
            drafts,
        } => commands::dev::run(port, host, force, drafts).map(ok),
        cli::Command::Search { query, limit, json } => {
            commands::search::run(&query, limit, json).map(ok)
        }
        cli::Command::Llms { full } => commands::llms::run(full).map(ok),
        cli::Command::Fmt { check } => commands::fmt::run(check),
        cli::Command::Lint => commands::lint::run(),
        cli::Command::Check => commands::check::run(),
    }
}
