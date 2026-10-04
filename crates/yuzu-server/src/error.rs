/// 配信・監視のエラー
#[derive(Debug, thiserror::Error)]
pub enum ServerError {
    #[error("サーバの起動に失敗しました: {0}")]
    Io(#[from] std::io::Error),

    /// ポートが使用中。`{0}` をそのまま出すと「Address already in use (os error 48)」
    /// だけになり、**どのポートか・どう回避するか**が分からない
    #[error(
        "ポート {port} は既に使用中です（{host} にバインドできません）。`--port` で別のポートを指定するか、そのポートを使っているプロセスを止めてください"
    )]
    PortInUse { host: std::net::IpAddr, port: u16 },

    #[error("ファイル監視に失敗しました: {0}")]
    Notify(#[from] notify::Error),

    /// 監視中の再ビルドが panic して監視スレッドが止まったので、配信も止めた
    #[error("監視中の再ビルドで内部エラーが起きたため、配信を終了しました: {0}")]
    WatchStopped(String),
}
