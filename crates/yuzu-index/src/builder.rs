//! インデックス構築: サイトモデル → `dist/_search/` 一式

use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::{Path, PathBuf};

use rust_embed::RustEmbed;
use sha2::{Digest, Sha256};

use yuzu_core::{MarkdownOptions, SiteModel};
use yuzu_index_format::{
    Bm25Params, FORMAT_VERSION, Fragment, Manifest, ShardMeta, Tokenizer, TokenizerMeta,
    TypoParams, encode_shard,
};

use crate::SEARCH_DIR_NAME;
use crate::error::IndexError;

/// vendor 済みの wasm 成果物（search.js / search_bg.wasm）。
/// 未生成でもビルドは通る（コピーをスキップして警告するだけ）
#[derive(RustEmbed)]
#[folder = "assets/search"]
struct SearchAssets;

/// タイトル token の追加重み（本文 ×1 に対して +2）。リード doc にだけ載せる
const TITLE_WEIGHT: u32 = 2;
/// 自セクション見出し token の重み。v1 は「本文に含まれる分＋TOC 加算」で実質 2 だった。
/// v2 では見出しを body から外した分ここで 2 にして同等を保つ
const HEADING_WEIGHT: u32 = 2;

/// インデックス生成の入力（cli が設定から写す。yuzu-config には依存しない）
#[derive(Debug, Clone)]
pub struct IndexParams {
    /// vaporetto モデル（`.model.zst`）のパス。None = 同梱モデル
    pub dictionary: Option<PathBuf>,
    pub typo_enabled: bool,
    /// v1 では 0..=1 に clamp される
    pub max_edits: u8,
    pub max_terms_per_shard: u32,
}

