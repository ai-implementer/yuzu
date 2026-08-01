//! yuzu CLI のエントリポイント
// 標準出力は out モジュールに集約する（SIGPIPE で panic しないため。out.rs 参照）。
// print! / println! を書いた瞬間に clippy が落とすので、規律が機械的に守られる
#![deny(clippy::print_stdout)]

mod cli;
mod commands;
mod out;

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
        // ログは必ず stderr へ。tracing-subscriber の既定は stdout で、
        // `yuzu check --format json` の「標準出力へ JSON 以外を書かない」契約を破る
        // （yuzu.jsonc の重複キー警告が JSON の前に出てパースが失敗していた）
        .with_writer(std::io::stderr)
        .init();

    let code = match run(cli::Cli::parse()) {
        Ok(code) => code,
        Err(err) => {
            eprintln!("Error: {err:?}");
            ExitCode::from(2)
        }
    };
    // 標準出力の I/O エラー（ディスクフル等）は実行エラー扱い。
    // 下流が閉じただけ（BrokenPipe）は成功で、コマンド本来の終了コードを保つ
    if let Err(err) = out::finish() {
        eprintln!("Error: 標準出力へ書き出せません: {err}");
        return ExitCode::from(2);
    }
    code
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
            port,
            host,
        } => commands::build::run(watch, base_url, force, drafts, port, host).map(ok),
        cli::Command::Preview { port, host } => commands::preview::run(port, host).map(ok),
        cli::Command::Dev {
            port,
            host,
            force,
            drafts,
        } => commands::dev::run(port, host, force, drafts).map(ok),
        cli::Command::Search {
            query,
            limit,
            section,
            json,
        } => commands::search::run(&query, limit, &section, json).map(ok),
        cli::Command::Llms { full } => commands::llms::run(full).map(ok),
        cli::Command::Fmt { check, diff } => commands::fmt::run(check, diff),
        cli::Command::Lint { fix, format } => commands::lint::run(fix, format),
        cli::Command::Check { format } => commands::check::run(format),
    }
}
