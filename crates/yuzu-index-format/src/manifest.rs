//! `dist/_search/manifest.json` のスキーマ（初期ロードで最初に読むメタ情報）

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    /// インデックスフォーマットのバージョン（[`crate::FORMAT_VERSION`]）
    pub version: u16,
    pub tokenizer: TokenizerMeta,
    pub bm25: Bm25Params,
    pub typo: TypoParams,
    pub doc_count: u32,
    pub avg_doc_len: f32,
    /// doc_id（= 添字）→ 重み付き文書長。小規模 docs 前提で manifest に直置き
    pub doc_lens: Vec<u32>,
    pub term_count: u32,
    /// term 辞書ファイル名（`_search/` からの相対。fst::Map のバイト列）
    pub terms_file: String,
    /// term_id の連続範囲で分割されたシャード（添字 = shard_id）
    pub shards: Vec<ShardMeta>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenizerMeta {
    /// v1 では "vaporetto" 固定
    pub kind: String,
    /// モデルファイル名（`_search/` からの相対）
    pub model_file: String,
    /// モデルバイトの sha256（native/wasm の整合検知用）
    pub model_sha256: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Bm25Params {
    pub k1: f32,
    pub b: f32,
}

impl Default for Bm25Params {
    fn default() -> Self {
        Self { k1: 1.2, b: 0.75 }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TypoParams {
    pub enabled: bool,
    pub max_edits: u8,
}

/// `fragment/<docId>.json` の中身（結果描画用。ブラウザは JS が直接読む）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Fragment {
    pub title: String,
    /// サイト相対 URL（route）。base の付与は表示側の責務
    pub url: String,
    pub excerpt: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShardMeta {
    /// シャードファイル名（`_search/` からの相対。例: `index/0000.bin`）
    pub file: String,
    /// このシャードが受け持つ term_id 範囲 [term_start, term_end)
    pub term_start: u32,
    pub term_end: u32,
}
