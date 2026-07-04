//! clap によるサブコマンド定義

use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "yuzu",
    version,
    about = "Markdown で書いたドキュメントを静的 HTML サイトに変換する俺々ツール",
    long_about = "yuzu 🍊 — Markdown で書いた設計書をプロダクション品質の\n\
                  静的 HTML ドキュメントサイトに変換するツール。\n\
                  ロードマップと設計は README.md を参照。"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
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
    },

    /// dist/ を配信する最小静的サーバ
    Preview {
        /// ポート番号（既定: 設定の dev.port）
        #[arg(long)]
        port: Option<u16>,
    },

    /// 開発サーバ（監視ビルド＋配信＋WS ライブリロード）
    Dev {
        /// ポート番号（既定: 設定の dev.port）
        #[arg(long)]
        port: Option<u16>,
    },

    /// ビルド済みサイトの全文検索（dist/_search をブラウザと同じエンジンで検索）
    Search {
        /// 検索クエリ（日本語可。1 文字の誤字にも寛容）
        query: String,
        /// 表示件数
        #[arg(long, default_value_t = 10)]
        limit: usize,
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
}
