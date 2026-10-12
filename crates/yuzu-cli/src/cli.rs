//! clap によるサブコマンド定義

use std::path::PathBuf;

use clap::{Args, CommandFactory, Parser, Subcommand};
use yuzu_config::BindAddr;

#[derive(Parser)]
#[command(
    name = "yuzu",
    version,
    about = "Markdown で書いたドキュメントを静的 HTML サイトに変換するツール",
    long_about = "yuzu 🍊 — Markdown で書いた設計書を静的 HTML のドキュメントサイトに\n\
                  変換するツール。\n\
                  使い方: https://ai.implementer.net/yuzu/",
    // clap が自動で足す -h / -V / help サブコマンドは説明が英語で、変える手段が無い
    // （`mut_arg("help", …)` は組み込みの引数が build 時に足されるため panic する）。
    // 自動の分を止め、同じ動作の引数とサブコマンドを自前で定義する
    disable_help_flag = true,
    disable_version_flag = true,
    disable_help_subcommand = true
)]
pub struct Cli {
    #[command(flatten)]
    pub global: GlobalArgs,

    /// 版を表示する
    // clap が表示して終了するので値は読まない。自動の -V と同じくトップレベルだけに置く
    #[arg(short = 'V', long, action = clap::ArgAction::Version, display_order = 2000)]
    #[allow(dead_code)]
    version: Option<bool>,

    #[command(subcommand)]
    pub command: Command,
}

impl Cli {
    /// clap のパースに、グローバル引数同士の排他検証を足した入口（`main` とテストが使う）。
    ///
    /// **`conflicts_with` はサブコマンドの境界をまたぐ指定を検出しない** —
    /// `yuzu -q build -v` はトップレベルとサブコマンドで別々に検証され、どちらの側にも
    /// 矛盾が無いので通ってしまう（値は親へ伝播するので両方 true になる）。
    /// 黙ってどちらかを勝たせず、`-q -v` を同じ側に書いたときと同じ clap のエラー
    /// （終了コード 2）にする
    pub fn parse_validated<I, T>(args: I) -> Result<Self, clap::Error>
    where
        I: IntoIterator<Item = T>,
        T: Into<std::ffi::OsString> + Clone,
    {
        let cli = Self::try_parse_from(args)?;
        if let Command::Help { command } = &cli.command {
            return Err(help_for(command.as_deref()));
        }
        if cli.global.quiet && cli.global.verbose {
            return Err(Self::command().error(
                clap::error::ErrorKind::ArgumentConflict,
                "the argument '--quiet' cannot be used with '--verbose'",
            ));
        }
        Ok(cli)
    }
}

/// `yuzu help [コマンド]` の表示。`yuzu [コマンド] --help` として読み直し、表示と終了コード
/// （0）を clap のヘルプ表示に任せる。知らないコマンド名は clap の使い方エラー（2）になる
fn help_for(command: Option<&str>) -> clap::Error {
    let argv = ["yuzu"].into_iter().chain(command).chain(["--help"]);
    Cli::command()
        .try_get_matches_from(argv)
        .expect_err("--help を付けたパースはヘルプの表示で終わる")
}

/// サブコマンドをまたいで効く引数。`--root` の解決結果は [`crate::cx::Cx`] が持ち、
/// `-q` / `-v` はログのサブスクライバを組む `main` が消費する（各コマンドは見ない）。
/// **`global = true` が必須** — 無いと `yuzu build --root x`（サブコマンドの後ろ）が
/// パースエラーになる。`display_order` はサブコマンドのヘルプでグローバル引数を
/// 固有の引数の後ろへまとめるため（無いと `--limit` と `--section` の間に `--root` が挟まる）
#[derive(Args)]
#[command(next_display_order = 1000)]
pub struct GlobalArgs {
    /// プロジェクトルート（yuzu.toml のあるディレクトリ）。
    /// 指定するとカレントディレクトリからの上方向探索を行わない。
    /// new・completions では使えない
    #[arg(long, global = true, value_name = "DIR")]
    pub root: Option<PathBuf>,

    /// 進捗ログ（info 以下）を出さない。警告とエラーは出る。RUST_LOG より優先
    // `conflicts_with` は同じ側に書いたときだけ効く。境界をまたぐ指定は
    // `Cli::parse_validated` が弾く（doc コメントに書くと --help に出てしまう）
    #[arg(short, long, global = true, conflicts_with = "verbose")]
    pub quiet: bool,

