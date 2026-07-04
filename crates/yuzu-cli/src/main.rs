//! yuzu CLI のエントリポイント

mod cli;
mod commands;

// 依存方向（cli → index）の配線。Phase 3 で実体を使う
use yuzu_index as _;

use clap::Parser;
use tracing_subscriber::EnvFilter;

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .with_target(false)
        .init();

    let cli = cli::Cli::parse();
    match cli.command {
        cli::Command::New { dir } => commands::new::run(&dir),
        cli::Command::Build { watch } => commands::build::run(watch),
        cli::Command::Preview { port } => commands::preview::run(port),
        cli::Command::Dev => commands::stubs::not_implemented("dev"),
        cli::Command::Search => commands::stubs::not_implemented("search"),
        cli::Command::Llms => commands::stubs::not_implemented("llms"),
    }
}
