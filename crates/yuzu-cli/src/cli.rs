//! clap によるサブコマンド定義

use std::path::PathBuf;

use clap::{Args, CommandFactory, Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "yuzu",
    version,
    about = "Markdown で書いたドキュメントを静的 HTML サイトに変換するツール",
    long_about = "yuzu 🍊 — Markdown で書いた設計書をプロダクション品質の\n\
                  静的 HTML ドキュメントサイトに変換するツール。\n\
                  ロードマップと設計は README.md を参照。"
)]
pub struct Cli {
    #[command(flatten)]
    pub global: GlobalArgs,

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
        if cli.global.quiet && cli.global.verbose {
            return Err(Self::command().error(
                clap::error::ErrorKind::ArgumentConflict,
                "the argument '--quiet' cannot be used with '--verbose'",
            ));
        }
        Ok(cli)
    }
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
    /// 指定するとカレントディレクトリからの上方向探索を行わない
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
        /// content/・theme/ を監視して自動再ビルドし、配信＋オートリフレッシュする
        #[arg(long)]
        watch: bool,
        /// baseUrl を上書きする（site/build の設定より優先。
        /// CI から configure-pages の base_path を渡す用途）
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
        #[arg(long)]
        port: Option<u16>,
        /// --watch のときの配信ホスト（既定は dev.host）
        #[arg(long)]
        host: Option<String>,
    },

    /// dist/ を配信する最小静的サーバ
    Preview {
        /// ポート番号（既定: 設定の dev.port）
        #[arg(long)]
        port: Option<u16>,
        /// バインドアドレス（既定: 設定の dev.host。コンテナ内からは 0.0.0.0）
        #[arg(long)]
        host: Option<String>,
    },

    /// 開発サーバ（監視ビルド＋配信＋WS ライブリロード）
    Dev {
        /// ポート番号（既定: 設定の dev.port）
        #[arg(long)]
        port: Option<u16>,
        /// バインドアドレス（既定: 設定の dev.host。コンテナ内からは 0.0.0.0）
        #[arg(long)]
        host: Option<String>,
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
        /// そのまま `patch -p1` / `git apply` に食わせられる
        #[arg(long)]
        diff: bool,
    },

    /// 文書規約の診断（見出し・frontmatter）。違反があれば非ゼロ終了
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
    },

    /// シェル補完スクリプトを標準出力へ出す（例: `eval "$(yuzu completions bash)"`）
    Completions {
        /// 対象シェル
        #[arg(value_enum)]
        shell: clap_complete::Shell,
    },
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
