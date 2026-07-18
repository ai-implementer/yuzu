//! yuzu の検索インデックスフォーマットとクエリエンジン。
//!
//! `yuzu-index`（ネイティブのインデクサ・`yuzu search`）と
//! `yuzu-search-wasm`（ブラウザのクエリエンジン）が**この crate の同一コードを共有**する。
//!
//! ⚠️ 最重要の整合制約: index 時（ネイティブ）と query 時（wasm）で
//! **同一トークナイザ・同一モデルバイト**を使うこと。ズレると検索がヒットしない。
//! そのため [`Tokenizer`] はここに 1 実装だけ置き、モデル（`.model.zst`）は
//! インデクサが `dist/_search/model.zst` へそのままコピーして両側で読む。
//!
//! 構成:
//! - [`varint`] — postings の LEB128 + delta エンコード
//! - [`shard`] — シャードバイナリ（`index/NNNN.bin`）の読み書き
//! - [`manifest`] — `manifest.json` のスキーマ
//! - [`tokenizer`] — vaporetto による分かち書き＋正規化
//! - [`engine`] — fst 照合・タイポトレランス・BM25（純計算・I/O なし。
//!   fetch/ファイル読みは呼び出し側 = JS / CLI の責務）
//!
//! wasm バイナリを軽く保つため、この crate は依存を必要最小限に保つこと。

mod builder;
mod engine;
mod error;
mod excerpt;
mod manifest;
mod shard;
mod tokenizer;
pub mod varint;

pub use builder::{BuildOptions, BuiltIndex, DocumentInput, SectionInput, build};
pub use engine::{Hit, SearchEngine};
pub use error::FormatError;
pub use excerpt::{ExcerptSegment, make_excerpt};
pub use manifest::{Bm25Params, Fragment, Manifest, ShardMeta, TokenizerMeta, TypoParams};
pub use shard::{Posting, Shard, encode_shard};
pub use tokenizer::Tokenizer;

/// インデックスフォーマットのバージョン。互換性を壊す変更で上げる。
/// v2: doc をページ単位 → セクション（h2/h3）単位に変更。Fragment v2（heading/anchor/text）
/// v3: postings に出現位置（pos_count 明示＋delta varint 列）を追加（フレーズ検索の土台）
pub const FORMAT_VERSION: u16 = 3;

/// 同梱モデル（`yuzu-index` が有効化する）
#[cfg(feature = "builtin-model")]
pub fn builtin_model_zst() -> &'static [u8] {
    include_bytes!("../assets/model/bccwj-suw_c1.0.model.zst")
}
