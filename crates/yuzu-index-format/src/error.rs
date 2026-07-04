/// インデックスフォーマットの読み書き・クエリ処理のエラー
#[derive(Debug, thiserror::Error)]
pub enum FormatError {
    #[error("manifest.json のパースに失敗しました: {0}")]
    Manifest(#[from] serde_json::Error),

    #[error(
        "インデックスフォーマットのバージョンが一致しません（期待: {expected}, 実際: {actual}）。サイトを再ビルドしてください"
    )]
    VersionMismatch { expected: u16, actual: u16 },

    #[error("term 辞書（fst）の読み込みに失敗しました: {0}")]
    Fst(#[from] fst::Error),

    #[error("トークナイザモデルの読み込みに失敗しました: {0}")]
    Model(String),

    #[error("シャードの magic が不正です（インデックスが壊れています）")]
    BadMagic,

    #[error("シャードのデータが途中で終わっています")]
    UnexpectedEof,

    #[error("varint が u32 の範囲を超えています")]
    VarintOverflow,

    #[error(
        "シャード {shard_id} は manifest と term 数が一致しません（期待: {expected}, 実際: {actual}）"
    )]
    ShardTermCountMismatch {
        shard_id: u32,
        expected: u32,
        actual: u32,
    },

    #[error("シャード {0} はロードされていません（needed_shards → load_shard の順で呼ぶこと）")]
    ShardNotLoaded(u32),

    #[error("term のローカル添字 {0} がシャードの範囲外です")]
    TermOutOfRange(u32),
}