    /// 詳細ログ（debug）も出す。RUST_LOG より優先
    #[arg(short, long, global = true)]
    pub verbose: bool,

    /// ヘルプを表示する
    // 自動の -h / --help の代わり（`Cli` の disable_help_flag を参照）。global なので
    // 全サブコマンドで使える。clap が表示して終了するので値は読まない
    #[arg(short = 'h', long, global = true, action = clap::ArgAction::Help)]
    #[allow(dead_code)]
    help: Option<bool>,
}

impl GlobalArgs {
    /// `-q` / `-v` の解決結果（同時指定は [`Cli::parse_validated`] が弾くので両立しない）
    pub fn verbosity(&self) -> Verbosity {
        match (self.quiet, self.verbose) {
            (true, _) => Verbosity::Quiet,
            (_, true) => Verbosity::Verbose,
            _ => Verbosity::Normal,
        }
    }
}

/// ログの量（`-q` / `-v` の解決結果）。
/// **CLI フラグは `RUST_LOG` より優先する** — 打ったフラグのほうが意図として明示的で、
/// シェルに残った環境変数に黙って負けると「`-q` を付けたのに進捗が出る」になる
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verbosity {
    /// `-q`: warn 以上だけ（`RUST_LOG=warn` 相当）
    Quiet,
    /// 無指定: `RUST_LOG` があればそれ、無ければ info
    Normal,
    /// `-v`: debug 以上（`RUST_LOG=debug` 相当）
    Verbose,
}

#[derive(Subcommand)]
pub enum Command {
    /// サンプル docs プロジェクトを生成する
    New {
        /// 生成先ディレクトリ
        dir: PathBuf,
    },

    /// content/ をビルドして dist/ に静的サイトを出力する
    Build {
        /// プロジェクトを監視して自動で再ビルドし、配信する
        /// （ブラウザは定期的に確認して再読み込みする。yuzu dev が使えない環境向け）
        #[arg(long)]
        watch: bool,
        /// 公開先の URL かパス（例: https://example.com/docs/ ・ /docs/）。yuzu.toml の
        /// base_url より優先する（CI から渡す用途）。./ のような相対パスは使えない
        #[arg(long)]
        base_url: Option<String>,
        /// インクリメンタルキャッシュ（.yuzu/cache/）を破棄してフルビルドする
        #[arg(long)]
        force: bool,
        /// draft ページ（frontmatter `draft: true`）も含めてビルドする（プレビュー用途）
        #[arg(long)]
        drafts: bool,
        /// --watch のときの配信ポート（既定は dev.port）。
        /// `yuzu dev` と並走させるときに使う
        // --watch が無いと配信しないので、黙って無視せず使い方エラーにする（10-12 レビューの R3）
        #[arg(long, requires = "watch")]
        port: Option<u16>,
        /// --watch のときの配信ホスト（IP アドレス。既定は dev.host）
        #[arg(long, requires = "watch", value_parser = parse_host)]
        host: Option<BindAddr>,
    },

    /// dist/ を配信する最小静的サーバ
    Preview {
        /// ポート番号（既定: 設定の dev.port）
        #[arg(long)]
        port: Option<u16>,
        /// バインドアドレス（IP アドレス。既定: 設定の dev.host。コンテナ内からは 0.0.0.0）
        #[arg(long, value_parser = parse_host)]
        host: Option<BindAddr>,
    },

    /// 開発サーバ（変更を監視して再ビルドし、配信中のブラウザを自動で再読み込みする）
    Dev {
        /// ポート番号（既定: 設定の dev.port）
        #[arg(long)]
        port: Option<u16>,
        /// バインドアドレス（IP アドレス。既定: 設定の dev.host。コンテナ内からは 0.0.0.0）
        #[arg(long, value_parser = parse_host)]
        host: Option<BindAddr>,
        /// インクリメンタルキャッシュ（.yuzu/cache/）を破棄してフルビルドする
        #[arg(long)]
        force: bool,
        /// draft ページ（frontmatter `draft: true`）も含めて表示する（プレビュー用途）
        #[arg(long)]
        drafts: bool,
    },

