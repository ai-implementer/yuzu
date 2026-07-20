use std::path::PathBuf;

/// レンダリングパイプラインのエラー
#[derive(Debug, thiserror::Error)]
pub enum RenderError {
    #[error("{path} の入出力に失敗しました: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("テンプレートエラー: {0}")]
    Template(#[from] minijinja::Error),

    #[error(transparent)]
    Core(#[from] yuzu_core::CoreError),

    #[error(
        "syntect テーマ `{name}` が見つかりません（設定 markdown.highlight を確認してください）"
    )]
    UnknownHighlightTheme { name: String },

    #[error("シンタックスハイライトの CSS 生成に失敗しました: {0}")]
    HighlightCss(#[from] syntect::Error),

    #[error(
        "frontmatter の aliases に問題が {count} 件あります（`yuzu check` で一覧できます）。最初の 1 件: {first}"
    )]
    InvalidAliases { count: usize, first: String },
}

impl RenderError {
    pub(crate) fn io(path: impl Into<PathBuf>) -> impl FnOnce(std::io::Error) -> Self {
        let path = path.into();
        move |source| Self::Io { path, source }
    }
}
