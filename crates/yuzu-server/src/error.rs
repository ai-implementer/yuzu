/// 配信・監視のエラー
#[derive(Debug, thiserror::Error)]
pub enum ServerError {
    #[error("サーバの起動に失敗しました: {0}")]
    Io(#[from] std::io::Error),

    #[error("ファイル監視に失敗しました: {0}")]
    Notify(#[from] notify::Error),
}
