//! クエリエンジン（純計算・同期・I/O なし）。
//!
//! fetch / ファイル読みは呼び出し側の責務:
//! - ブラウザ: search-ui.js が fetch して [`SearchEngine::load_shard`] に渡す
//! - ネイティブ: `yuzu search` が fs::read して同じ API を叩く
//!
//! 流れ: クエリを tokenize → fst 完全一致（未ヒット token のみ編集距離 1 で展開・
//! ペナルティ付き）→ 必要シャードの postings をデコード → BM25 で加算スコアリング。

use std::collections::HashMap;

use fst::automaton::Levenshtein;
use fst::{IntoStreamer, Map, Streamer};

use crate::FORMAT_VERSION;
use crate::error::FormatError;
use crate::manifest::Manifest;
use crate::shard::Shard;
use crate::tokenizer::Tokenizer;

/// タイポ展開の上限（1 token あたり。ノイズと計算量の抑制）
const LEV_EXPANSION_LIMIT: usize = 8;
/// タイポ一致のスコアペナルティ（完全一致 = 1.0）
const LEV_WEIGHT: f32 = 0.5;
/// タイポ展開する token の最小文字数（1 文字は広範囲マッチしすぎる）
const LEV_MIN_CHARS: usize = 2;

/// 検索ヒット（doc_id は SiteModel.pages の並び順の添字）
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Hit {
    pub doc_id: u32,
    pub score: f32,
}

pub struct SearchEngine {
    tokenizer: Tokenizer,
    manifest: Manifest,
    terms: Map<Vec<u8>>,
    shards: HashMap<u32, Shard>,
}

impl SearchEngine {
    /// manifest.json / terms.fst / model.zst の 3 点から構築する
    pub fn new(
        manifest_json: &[u8],
        terms_fst: Vec<u8>,
        model_zst: &[u8],
    ) -> Result<Self, FormatError> {
        let manifest: Manifest = serde_json::from_slice(manifest_json)?;
        if manifest.version != FORMAT_VERSION {
            return Err(FormatError::VersionMismatch {
                expected: FORMAT_VERSION,
                actual: manifest.version,
            });
        }
        let terms = Map::new(terms_fst)?;
        let tokenizer = Tokenizer::from_zstd_model_bytes(model_zst)?;
        Ok(Self {
            tokenizer,
            manifest,
            terms,
            shards: HashMap::new(),
        })
    }

    pub fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    /// クエリに必要で**まだロードされていない**シャード id 列（昇順・重複なし）。
    /// 呼び出し側はこれを fetch/read して [`Self::load_shard`] に渡す
    pub fn needed_shards(&self, query: &str) -> Vec<u32> {
        let mut ids: Vec<u32> = self
            .resolve_terms(query)
            .keys()
            .filter_map(|&term_id| self.shard_for_term(term_id))
            .filter(|id| !self.shards.contains_key(id))
            .collect();
        ids.sort_unstable();
        ids.dedup();
        ids
    }

    /// fetch/read 済みのシャードバイナリを登録する
    pub fn load_shard(&mut self, shard_id: u32, bytes: &[u8]) -> Result<(), FormatError> {
        let shard = Shard::parse(bytes)?;
        let meta = self
            .manifest
            .shards
            .get(shard_id as usize)
            .ok_or(FormatError::ShardNotLoaded(shard_id))?;
        let expected = meta.term_end - meta.term_start;
        if shard.term_count() != expected {
            return Err(FormatError::ShardTermCountMismatch {
                shard_id,
                expected,
                actual: shard.term_count(),
            });
        }
        self.shards.insert(shard_id, shard);
        Ok(())
    }

