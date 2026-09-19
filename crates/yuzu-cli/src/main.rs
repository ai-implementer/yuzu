//! yuzu CLI のエントリポイント
// 標準出力は out モジュールに集約する（SIGPIPE で panic しないため。out.rs 参照）。
// print! / println! を書いた瞬間に clippy が落とすので、規律が機械的に守られる
#![deny(clippy::print_stdout)]

mod cli;
mod commands;
mod cx;
mod out;

// 依存方向（cli → index）の配線。Phase 3 で実体を使う
use yuzu_index as _;

use std::process::ExitCode;

use tracing_subscriber::EnvFilter;

/// 終了コード規約（grep 流）:
/// 0 = 成功（違反なし）/ 1 = fmt・lint・check の違反あり / 2 = 実行エラー
fn main() -> ExitCode {
    // 引数のパースをログ初期化より先に行う（`-q` / `-v` がフィルタを決めるため）。
    // パースエラーは clap が自前で stderr へ出して終了するのでログは要らない
    // （`--help` / `--version` も同じ経路で終了コード 0）
    let cli = match cli::Cli::parse_validated(std::env::args_os()) {
        Ok(cli) => cli,
        Err(err) => err.exit(),
    };
    tracing_subscriber::fmt()
        .with_env_filter(log_filter(
            cli.global.verbosity(),
            std::env::var("RUST_LOG").ok().as_deref(),
        ))
        .with_target(false)
        // ログは必ず stderr へ。tracing-subscriber の既定は stdout で、
        // `yuzu check --format json` の「標準出力へ JSON 以外を書かない」契約を破る
        // （yuzu.toml の警告が JSON の前に出てパースが失敗していた）
        .with_writer(std::io::stderr)
        // **必須**: `fmt()` の既定は true で、stderr への書き込みに失敗すると
        // 「代替として stderr へ eprintln!」する = 同じ stderr なので必ず失敗して
        // `failed printing to stderr` で panic する。`yuzu build 2>&1 | head` のように
        // 読み手が先に閉じると（EPIPE）ビルドが途中で落ち、`--force` なら dist を
        // 作り直した後なので `_search` が消えたままになる。ログは stdout（out.rs）と
        // 同じく「書けなければ捨てる」
        .log_internal_errors(false)
        .init();

    let code = match run(cli) {
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

/// ログフィルタの決定。**`-q` / `-v` は `RUST_LOG` より優先**し、無指定のときだけ
/// `RUST_LOG` を見る（不正な指定は従来どおり黙って `info` に落とす）。
/// 環境変数は引数で受け取り、テストがプロセス全体の環境を触らずに済むようにする
fn log_filter(verbosity: cli::Verbosity, rust_log: Option<&str>) -> EnvFilter {
    match verbosity {
        cli::Verbosity::Quiet => EnvFilter::new("warn"),
        cli::Verbosity::Verbose => EnvFilter::new("debug"),
        cli::Verbosity::Normal => rust_log
            .and_then(|spec| EnvFilter::try_new(spec).ok())
            .unwrap_or_else(|| EnvFilter::new("info")),
    }
}

fn run(cli: cli::Cli) -> anyhow::Result<ExitCode> {
    let ok = |()| ExitCode::SUCCESS;
    // グローバル引数の解決はここ 1 回だけ（`--root` の正規化を含む）
    let cx = cx::Cx::new(cli.global.root)?;
    match cli.command {
        cli::Command::New { dir } => commands::new::run(&cx, &dir).map(ok),
        cli::Command::Build {
            watch,
            base_url,
            force,
            drafts,
            port,
            host,
        } => commands::build::run(&cx, watch, base_url, force, drafts, port, host).map(ok),
        cli::Command::Preview { port, host } => commands::preview::run(&cx, port, host).map(ok),
        cli::Command::Dev {
            port,
            host,
            force,
            drafts,
        } => commands::dev::run(&cx, port, host, force, drafts).map(ok),
        cli::Command::Search {
            query,
            limit,
            section,
            format,
            json,
        } => {
            // `--json` は `--format json` の旧表記（両方指定は clap が矛盾として弾く）
            let format = match json {
                true => commands::search::Format::Json,
                false => format,
            };
            commands::search::run(&cx, &query, limit, &section, format).map(ok)
        }
        cli::Command::Llms { full } => commands::llms::run(&cx, full).map(ok),
        cli::Command::Fmt { check, diff } => commands::fmt::run(&cx, check, diff),
        cli::Command::Lint { fix, format } => commands::lint::run(&cx, fix, format),
        cli::Command::Check {
            format,
            external_links,
        } => commands::check::run(&cx, format, external_links),
        cli::Command::Completions { shell } => commands::completions::run(&cx, shell).map(ok),
    }
}

#[cfg(test)]
mod tests {
    use super::log_filter;
    use crate::cli::Verbosity;

    /// `-q` / `-v` は `RUST_LOG` より優先する
    #[test]
    fn フラグは_rust_log_より優先する() {
        assert_eq!(
            log_filter(Verbosity::Quiet, Some("debug")).to_string(),
            "warn"
        );
        assert_eq!(
            log_filter(Verbosity::Verbose, Some("error")).to_string(),
            "debug"
        );
    }

    /// 無指定のときは `RUST_LOG` を見て、無ければ info
    #[test]
    fn 無指定は_rust_log_を見て既定は_info() {
        assert_eq!(log_filter(Verbosity::Normal, None).to_string(), "info");
        assert_eq!(
            log_filter(Verbosity::Normal, Some("yuzu_render=debug")).to_string(),
            "yuzu_render=debug"
        );
    }

    /// 不正な `RUST_LOG` は従来どおり黙って info に落とす
    #[test]
    fn 不正な_rust_log_は_info_に落とす() {
        assert_eq!(
            log_filter(Verbosity::Normal, Some("=bogus=")).to_string(),
            "info"
        );
    }
}
