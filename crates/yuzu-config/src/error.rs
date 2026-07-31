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

    /// `output.clean` の既定は true で、出力ディレクトリを丸ごと再帰削除する。
    /// 無検証だとルート外・ルート自身・原稿ディレクトリが消えるため load で弾く
    #[error(
        "{key} の値 `{value}` は使えません（{reason}）。プロジェクトルート（{root}）配下の相対パスを指定してください — output.clean はこのディレクトリを丸ごと削除します"
    )]
    UnsafeOutputDir {
        key: &'static str,
        value: String,
        reason: &'static str,
        root: PathBuf,
    },
}