    /// BM25 でスコアリングした上位 `limit` 件を返す。
    /// 未ロードのシャードに載っている term は無視される
    /// （[`Self::needed_shards`] → [`Self::load_shard`] を先に済ませる規約）
    pub fn search(&self, query: &str, limit: usize) -> Vec<Hit> {
        let resolved = self.resolve_terms(query);
        let doc_count = self.manifest.doc_count as f32;
        let avg_len = self.manifest.avg_doc_len.max(1.0);
        let (k1, b) = (self.manifest.bm25.k1, self.manifest.bm25.b);

        let mut scores: HashMap<u32, f32> = HashMap::new();
        for (&term_id, &weight) in &resolved {
            let Some(shard_id) = self.shard_for_term(term_id) else {
                continue;
            };
            let Some(shard) = self.shards.get(&shard_id) else {
                continue;
            };
            let local = term_id - self.manifest.shards[shard_id as usize].term_start;
            let Ok(postings) = shard.postings(local) else {
                continue;
            };

            // Lucene 型の非負 idf
            let df = postings.len() as f32;
            let idf = ((doc_count - df + 0.5) / (df + 0.5) + 1.0).ln();

            for (doc_id, tf) in postings {
                let len = self
                    .manifest
                    .doc_lens
                    .get(doc_id as usize)
                    .copied()
                    .unwrap_or(0) as f32;
                let tf = tf as f32;
                let tf_component = tf * (k1 + 1.0) / (tf + k1 * (1.0 - b + b * len / avg_len));
                *scores.entry(doc_id).or_insert(0.0) += weight * idf * tf_component;
            }
        }

        let mut hits: Vec<Hit> = scores
            .into_iter()
            .map(|(doc_id, score)| Hit { doc_id, score })
            .collect();
        hits.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(a.doc_id.cmp(&b.doc_id))
        });
        hits.truncate(limit);
        hits
    }

    /// クエリを token → term_id（重み付き）へ解決する。
    /// 完全一致を優先し、未ヒット token だけタイポ展開する
    fn resolve_terms(&self, query: &str) -> HashMap<u32, f32> {
        let mut resolved: HashMap<u32, f32> = HashMap::new();
        let merge = |resolved: &mut HashMap<u32, f32>, id: u64, weight: f32| {
            let entry = resolved.entry(id as u32).or_insert(0.0);
            *entry = entry.max(weight);
        };

        for token in self.tokenizer.tokenize(query) {
            if let Some(id) = self.terms.get(&token) {
                merge(&mut resolved, id, 1.0);
                continue;
            }

            let max_edits = self.manifest.typo.max_edits.min(1);
            if !self.manifest.typo.enabled
                || max_edits == 0
                || token.chars().count() < LEV_MIN_CHARS
            {
                continue;
            }
            // DFA 構築の失敗（過長クエリ等）はタイポ補正なしで続行
            let Ok(automaton) = Levenshtein::new(&token, u32::from(max_edits)) else {
                continue;
            };
            let mut stream = self.terms.search(&automaton).into_stream();
            let mut expanded = 0;
            while let Some((_, id)) = stream.next() {
                merge(&mut resolved, id, LEV_WEIGHT);
                expanded += 1;
                if expanded >= LEV_EXPANSION_LIMIT {
                    break;
                }
            }
        }
        resolved
    }

    /// term_id → シャード id（manifest.shards は term_start 昇順・連続範囲）
    fn shard_for_term(&self, term_id: u32) -> Option<u32> {
        let shards = &self.manifest.shards;
        let idx = shards.partition_point(|s| s.term_end <= term_id);
        (idx < shards.len() && shards[idx].term_start <= term_id).then_some(idx as u32)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use crate::manifest::{Bm25Params, Manifest, ShardMeta, TokenizerMeta, TypoParams};
    use crate::shard::encode_shard;
    use crate::{FORMAT_VERSION, SearchEngine, Tokenizer};

    const MODEL: &[u8] = include_bytes!("../assets/model/bccwj-suw_c1.0.model.zst");

    /// テスト用の極小インデクサ（本物は yuzu-index 側。ここではエンジンの検証用）
    fn build_index(docs: &[&str], max_terms_per_shard: u32) -> SearchEngine {
        let tokenizer = Tokenizer::from_zstd_model_bytes(MODEL).unwrap();

        let mut terms: BTreeMap<String, Vec<(u32, u32)>> = BTreeMap::new();
        let mut doc_lens = Vec::new();
        for (doc_id, text) in docs.iter().enumerate() {
            let tokens = tokenizer.tokenize(text);
            doc_lens.push(tokens.len() as u32);
            let mut tf: BTreeMap<String, u32> = BTreeMap::new();
            for t in tokens {
                *tf.entry(t).or_insert(0) += 1;
            }
            for (term, count) in tf {
                terms.entry(term).or_default().push((doc_id as u32, count));
            }
        }

        let mut fst_builder = fst::MapBuilder::memory();
        for (term_id, term) in terms.keys().enumerate() {
            fst_builder.insert(term, term_id as u64).unwrap();
        }
        let terms_fst = fst_builder.into_inner().unwrap();

        let postings: Vec<Vec<(u32, u32)>> = terms.values().cloned().collect();
        let mut shards_meta = Vec::new();
        let mut shard_bytes = Vec::new();
        let mut start = 0u32;
        while (start as usize) < postings.len().max(1) {
            let end = (start + max_terms_per_shard).min(postings.len() as u32);
            shard_bytes.push(encode_shard(&postings[start as usize..end as usize]));
            shards_meta.push(ShardMeta {
                file: format!("index/{:04}.bin", shards_meta.len()),
                term_start: start,
                term_end: end,
            });
            start = end;
            if postings.is_empty() {
                break;
            }
        }

        let avg = doc_lens.iter().sum::<u32>() as f32 / docs.len().max(1) as f32;
        let manifest = Manifest {
            version: FORMAT_VERSION,
            tokenizer: TokenizerMeta {
                kind: "vaporetto".into(),
                model_file: "model.zst".into(),
                model_sha256: String::new(),
            },
            bm25: Bm25Params::default(),
            typo: TypoParams {
                enabled: true,
                max_edits: 1,
            },
            doc_count: docs.len() as u32,
            avg_doc_len: avg,
            doc_lens,
            term_count: postings.len() as u32,
            terms_file: "terms.fst".into(),
            shards: shards_meta,
        };

        let mut engine =
            SearchEngine::new(&serde_json::to_vec(&manifest).unwrap(), terms_fst, MODEL).unwrap();
        for (i, bytes) in shard_bytes.iter().enumerate() {
            engine.load_shard(i as u32, bytes).unwrap();
        }
        engine
    }

    #[test]
    fn 出現頻度が高い文書が上位に来る() {
        let engine = build_index(
            &[
                "柚子と柚子と柚子の話をします",
                "柚子とりんごとばななの話をします",
                "検索エンジンだけの話をします",
            ],
            10_000,
        );
        let hits = engine.search("柚子", 10);
        assert!(hits.len() >= 2, "hits={hits:?}");
        assert_eq!(hits[0].doc_id, 0, "tf が高い doc 0 が先頭: {hits:?}");
        assert!(hits.iter().all(|h| h.doc_id != 2), "無関係文書は出ない");
    }

    #[test]
    fn 複数語クエリは両方含む文書が上位() {
        let engine = build_index(
            &[
                "静的サイトを生成する。検索もできる。",
                "静的サイトを生成する。",
                "検索の仕組みだけを説明する。",
            ],
            10_000,
        );
        let hits = engine.search("静的サイト 検索", 10);
        assert_eq!(hits[0].doc_id, 0, "両方の語を含む doc 0 が先頭: {hits:?}");
    }

    #[test]
    fn 一編集距離の誤字でもヒットする() {
        let engine = build_index(
            &["yuzu の build コマンドの説明", "無関係な文書です"],
            10_000,
        );

        // 前提: 誤字 token が 1 token に分かち書きされること（モデル依存の前提を明示）
        let tokenizer = Tokenizer::from_zstd_model_bytes(MODEL).unwrap();
        let typo_tokens = tokenizer.tokenize("biild");
        assert_eq!(typo_tokens.len(), 1, "前提が崩れた: {typo_tokens:?}");

        // "biild" は "build" の 1 置換
        let hits = engine.search("biild", 10);
        assert_eq!(hits.len(), 1, "hits={hits:?}");
        assert_eq!(hits[0].doc_id, 0);

        // 完全一致よりスコアが低い（ペナルティ 0.5）
        let exact = engine.search("build", 10);
        assert!(exact[0].score > hits[0].score);
    }

    #[test]
    fn シャード分割をまたいでも検索できる() {
        let docs = [
            "あか あお きいろ みどり むらさき",
            "いぬ ねこ うさぎ とり さかな",
            "はる なつ あき ふゆ",
        ];
        // term 数を強制的に複数シャードへ分割
        let engine = build_index(&docs, 3);
        assert!(engine.manifest().shards.len() > 1, "複数シャードの前提");
        let hits = engine.search("ねこ", 10);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].doc_id, 1);
    }

    #[test]
    fn needed_shards_はロード済みを除外する() {
        let engine = build_index(&["あか あお きいろ", "いぬ ねこ"], 2);
        // build_index が全シャードをロード済みなので needed は空
        assert!(engine.needed_shards("ねこ").is_empty());
    }

    #[test]
    fn 不正なバージョンの_manifest_はエラー() {
        let json = format!(
            r#"{{"version":{},"tokenizer":{{"kind":"vaporetto","modelFile":"m","modelSha256":""}},
                "bm25":{{"k1":1.2,"b":0.75}},"typo":{{"enabled":true,"maxEdits":1}},
                "docCount":0,"avgDocLen":0.0,"docLens":[],"termCount":0,
                "termsFile":"terms.fst","shards":[]}}"#,
            FORMAT_VERSION + 1
        );
        let empty_fst = fst::MapBuilder::memory().into_inner().unwrap();
        assert!(SearchEngine::new(json.as_bytes(), empty_fst, MODEL).is_err());
    }
}
