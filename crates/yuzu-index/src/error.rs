use std::path::PathBuf;

/// インデックス生成・ネイティブ検索のエラー
#[derive(Debug, thiserror::Error)]
pub enum IndexError {
    #[error("{path} の入出力に失敗しました: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error(transparent)]
    Core(#[from] yuzu_core::CoreError),

    #[error(transparent)]
    Format(#[from] mikan::FormatError),

    #[error("term 辞書（fst）の構築に失敗しました: {0}")]
    Fst(#[from] fst::Error),

    #[error("fragment の直列化に失敗しました: {0}")]
    Json(#[from] serde_json::Error),

    #[error(
        "検索インデックス（{0}）がありません。`yuzu build` を実行してください（search.enabled が true であること）"
    )]
    MissingIndex(PathBuf),
}

impl IndexError {
    pub(crate) fn io(path: impl Into<PathBuf>) -> impl FnOnce(std::io::Error) -> Self {
        let path = path.into();
        move |source| Self::Io { path, source }
    }
}