impl Default for IndexParams {
    fn default() -> Self {
        Self {
            dictionary: None,
            typo_enabled: true,
            max_edits: 1,
            max_terms_per_shard: 16384,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct IndexStats {
    pub pages: usize,
    /// doc 数（= セクション数。1 ページにつきリード 1 ＋ h2/h3 セクション）
    pub docs: usize,
    pub terms: usize,
    pub shards: usize,
}

/// `output_dir/_search/` に検索インデックス一式を書き出す
pub fn build_search_index(
    site: &SiteModel,
    md_opts: &MarkdownOptions,
    params: &IndexParams,
    output_dir: &Path,
) -> Result<IndexStats, IndexError> {
    let search_dir = output_dir.join(SEARCH_DIR_NAME);

    // モデル読み込み（このバイト列をそのまま dist へコピーし、
    // ネイティブ / wasm の両側で同一バイトを使う＝トークナイザ整合の保証）
    let model_bytes: Vec<u8> = match &params.dictionary {
        Some(path) => fs::read(path).map_err(IndexError::io(path))?,
        None => yuzu_index_format::builtin_model_zst().to_vec(),
    };
    let tokenizer = Tokenizer::from_zstd_model_bytes(&model_bytes)?;

    // セクション（h2/h3 境界）ごとの tf 集計（重み付き）。1 doc = 1 セクション
    let mut doc_lens: Vec<u32> = Vec::new();
    let mut terms: BTreeMap<String, Vec<(u32, u32)>> = BTreeMap::new();
    let mut fragments: Vec<Fragment> = Vec::new();
    let mut doc_id: u32 = 0;

    for page in &site.pages {
        let sections = yuzu_core::extract_plain_sections(page, md_opts)?;
        for (sec_idx, section) in sections.iter().enumerate() {
            let mut tf: HashMap<String, u32> = HashMap::new();
            let mut doc_len = 0u32;
            let mut add = |tokens: Vec<String>, weight: u32, tf: &mut HashMap<String, u32>| {
                for token in tokens {
                    *tf.entry(token).or_insert(0) += weight;
                    doc_len += weight;
                }
            };
            add(tokenizer.tokenize(&section.body), 1, &mut tf);
            if let Some(heading) = &section.heading {
                add(tokenizer.tokenize(heading), HEADING_WEIGHT, &mut tf);
            }
            // ページタイトルは**リード doc（先頭セクション）だけ**に載せる。
            // 全セクションに載せると同一ページの全 doc がタイトル語で同点ヒットして
            // 重複ノイズになり、df も膨らんで idf が下がる。リード doc は空本文でも
            // 必ず生成されるため「タイトル検索 → ページ先頭 1 件」の挙動が保たれる
            if sec_idx == 0 {
                add(tokenizer.tokenize(&page.title), TITLE_WEIGHT, &mut tf);
            }
            doc_lens.push(doc_len);

            // doc_id 昇順で処理しているので postings は自然に昇順になる
            for (term, count) in tf {
                terms.entry(term).or_default().push((doc_id, count));
            }

            fragments.push(Fragment {
                title: page.title.clone(),
                heading: section.heading.clone(),
                url: page.route.clone(),
                anchor: section.anchor.clone(),
                text: single_line(&section.body),
            });
            doc_id += 1;
        }
    }

    // term 辞書（fst は辞書順挿入が必須。BTreeMap の走査順で満たす）
    let mut fst_builder = fst::MapBuilder::memory();
    for (term_id, term) in terms.keys().enumerate() {
        fst_builder.insert(term, term_id as u64)?;
    }
    let terms_fst = fst_builder.into_inner()?;

    // postings の doc_id 昇順を保証（HashMap 経由でも上の理由で保たれるが、明示的に）
    let mut postings: Vec<Vec<(u32, u32)>> = terms.into_values().collect();
    for p in &mut postings {
        p.sort_unstable_by_key(|&(doc, _)| doc);
    }

    // シャード分割（term_id の連続範囲）と書き出し
    if search_dir.exists() {
        fs::remove_dir_all(&search_dir).map_err(IndexError::io(&search_dir))?;
    }
    let index_dir = search_dir.join("index");
    fs::create_dir_all(&index_dir).map_err(IndexError::io(&index_dir))?;

    let chunk = params.max_terms_per_shard.max(1) as usize;
    let mut shards_meta: Vec<ShardMeta> = Vec::new();
    for (i, chunk_postings) in postings.chunks(chunk).enumerate() {
        let file = format!("index/{i:04}.bin");
        let bytes = encode_shard(chunk_postings);
        fs::write(search_dir.join(&file), bytes).map_err(IndexError::io(search_dir.join(&file)))?;
        let start = (i * chunk) as u32;
        shards_meta.push(ShardMeta {
            file,
            term_start: start,
            term_end: start + chunk_postings.len() as u32,
        });
    }

    // fragment（JS が直接読むので 1 doc = 1 JSON）
    let fragment_dir = search_dir.join("fragment");
    fs::create_dir_all(&fragment_dir).map_err(IndexError::io(&fragment_dir))?;
    for (doc_id, fragment) in fragments.iter().enumerate() {
        let path = fragment_dir.join(format!("{doc_id}.json"));
        fs::write(&path, serde_json::to_vec(fragment)?).map_err(IndexError::io(&path))?;
    }

    // モデルと manifest
    fs::write(search_dir.join("model.zst"), &model_bytes)
        .map_err(IndexError::io(search_dir.join("model.zst")))?;

    let avg_doc_len = if doc_lens.is_empty() {
        0.0
    } else {
        doc_lens.iter().map(|&l| l as f64).sum::<f64>() as f32 / doc_lens.len() as f32
    };
    let manifest = Manifest {
        version: FORMAT_VERSION,
        tokenizer: TokenizerMeta {
            kind: "vaporetto".to_string(),
            model_file: "model.zst".to_string(),
            model_sha256: hex(&Sha256::digest(&model_bytes)),
        },
        bm25: Bm25Params::default(),
        typo: TypoParams {
            enabled: params.typo_enabled,
            max_edits: params.max_edits.min(1),
        },
        doc_count: fragments.len() as u32,
        avg_doc_len,
        doc_lens,
        term_count: postings.len() as u32,
        terms_file: "terms.fst".to_string(),
        shards: shards_meta,
    };
    fs::write(
        search_dir.join("manifest.json"),
        serde_json::to_vec_pretty(&manifest)?,
    )
    .map_err(IndexError::io(search_dir.join("manifest.json")))?;
    fs::write(search_dir.join("terms.fst"), &terms_fst)
        .map_err(IndexError::io(search_dir.join("terms.fst")))?;

    copy_wasm_assets(&search_dir)?;

    let stats = IndexStats {
        pages: site.pages.len(),
        docs: manifest.doc_count as usize,
        terms: manifest.term_count as usize,
        shards: manifest.shards.len(),
    };
    tracing::info!(
        pages = stats.pages,
        docs = stats.docs,
        terms = stats.terms,
        shards = stats.shards,
        "検索インデックス生成完了"
    );
    Ok(stats)
}

/// vendor 済み wasm 成果物を dist へコピーする。
/// 未 vendor（プレースホルダのみ）の場合は警告してスキップ（ビルドは失敗させない）
fn copy_wasm_assets(search_dir: &Path) -> Result<(), IndexError> {
    let required = ["search.js", "search_bg.wasm"];
    let missing: Vec<&str> = required
        .iter()
        .filter(|name| SearchAssets::get(name).is_none())
        .copied()
        .collect();
    if !missing.is_empty() {
        tracing::warn!(
            "検索 wasm 成果物（{}）が未生成のためコピーをスキップします。\
             ブラウザ検索を有効にするには scripts/build-search-wasm.sh を実行してください",
            missing.join(", ")
        );
        return Ok(());
    }
    for name in required {
        let data = SearchAssets::get(name).expect("存在確認済み");
        let path = search_dir.join(name);
        fs::write(&path, data.data.as_ref()).map_err(IndexError::io(&path))?;
    }
    Ok(())
}

/// セクション本文を 1 行に折り畳む（fragment.text 用。改行・連続空白を空白 1 つに）
fn single_line(body: &str) -> String {
    body.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
