use std::path::PathBuf;

/// サイトモデル構築・本文 HTML 化で起きるエラー
#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    #[error("{path} を読み込めません: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("{path} の frontmatter（YAML）が不正です: {message}")]
    Frontmatter { path: PathBuf, message: String },

    #[error("ignore パターン `{pattern}` が不正です: {message}")]
    InvalidIgnorePattern { pattern: String, message: String },

    #[error("HTML 出力に失敗しました: {0}")]
    Render(#[from] std::fmt::Error),
}
