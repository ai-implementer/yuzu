//! ネイティブ検索（`yuzu search` 用）。
//! ブラウザの wasm と同じ [`SearchEngine`] を、fetch の代わりに fs::read で駆動する

use std::fs;
use std::path::Path;

use mikan::{Fragment, Manifest, SearchEngine, SearchOptions};

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
    /// 絞り込みグループ（ナビ第 1 階層）の表示名。未分類は None
    pub section: Option<String>,
}

/// 絞り込み込みの検索結果（件数表示とファセットに必要な情報つき）
#[derive(Debug, Clone)]
pub struct SearchOutput {
    pub results: Vec<SearchResult>,
    /// 絞り込み後の総ヒット数
    pub total: usize,
    /// 絞り込み前の総ヒット数
    pub total_unfiltered: usize,
    /// (グループ表示名, 絞り込み前のヒット数) をナビ順で
    pub group_counts: Vec<(String, u32)>,
}

/// `dist/_search/` を読み込んで検索する（総ヒット数つき）
pub fn search_dist_with_total(
    dist: &Path,
    query: &str,
    limit: usize,
) -> Result<(Vec<SearchResult>, usize), IndexError> {
    let out = search_dist_with_options(dist, query, limit, &[])?;
    Ok((out.results, out.total))
}

/// グループ絞り込みに対応した検索。`sections` はグループの**表示名**（OR）で、
/// 空なら絞り込まない。未知の名前は [`IndexError::UnknownSection`]
pub fn search_dist_with_options(
    dist: &Path,
    query: &str,
    limit: usize,
    sections: &[String],
) -> Result<SearchOutput, IndexError> {
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

    // 表示名 → グループ id。未知の名前は候補を添えて弾く（黙って 0 件にしない）
    let mut group_ids = Vec::with_capacity(sections.len());
    for name in sections {
        match engine.group_id(name) {
            Some(id) => group_ids.push(id),
            None => {
                return Err(IndexError::UnknownSection {
                    name: name.clone(),
                    available: engine.groups().to_vec(),
                });
            }
        }
    }

    let outcome =
        engine.search_with_options(query, &SearchOptions::new(limit).with_groups(&group_ids));
    let group_counts: Vec<(String, u32)> = engine
        .groups()
        .iter()
        .cloned()
        .zip(outcome.group_counts.iter().copied())
        .collect();
    let mut results = Vec::with_capacity(outcome.hits.len());
    for hit in outcome.hits {
        let path = search_dir.join(format!("fragment/{}.json", hit.doc_id));
        let bytes = fs::read(&path).map_err(IndexError::io(&path))?;
        let fragment: Fragment = serde_json::from_slice(&bytes)?;
        // 動的抜粋は wasm と完全に同じ SearchEngine::excerpt を通す（整合の実証を兼ねる）
        let excerpt: String = engine
            .excerpt(&fragment.text, query, EXCERPT_CHARS)
            .into_iter()
            .map(|s| s.text)
            .collect();
        // グループは manifest の写像（doc_id → id → 表示名）から引く。
        // url から導き直すと 2 実装になるので必ずこちらを通す
        let section = engine
            .manifest()
            .doc_groups
            .get(hit.doc_id as usize)
            .and_then(|&gid| engine.groups().get(gid as usize))
            .cloned();
        results.push(SearchResult {
            doc_id: hit.doc_id,
            score: hit.score,
            title: fragment.title,
            heading: fragment.heading,
            url: fragment.url,
            anchor: fragment.anchor,
            excerpt,
            section,
        });
    }
    Ok(SearchOutput {
        results,
        total: outcome.total,
        total_unfiltered: outcome.total_unfiltered,
        group_counts,
    })
}

/// [`search_dist_with_total`] の従来形（総ヒット数なし）
pub fn search_dist(
    dist: &Path,
    query: &str,
    limit: usize,
) -> Result<Vec<SearchResult>, IndexError> {
    Ok(search_dist_with_total(dist, query, limit)?.0)
}
