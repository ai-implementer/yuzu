//! インデックス構築: サイトモデル → `dist/_search/` 一式

use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use rust_embed::RustEmbed;
use sha2::{Digest, Sha256};

use yuzu_core::{BuildCache, CachedSection, MarkdownOptions, OutputTracker, Page, SiteModel};
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
    /// 同義語グループ（lint.terms ＋ search.synonyms を cli が合成）。
    /// manifest に焼き込まれ、クエリ拡張に使われる
    pub synonyms: Vec<Vec<String>>,
    /// フェンスコードブロックの本文を検索対象に含めるか（`search.indexCode`）
    pub index_code: bool,
}

impl Default for IndexParams {
    fn default() -> Self {
        Self {
            dictionary: None,
            typo_enabled: true,
            max_edits: 1,
            max_terms_per_shard: 16384,
            synonyms: Vec::new(),
            index_code: false,
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

/// `_search/` 相対パスへの書き出し関数（インクリメンタル分岐を隠蔽）
type WriteFn<'a> = dyn Fn(&str, &[u8]) -> Result<(), IndexError> + 'a;

/// インクリメンタルビルドの文脈（すべて None = 従来のフル生成と同一動作）
#[derive(Default)]
pub struct IndexCtx<'a> {
    /// セクション tf のキャッシュ（全ヒット時はトークナイザ構築ごとスキップ）
    pub cache: Option<&'a BuildCache>,
    /// 出力トラッキング（Some なら _search の全削除をやめ compare-write）
    pub outputs: Option<&'a OutputTracker>,
    /// watch/dev セッションで Tokenizer を再利用する
    pub session: Option<&'a IndexSession>,
}

/// vaporetto Tokenizer のセッション共有（初回 miss 時に遅延構築）
#[derive(Default)]
pub struct IndexSession {
    tokenizer: OnceLock<Tokenizer>,
}

impl IndexSession {
    /// 構築済みならそれを返し、なければ model_bytes から構築して保持する
    fn tokenizer(&self, model_bytes: &[u8]) -> Result<&Tokenizer, IndexError> {
        if let Some(t) = self.tokenizer.get() {
            return Ok(t);
        }
        let t = Tokenizer::from_zstd_model_bytes(model_bytes)?;
        Ok(self.tokenizer.get_or_init(|| t))
    }
}

/// 同義語グループを正規化する（決定的な manifest のため）:
/// グループ内の重複・空文字列を除去してソートし、1 語以下のグループを捨て、
/// グループ列自体もソートする
fn normalized_synonyms(groups: &[Vec<String>]) -> Vec<Vec<String>> {
    let mut out: Vec<Vec<String>> = groups
        .iter()
        .map(|group| {
            let mut g: Vec<String> = group.iter().filter(|m| !m.is_empty()).cloned().collect();
            g.sort();
            g.dedup();
            g
        })
        .filter(|g| g.len() >= 2)
        .collect();
    out.sort();
    out.dedup();
    out
}

/// envKey 用: 辞書（または同梱モデル）バイトの sha256
pub fn model_fingerprint(dictionary: Option<&Path>) -> Result<String, IndexError> {
    let bytes: Vec<u8> = match dictionary {
        Some(path) => fs::read(path).map_err(IndexError::io(path))?,
        None => yuzu_index_format::builtin_model_zst().to_vec(),
    };
    Ok(hex(&Sha256::digest(&bytes)))
}

/// `output_dir/_search/` に検索インデックス一式を書き出す
pub fn build_search_index(
    site: &SiteModel,
    md_opts: &MarkdownOptions,
    params: &IndexParams,
    output_dir: &Path,
) -> Result<IndexStats, IndexError> {
    build_search_index_with(site, md_opts, params, output_dir, &IndexCtx::default())
}

/// [`build_search_index`] のインクリメンタル対応版
pub fn build_search_index_with(
    site: &SiteModel,
    md_opts: &MarkdownOptions,
    params: &IndexParams,
    output_dir: &Path,
    ctx: &IndexCtx,
) -> Result<IndexStats, IndexError> {
    let search_dir = output_dir.join(SEARCH_DIR_NAME);

    // モデル読み込み（このバイト列をそのまま dist へコピーし、
    // ネイティブ / wasm の両側で同一バイトを使う＝トークナイザ整合の保証）
    let model_bytes: Vec<u8> = match &params.dictionary {
        Some(path) => fs::read(path).map_err(IndexError::io(path))?,
        None => yuzu_index_format::builtin_model_zst().to_vec(),
    };
    // Tokenizer はキャッシュ miss が出たときだけ遅延構築する（zstd 展開が重い）。
    // セッション（watch/dev）があればそちらに保持して再利用する
    let local_session = IndexSession::default();
    let session = ctx.session.unwrap_or(&local_session);

    // セクション（h2/h3 境界）ごとの tf 集計（重み付き）。1 doc = 1 セクション
    let mut doc_lens: Vec<u32> = Vec::new();
    let mut terms: BTreeMap<String, Vec<(u32, u32)>> = BTreeMap::new();
    let mut fragments: Vec<Fragment> = Vec::new();
    let mut doc_id: u32 = 0;

    for page in &site.pages {
        let sections = match ctx.cache.and_then(|c| c.search(&page.rel, &page.source)) {
            Some(cached) => cached,
            None => {
                let computed = compute_sections(
                    page,
                    md_opts,
                    session.tokenizer(&model_bytes)?,
                    params.index_code,
                )?;
                if let Some(cache) = ctx.cache {
                    cache.store_search(&page.rel, &page.source, computed.clone());
                }
                computed
            }
        };
        for section in sections {
            doc_lens.push(section.doc_len);
            // doc_id 昇順で処理しているので postings は自然に昇順になる
            for (term, count) in section.tf {
                terms.entry(term).or_default().push((doc_id, count));
            }
            fragments.push(Fragment {
                title: page.title.clone(),
                heading: section.heading,
                url: page.route.clone(),
                anchor: section.anchor,
                text: section.text,
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

    // 書き出し。インクリメンタル時（outputs あり）は全削除をやめ、
    // compare-write ＋孤児掃除マニフェスト（cli 側）で差分管理する
    if ctx.outputs.is_none() && search_dir.exists() {
        fs::remove_dir_all(&search_dir).map_err(IndexError::io(&search_dir))?;
    }
    fs::create_dir_all(search_dir.join("index")).map_err(IndexError::io(&search_dir))?;
    let write = |rel: &str, data: &[u8]| -> Result<(), IndexError> {
        let rel_from_dist = format!("{SEARCH_DIR_NAME}/{rel}");
        match ctx.outputs {
            Some(tracker) => {
                tracker
                    .write(&rel_from_dist, data)
                    .map_err(IndexError::io(search_dir.join(rel)))?;
            }
            None => {
                let path = search_dir.join(rel);
                if let Some(parent) = path.parent() {
                    fs::create_dir_all(parent).map_err(IndexError::io(parent))?;
                }
                fs::write(&path, data).map_err(IndexError::io(&path))?;
            }
        }
        Ok(())
    };

    // シャード分割（term_id の連続範囲）と書き出し
    let chunk = params.max_terms_per_shard.max(1) as usize;
    let mut shards_meta: Vec<ShardMeta> = Vec::new();
    for (i, chunk_postings) in postings.chunks(chunk).enumerate() {
        let file = format!("index/{i:04}.bin");
        write(&file, &encode_shard(chunk_postings))?;
        let start = (i * chunk) as u32;
        shards_meta.push(ShardMeta {
            file,
            term_start: start,
            term_end: start + chunk_postings.len() as u32,
        });
    }

    // fragment（JS が直接読むので 1 doc = 1 JSON）
    for (doc_id, fragment) in fragments.iter().enumerate() {
        write(
            &format!("fragment/{doc_id}.json"),
            &serde_json::to_vec(fragment)?,
        )?;
    }

    // モデルと manifest
    write("model.zst", &model_bytes)?;

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
        synonyms: normalized_synonyms(&params.synonyms),
    };
    write("manifest.json", &serde_json::to_vec_pretty(&manifest)?)?;
    write("terms.fst", &terms_fst)?;

    copy_wasm_assets(&search_dir, &write)?;

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
fn copy_wasm_assets(_search_dir: &Path, write: &WriteFn<'_>) -> Result<(), IndexError> {
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
        write(name, data.data.as_ref())?;
    }
    Ok(())
}

/// 1 ページぶんのセクション tf を計算する（キャッシュ miss 時のみ呼ばれる）
fn compute_sections(
    page: &Page,
    md_opts: &MarkdownOptions,
    tokenizer: &Tokenizer,
    index_code: bool,
) -> Result<Vec<CachedSection>, IndexError> {
    let sections = yuzu_core::extract_plain_sections(page, md_opts, index_code)?;
    let mut out = Vec::with_capacity(sections.len());
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
        // postings の決定性（バイト同一の出力）のため tf はソートして保存する
        let mut tf: Vec<(String, u32)> = tf.into_iter().collect();
        tf.sort_unstable();
        out.push(CachedSection {
            anchor: section.anchor.clone(),
            heading: section.heading.clone(),
            text: single_line(&section.body),
            doc_len,
            tf,
        });
    }
    Ok(out)
}

/// セクション本文を 1 行に折り畳む（fragment.text 用。改行・連続空白を空白 1 つに）
fn single_line(body: &str) -> String {
    body.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
