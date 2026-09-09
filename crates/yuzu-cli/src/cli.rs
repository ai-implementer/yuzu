//! clap によるサブコマンド定義

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

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

/// サブコマンドをまたいで効く引数。解決結果は [`crate::cx::Cx`] が持つ。
/// **`global = true` が必須** — 無いと `yuzu build --root x`（サブコマンドの後ろ）が
/// パースエラーになる
#[derive(Args)]
pub struct GlobalArgs {
    /// プロジェクトルート（yuzu.toml のあるディレクトリ）。
    /// 指定するとカレントディレクトリからの上方向探索を行わない
    #[arg(long, global = true, value_name = "DIR")]
    pub root: Option<PathBuf>,
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
        /// JSON で出力する
        #[arg(long)]
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
}

#[cfg(test)]
mod tests {
    use super::{Cli, Command};
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
    }
}