    /// ビルド済みサイトの全文検索（dist/_search をブラウザと同じエンジンで検索）
    Search {
        /// 検索クエリ（日本語可。1 文字の誤字にも寛容。
        /// "..." で囲むとフレーズ検索 = 連続出現だけにヒット）
        query: String,
        /// 表示件数
        #[arg(long, default_value_t = 10)]
        limit: usize,
        /// セクション（サイドバーの第 1 階層）で絞り込む。複数指定でいずれか
        #[arg(long, value_name = "名前")]
        section: Vec<String>,
        /// 出力形式（human = 人向けの一覧 / json = 結果の JSON 配列）
        #[arg(long, value_enum, default_value_t = crate::commands::search::Format::Human)]
        format: crate::commands::search::Format,
        /// `--format json` の旧表記（v0.17 より前との互換用。ヘルプには出さない）。
        /// `--format` と同時には指定できない
        #[arg(long, hide = true, conflicts_with = "format")]
        json: bool,
    },

    /// llms.txt をその場で生成して標準出力へ（dist/ 不要）
    Llms {
        /// llms-full.txt（全ページの正規化 Markdown 連結）を出力する
        #[arg(long)]
        full: bool,
    },

    /// content/ の Markdown を正規形へ整形する（既定: その場で書き換え）
    Fmt {
        /// 書き換えず、差分のあるファイルを列挙して非ゼロ終了する（CI 用）
        #[arg(long)]
        check: bool,
        /// 書き換えず、整形前後の unified diff を標準出力へ出す（--check を含意）。
        /// そのまま `patch -p1` / `git apply` に渡せる
        #[arg(long)]
        diff: bool,
    },

    /// 文書規約（見出し・frontmatter）と表記ゆれを診断する。違反があれば非ゼロ終了
    Lint {
        /// 表記ゆれ（全角英数字・半角カナ・長音符ゆれ・lint.terms）の
        /// 変換候補をソースへ自動適用する（修正できない違反は残り、従来どおり報告）
        #[arg(long)]
        fix: bool,
        /// 出力形式（human = 1 行テキスト / json = 単一 JSON オブジェクト /
        /// github = GitHub Actions の注釈）
        #[arg(long, value_enum, default_value_t = crate::commands::diag::Format::Human)]
        format: crate::commands::diag::Format,
    },

    /// lint ＋ リンク切れ検査 ＋ fmt 差分検出の統合チェック（CI 用）
    Check {
        /// 出力形式（human = 1 行テキスト / json = 単一 JSON オブジェクト /
        /// github = GitHub Actions の注釈）
        #[arg(long, value_enum, default_value_t = crate::commands::diag::Format::Human)]
        format: crate::commands::diag::Format,
        /// 外部リンク（http / https）の到達性も検査する（curl へ委譲。
        /// HTTP 4xx を warning `external-link-broken` として報告し、
        /// 到達不能・5xx・429 はスキップ件数に数える）
        #[arg(long)]
        external_links: bool,
        /// 整形差分（fmt）を検査しない。既存の原稿を持ち込んで、整形を後回しにするとき
        #[arg(long)]
        no_fmt: bool,
    },

    /// シェル補完スクリプトを標準出力へ出す（例: `eval "$(yuzu completions bash)"`）
    Completions {
        /// 対象シェル
        #[arg(value_enum)]
        shell: clap_complete::Shell,
    },

    /// コマンドのヘルプを表示する（`yuzu help build` は `yuzu build --help` と同じ）
    // clap の自動の help サブコマンドの代わり（`Cli` の disable_help_subcommand を参照）。
    // 実行はせず、`Cli::parse_validated` がヘルプの表示に変える
    Help {
        /// ヘルプを表示するコマンド（省略すると yuzu 全体）
        #[arg(value_name = "コマンド")]
        command: Option<String>,
    },
}

/// `--host` の解析。`yuzu.toml` の `dev.host` と同じ規則（yuzu-config の 1 実装）を通す
fn parse_host(raw: &str) -> Result<BindAddr, &'static str> {
    BindAddr::parse(raw)
}

impl Command {
    /// 監視して再ビルドを繰り返すコマンド（`dev` / `build --watch`）か。
    /// ログの時刻はこのときだけ出す（再ビルドの区切りになる）
    pub fn watches(&self) -> bool {
        matches!(self, Self::Dev { .. } | Self::Build { watch: true, .. })
    }
}

