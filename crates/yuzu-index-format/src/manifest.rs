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
    /// 同義語グループ（lint.terms ＋ search.synonyms 由来。クエリ拡張に使う）。
    /// v0.3 以前の manifest には無いフィールドなので default で互換を保つ
    #[serde(default)]
    pub synonyms: Vec<Vec<String>>,
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

/// `fragment/<docId>.json` の中身（結果描画用。ブラウザは JS が直接読む）。
/// v2: 1 doc = 1 セクション（h2/h3 境界）。抜粋はクエリ時に text から動的生成する
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Fragment {
    /// ページタイトル
    pub title: String,
    /// セクション見出し（リード doc は None）。表示は「title › heading」
    pub heading: Option<String>,
    /// サイト相対 URL（route）。base の付与は表示側の責務
    pub url: String,
    /// 見出しアンカー ID（リード doc は None）。遷移先は `url + "#" + anchor`
    pub anchor: Option<String>,
    /// 動的抜粋用のセクション全文（空白折り畳み済みの生テキスト）
    pub text: String,
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
