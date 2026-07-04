use std::path::PathBuf;

/// 設定の探索・読み込み・解決で起きるエラー
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error(
        "yuzu.jsonc が見つかりません（{start} から上方向に探索）。`yuzu new` で作成するか、プロジェクトルートで実行してください"
    )]
    ProjectRootNotFound { start: PathBuf },

    #[error("{path} を読み込めません: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("{path} の JSONC 構文エラー: {message}")]
    Jsonc { path: PathBuf, message: String },

    #[error("{path} のスキーマ不一致: {source}")]
    Schema {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
}
