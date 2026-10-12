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

    /// cli は設定の読み込み直後に位置付きの設定エラーにするので、ここへ来るのは
    /// ライブラリを直接使ったときだけ（既定へ黙って縮退しない）
    #[error("theme.toc.levels の値 `{value}` は使えません。{reason}")]
    InvalidTocLevels { value: String, reason: &'static str },

    #[error("シンタックスハイライトの CSS 生成に失敗しました: {0}")]
    HighlightCss(#[from] syntect::Error),

    #[error(
        "frontmatter の aliases に問題が {count} 件あります（`yuzu check` で一覧できます）。最初の 1 件: {first}"
    )]
    InvalidAliases { count: usize, first: String },

    /// route の衝突（`route-conflict`）とファイル名の不正（`unsafe-page-path`）。
    /// どちらも「書き出すと壊れた URL が生成物に残る」ので中断する
    #[error(
        "ページ URL に問題が {count} 件あります（`yuzu check` で一覧できます）。最初の 1 件: {first}"
    )]
    InvalidRoutes { count: usize, first: String },
}

impl RenderError {
    pub(crate) fn io(path: impl Into<PathBuf>) -> impl FnOnce(std::io::Error) -> Self {
        let path = path.into();
        move |source| Self::Io { path, source }
    }
}