#[cfg(test)]
mod tests {
    use super::{Cli, Command, Verbosity};
    use crate::commands::search::Format;
    use clap::error::ErrorKind;
    use clap::{CommandFactory, Parser};

    /// clap 公式のスモークテスト（引数名の重複・不正な設定を検出する）。
    /// グローバル引数を足すと既存のサブコマンド固有の引数と衝突しうるので、
    /// 追加のたびにここが番人になる
    #[test]
    fn 引数定義に矛盾がない() {
        Cli::command().debug_assert();
    }

    /// `-h` / `-V` / `help` サブコマンドの説明は日本語（clap の自動の分は英語なので
    /// 自前で定義している）。サブコマンドの `-h` も同じ説明になる（global）
    #[test]
    fn ヘルプと版の説明は日本語() {
        for args in [vec!["yuzu", "-h"], vec!["yuzu", "build", "-h"]] {
            let err = Cli::parse_validated(args.clone()).err().unwrap();
            assert_eq!(err.kind(), ErrorKind::DisplayHelp, "{args:?}");
            let text = err.render().to_string();
            assert!(text.contains("ヘルプを表示する"), "{args:?}:\n{text}");
            assert!(
                !text.contains("Print"),
                "{args:?} に英語の説明が残る:\n{text}"
            );
        }
        let top = Cli::parse_validated(["yuzu", "--help"]).err().unwrap();
        assert!(top.render().to_string().contains("版を表示する"));
        let version = Cli::parse_validated(["yuzu", "-V"]).err().unwrap();
        assert_eq!(version.kind(), ErrorKind::DisplayVersion);
        assert_eq!(version.exit_code(), 0);
    }

    /// `yuzu help <コマンド>` は `yuzu <コマンド> --help` と同じ表示・終了コード 0。
    /// 知らないコマンド名は使い方エラー（終了コード 2）
    #[test]
    fn help_サブコマンドは_help_フラグと同じ表示になる() {
        for (sub, flag) in [
            (
                vec!["yuzu", "help", "build"],
                vec!["yuzu", "build", "--help"],
            ),
            (vec!["yuzu", "help"], vec!["yuzu", "--help"]),
        ] {
            let a = Cli::parse_validated(sub.clone()).err().unwrap();
            let b = Cli::parse_validated(flag).err().unwrap();
            assert_eq!(a.kind(), ErrorKind::DisplayHelp, "{sub:?}");
            assert_eq!(a.exit_code(), 0, "{sub:?}");
            assert_eq!(a.render().to_string(), b.render().to_string(), "{sub:?}");
        }
        let err = Cli::parse_validated(["yuzu", "help", "nosuch"])
            .err()
            .unwrap();
        assert_eq!(err.kind(), ErrorKind::InvalidSubcommand);
        assert_eq!(err.exit_code(), 2);
    }

    /// `--root` はサブコマンドの前後どちらでも書ける（`global = true` の効果）
    #[test]
    fn root_はサブコマンドの前後どちらでも受け付ける() {
        for args in [
            ["yuzu", "--root", "docs", "build"],
            ["yuzu", "build", "--root", "docs"],
        ] {
            let cli = Cli::try_parse_from(args).unwrap();
            assert_eq!(
                cli.global.root.as_deref(),
                Some(std::path::Path::new("docs"))
            );
            assert!(matches!(cli.command, Command::Build { .. }));
        }
    }

    /// 無指定なら None（cwd からの上方向探索へ回る）
    #[test]
    fn root_無指定なら_none() {
        let cli = Cli::try_parse_from(["yuzu", "check"]).unwrap();
        assert!(cli.global.root.is_none());
        assert_eq!(cli.global.verbosity(), Verbosity::Normal);
    }

    /// `-q` / `-v` もサブコマンドの前後どちらでも書ける
    #[test]
    fn quiet_と_verbose_はサブコマンドの前後どちらでも受け付ける() {
        for (args, expected) in [
            (["yuzu", "-q", "build"], Verbosity::Quiet),
            (["yuzu", "build", "--quiet"], Verbosity::Quiet),
            (["yuzu", "-v", "build"], Verbosity::Verbose),
            (["yuzu", "build", "--verbose"], Verbosity::Verbose),
        ] {
            let cli = Cli::parse_validated(args).unwrap();
            assert_eq!(cli.global.verbosity(), expected, "{args:?}");
        }
    }

