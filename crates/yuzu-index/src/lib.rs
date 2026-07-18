//! yuzu の検索インデクサ。
//!
//! サイトモデルから `dist/_search/` 一式（manifest / terms.fst / シャード /
//! fragment / モデル / wasm 成果物）を生成する。
//! クエリエンジンとフォーマットは `yuzu-index-format` にあり、
//! ブラウザ（wasm）とネイティブ（[`search_dist`]）で同一コードを共有する。
//!
//! この crate は設定（yuzu-config）に依存しない。cli が設定を [`IndexParams`] に
//! 写して渡す（依存方向の凍結: cli → index → {core, index-format}）。

mod builder;
mod error;
mod query;

pub use builder::{
    FIELD_POS_GAP, IndexCtx, IndexParams, IndexSession, IndexStats, build_search_index,
    build_search_index_with, model_fingerprint,
};
pub use error::IndexError;
pub use query::{SearchResult, search_dist, search_dist_with_total};

/// `dist/` 内の検索成果物ディレクトリ名
pub const SEARCH_DIR_NAME: &str = "_search";
