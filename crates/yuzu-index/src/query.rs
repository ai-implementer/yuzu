//! ネイティブ検索（`yuzu search` 用）。
//! ブラウザの wasm と同じ [`SearchEngine`] を、fetch の代わりに fs::read で駆動する

use std::fs;
use std::path::Path;

use yuzu_index_format::{Fragment, Manifest, SearchEngine};

use crate::SEARCH_DIR_NAME;
use crate::error::IndexError;

/// 抜粋の最大文字数（ブラウザ UI と同じ値）
const EXCERPT_CHARS: usize = 160;

/// ネイティブ検索の 1 件（fragment を解決済み）
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResult {
    pub doc_id: u32,
    pub score: f32,
    /// ページタイトル
    pub title: String,
    /// セクション見出し（リード doc は None）
    pub heading: Option<String>,
    /// サイト相対 URL（route）
    pub url: String,
    /// 見出しアンカー（`url + "#" + anchor` で遷移）
    pub anchor: Option<String>,
    /// クエリ一致箇所周辺の動的抜粋
    pub excerpt: String,
}

/// `dist/_search/` を読み込んで検索する
pub fn search_dist_with_total(
    dist: &Path,
    query: &str,
    limit: usize,
) -> Result<(Vec<SearchResult>, usize), IndexError> {
    let search_dir = dist.join(SEARCH_DIR_NAME);
    let manifest_path = search_dir.join("manifest.json");
    if !manifest_path.is_file() {
        return Err(IndexError::MissingIndex(search_dir));
    }

    let manifest_bytes = fs::read(&manifest_path).map_err(IndexError::io(&manifest_path))?;
    // ファイル名の解決に一度パースする（エンジンも内部で検証込みでパースする）
    let manifest: Manifest = serde_json::from_slice(&manifest_bytes)?;

    let terms_path = search_dir.join(&manifest.terms_file);
    let terms_fst = fs::read(&terms_path).map_err(IndexError::io(&terms_path))?;
    let model_path = search_dir.join(&manifest.tokenizer.model_file);
    let model = fs::read(&model_path).map_err(IndexError::io(&model_path))?;

    let mut engine = SearchEngine::new(&manifest_bytes, terms_fst, &model)?;

    // ブラウザの fetch と同じ 2 段取得をファイル読みで再現
    for shard_id in engine.needed_shards(query) {
        let file = &manifest.shards[shard_id as usize].file;
        let path = search_dir.join(file);
        let bytes = fs::read(&path).map_err(IndexError::io(&path))?;
        engine.load_shard(shard_id, &bytes)?;
    }

    let (hits, total) = engine.search_with_total(query, limit);
    let mut results = Vec::with_capacity(hits.len());
    for hit in hits {
        let path = search_dir.join(format!("fragment/{}.json", hit.doc_id));
        let bytes = fs::read(&path).map_err(IndexError::io(&path))?;
        let fragment: Fragment = serde_json::from_slice(&bytes)?;
        // 動的抜粋は wasm と完全に同じ SearchEngine::excerpt を通す（整合の実証を兼ねる）
        let excerpt: String = engine
            .excerpt(&fragment.text, query, EXCERPT_CHARS)
            .into_iter()
            .map(|s| s.text)
            .collect();
        results.push(SearchResult {
            doc_id: hit.doc_id,
            score: hit.score,
            title: fragment.title,
            heading: fragment.heading,
            url: fragment.url,
            anchor: fragment.anchor,
            excerpt,
        });
    }
    Ok((results, total))
}

/// [`search_dist_with_total`] の従来形（総ヒット数なし）
pub fn search_dist(
    dist: &Path,
    query: &str,
    limit: usize,
) -> Result<Vec<SearchResult>, IndexError> {
    Ok(search_dist_with_total(dist, query, limit)?.0)
}