    /// `-q` と `-v` の同時指定はエラー（黙ってどちらかを勝たせない）。
    /// 同じ側に書いた 2 通りは clap の `conflicts_with`、**サブコマンドの前後に分けた
    /// 2 通りは `parse_validated` の後検証**が弾く（clap は境界をまたぐ矛盾を見ない）
    #[test]
    fn quiet_と_verbose_の同時指定はエラー() {
        for args in [
            ["yuzu", "-q", "-v", "build"],
            ["yuzu", "build", "-q", "-v"],
            ["yuzu", "-q", "build", "-v"],
            ["yuzu", "-v", "build", "-q"],
        ] {
            let err = Cli::parse_validated(args)
                .err()
                .unwrap_or_else(|| panic!("{args:?} が通った"));
            assert_eq!(err.kind(), ErrorKind::ArgumentConflict, "{args:?}");
            // clap の使い方エラーと同じ終了コード 2
            assert_eq!(err.exit_code(), 2, "{args:?}");
        }
    }

    /// 10-12 レビューの R3: `build` の `--port` / `--host` は `--watch` が無いと配信しないので、
    /// 黙って無視せず使い方エラー（終了コード 2）にする
    #[test]
    fn build_の_port_と_host_は_watch_が必要() {
        for args in [
            vec!["yuzu", "build", "--port", "9999"],
            vec!["yuzu", "build", "--host", "0.0.0.0"],
        ] {
            let err = Cli::parse_validated(args.clone())
                .err()
                .unwrap_or_else(|| panic!("{args:?} が通った"));
            assert_eq!(err.kind(), ErrorKind::MissingRequiredArgument, "{args:?}");
            assert_eq!(err.exit_code(), 2, "{args:?}");
        }
        let cli = Cli::parse_validated(["yuzu", "build", "--watch", "--port", "9999"]).unwrap();
        assert!(matches!(
            cli.command,
            Command::Build {
                port: Some(9999),
                ..
            }
        ));
    }

    /// `--host` は IP アドレスだけ。ホスト名は `yuzu.toml` の `dev.host` と同じ文言の
    /// 使い方エラーにする（以前は起動してから「dev.host が不正です」と出ていた）
    #[test]
    fn host_は_ip_アドレスだけを受け付ける() {
        for sub in ["dev", "preview"] {
            let err = Cli::parse_validated(["yuzu", sub, "--host", "localhost"])
                .err()
                .unwrap_or_else(|| panic!("{sub} --host localhost が通った"));
            assert_eq!(err.kind(), ErrorKind::ValueValidation, "{sub}");
            assert_eq!(err.exit_code(), 2, "{sub}");
            assert!(
                err.to_string().contains("IP アドレスで指定してください"),
                "{err}"
            );
            assert!(Cli::parse_validated(["yuzu", sub, "--host", "0.0.0.0"]).is_ok());
        }
    }

    /// `search --json` は `--format json` の旧表記として残す。同時指定は矛盾エラー
    #[test]
    fn search_json_は互換で受け付け_format_との同時指定はエラー() {
        let cli = Cli::try_parse_from(["yuzu", "search", "--json", "q"]).unwrap();
        assert!(matches!(
            cli.command,
            Command::Search {
                json: true,
                format: Format::Human,
                ..
            }
        ));
        let cli = Cli::try_parse_from(["yuzu", "search", "--format", "json", "q"]).unwrap();
        assert!(matches!(
            cli.command,
            Command::Search {
                json: false,
                format: Format::Json,
                ..
            }
        ));
        assert!(
            Cli::try_parse_from(["yuzu", "search", "--json", "--format", "json", "q"]).is_err()
        );
        // 診断の github 形式は検索には無い
        assert!(Cli::try_parse_from(["yuzu", "search", "--format", "github", "q"]).is_err());
    }

    /// `completions` はシェル名を value_enum で受け、未知のシェルは使い方エラー
    #[test]
    fn completions_はシェル名を受け付ける() {
        let cli = Cli::try_parse_from(["yuzu", "completions", "zsh"]).unwrap();
        assert!(matches!(
            cli.command,
            Command::Completions {
                shell: clap_complete::Shell::Zsh
            }
        ));
        assert!(Cli::try_parse_from(["yuzu", "completions", "tcsh"]).is_err());
        assert!(Cli::try_parse_from(["yuzu", "completions"]).is_err());
    }
}
