//! インデックス構築: サイトモデル → `dist/_search/` 一式

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use rayon::prelude::*;
use rust_embed::RustEmbed;
use sha2::{Digest, Sha256};

use mikan::{Bm25Params, DocumentInput, SectionInput, Tokenizer, TokenizerMeta, TypoParams, build};
use yuzu_core::{BuildCache, CachedSection, MarkdownOptions, OutputTracker, Page, SiteModel};

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
/// フィールド境界（body → heading → title）に挟む出現位置のギャップ。
/// フレーズ照合（隣接 = 位置差 1）がフィールドをまたいで偽ヒットするのを防ぐ。
/// 隣接判定だけなら 2 で足りるが、将来の近接スコアリング（within-k）の余地として
/// Lucene の positionIncrementGap の慣行に合わせて大きめに取る
pub const FIELD_POS_GAP: u32 = 100;

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
    /// コンテンツインクルード（`file=`）解決の基準ディレクトリ（プロジェクトルート）。
    /// None なら展開しない（引用ブロックは索引されない）
    pub project_root: Option<PathBuf>,
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
            project_root: None,
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

/// envKey 用: 辞書（または同梱モデル）バイトの sha256
pub fn model_fingerprint(dictionary: Option<&Path>) -> Result<String, IndexError> {
    let bytes: Vec<u8> = match dictionary {
        Some(path) => fs::read(path).map_err(IndexError::io(path))?,
        None => mikan::builtin_model_zst().to_vec(),
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

    // 出力先までの経路にシンボリックリンクが無いことを毎回確認する
    // （`_search` へ書くのはこの関数だけなので、単独で呼ばれても素通りしない）。
    //
    // 通常は project_root から見る（yuzu-config が output_dir をルート配下に
    // 強制するので、ルート → output_dir → `_search` を一度に歩ける）。
    // ライブラリ直接利用で無関係なパスを渡された場合は、書き込む `_search` の
    // 側だけを見る（出力先がまだ無いなら、これから実ディレクトリを作るので検査不要）
    match params.project_root.as_deref() {
        Some(root) if search_dir.starts_with(root) => {
            yuzu_core::output::ensure_no_symlink_under(root, &search_dir)
                .map_err(IndexError::io(&search_dir))?;
        }
        _ if output_dir.exists() => {
            yuzu_core::output::ensure_no_symlink_under(output_dir, &search_dir)
                .map_err(IndexError::io(&search_dir))?;
        }
        _ => {}
    }

    // モデル読み込み（このバイト列をそのまま dist へコピーし、
    // ネイティブ / wasm の両側で同一バイトを使う＝トークナイザ整合の保証）
    let model_bytes: Vec<u8> = match &params.dictionary {
        Some(path) => fs::read(path).map_err(IndexError::io(path))?,
        None => mikan::builtin_model_zst().to_vec(),
    };
    // Tokenizer はキャッシュ miss が出たときだけ遅延構築する（zstd 展開が重い）。
    // セッション（watch/dev）があればそちらに保持して再利用する
    let local_session = IndexSession::default();
    let session = ctx.session.unwrap_or(&local_session);

    // 索引対象のページ集合。合成の検索結果ページ自身は索引しない
    // （JS 前提の空ページで、ヒットしても読むものが無い）。
    // 以降の 3 つの反復すべてをこの集合で回す（zip の整合を保つ）
    let pages: Vec<&yuzu_core::Page> = site.pages.iter().filter(|p| p.in_search_index()).collect();

    // ページごとのセクション計算。tokenize がフルビルドの支配的コストなので
    // rayon で並列化する（Phase 33）。キャッシュ判定を先行パスで済ませ、
    // miss があるときだけトークナイザを構築する（全ヒットなら zstd 展開ごと
    // スキップ = 従来どおり。並列ループ前に 1 回だけ作り &Tokenizer を共有）
    // (依存ハッシュ, キャッシュヒット) をページ順に用意する
    let prepared: Vec<(Option<String>, Option<Vec<CachedSection>>)> = pages
        .iter()
        .map(|page| {
            let deps = include_deps_hash(
                page,
                md_opts,
                params.index_code,
                params.project_root.as_deref(),
            );
            let hit = ctx
                .cache
                .and_then(|c| c.search(&page.rel, &page.source, deps.as_deref()));
            (deps, hit)
        })
        .collect();
    let tokenizer = match prepared.iter().any(|(_, hit)| hit.is_none()) {
        true => Some(session.tokenizer(&model_bytes)?),
        false => None,
    };
    let sections_per_page: Vec<Vec<CachedSection>> = pages
        .par_iter()
        .zip(prepared)
        .map(|(page, (deps, hit))| match hit {
            Some(sections) => Ok(sections),
            None => {
                let tokenizer = tokenizer.expect("miss があればトークナイザ構築済み");
                let computed = compute_sections(
                    page,
                    md_opts,
                    tokenizer,
                    params.index_code,
                    params.project_root.as_deref(),
                )?;
                if let Some(cache) = ctx.cache {
                    cache.store_search(&page.rel, &page.source, deps.as_deref(), computed.clone());
                }
                Ok(computed)
            }
        })
        .collect::<Result<_, IndexError>>()?;

    // ページ → ドキュメント入力への薄いマッピング。doc_id 採番・postings 集約・
    // fst/シャード構築・manifest 生成は mikan::build（yuzu 非依存の
    // 汎用ロジック）に委譲する
    // 絞り込みグループ = ナビ第 1 階層（サイドバーの区分そのもの）。
    // 順序もナビに従うので、検索のチップとサイドバーの並びが一致する
    let nav_groups = yuzu_core::nav_groups(&site.nav);
    let group_of = |route: &str| -> Option<String> {
        let key = yuzu_core::route_group_key(route);
        nav_groups
            .iter()
            .find(|g| g.key == key)
            .map(|g| g.label.clone())
    };
    let docs: Vec<DocumentInput> = pages
        .iter()
        .zip(&sections_per_page)
        .map(|(page, sections)| DocumentInput {
            title: page.title.clone(),
            url: page.route.clone(),
            group: group_of(&page.route),
            sections: sections
                .iter()
                .map(|s| SectionInput {
                    anchor: s.anchor.clone(),
                    heading: s.heading.clone(),
                    text: s.text.clone(),
                    doc_len: s.doc_len,
                    tf: s.tf.clone(),
                })
                .collect(),
        })
        .collect();

    let build_opts = mikan::BuildOptions {
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
        max_terms_per_shard: params.max_terms_per_shard,
        synonyms: params.synonyms.clone(),
        groups: nav_groups.iter().map(|g| g.label.clone()).collect(),
    };
    let built = build(&docs, &build_opts)?;

    // 書き出し。インクリメンタル時（outputs あり）は全削除をやめ、
    // compare-write ＋孤児掃除マニフェスト（cli 側）で差分管理する
    if ctx.outputs.is_none() {
        // output_dir は yuzu-config が検証済み。`_search` がその直下であることを
        // 確認してから削除する（yuzu-index はプロジェクトルートを知らない）
        yuzu_core::output::remove_dir_all_under(output_dir, &search_dir)
            .map_err(IndexError::io(&search_dir))?;
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
            // tracker 経路と同じ検証を通す（rel の解釈を 1 実装に揃える）
            None => {
                yuzu_core::output::write_under(output_dir, &rel_from_dist, data)
                    .map_err(IndexError::io(search_dir.join(rel)))?;
            }
        }
        Ok(())
    };

    // シャード書き出し
    for (file, bytes) in &built.shards {
        write(file, bytes)?;
    }

    // fragment（JS が直接読むので 1 doc = 1 JSON）
    for (doc_id, fragment) in built.fragments.iter().enumerate() {
        write(
            &format!("fragment/{doc_id}.json"),
            &serde_json::to_vec(fragment)?,
        )?;
    }

    // モデル
    write("model.zst", &model_bytes)?;

    // content_hash: ブラウザ側 OPFS キャッシュの版管理に使う識別子。
    // terms.fst ＋ 全シャード（連番順）＋ モデルバイトを連結してハッシュする
    // （manifest.json 以外の OPFS キャッシュ対象バイナリすべてを網羅するのが要点）。
    // mikan::build はこのフィールドを空文字で返すので、ここで計算して埋める
    let mut hasher = Sha256::new();
    hasher.update(&built.terms_fst);
    for (_, bytes) in &built.shards {
        hasher.update(bytes);
    }
    hasher.update(&model_bytes);
    let mut manifest = built.manifest;
    manifest.content_hash = hex(&hasher.finalize());

    write("manifest.json", &serde_json::to_vec_pretty(&manifest)?)?;
    write("terms.fst", &built.terms_fst)?;

    copy_wasm_assets(&search_dir, &write)?;

    let stats = IndexStats {
        pages: pages.len(),
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

/// vendor 済み wasm 成果物（＋対になる手書き JS クライアント）を dist へコピーする。
/// 未 vendor（プレースホルダのみ）の場合は警告してスキップ（ビルドは失敗させない）
fn copy_wasm_assets(_search_dir: &Path, write: &WriteFn<'_>) -> Result<(), IndexError> {
    let required = [
        "search.js",
        "search_bg.wasm",
        "search-client.js",
        "opfs-cache.js",
    ];
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
/// ページが引用する外部ファイル（`file=`）の内容ハッシュ。検索 tf の
/// キャッシュキーに source ハッシュとは**別フィールドで**添える
/// （畳み込むと meta / body / llms まで巻き添えでリセットされる）。
///
/// - コード引用は `index_code` 有効時のみ索引されるのでそのときだけハッシュへ。
///   Markdown 断片（```include）は散文として**常に**索引されるので常にハッシュへ
/// - 事前フィルタ（`contains("file=")`）で、引用のない大多数のページでは
///   comrak の再パースすら起こさない
/// - 特別レンダリング言語の引用は索引に入らないので除外する（render の
///   external_deps を流用すると openapi の `file:` まで巻き込んで過剰無効化になる）
/// - ハッシュ対象は**解決後のテキスト**（行範囲の切り出し後）なので、
///   参照先の引用範囲外の変更では無効化しない
fn include_deps_hash(
    page: &Page,
    md_opts: &MarkdownOptions,
    index_code: bool,
    project_root: Option<&Path>,
) -> Option<String> {
    let root = project_root?;
    // プレフィルタ: 断片（```include）もコード引用も file= が必須なので有効なまま
    if !page.source.contains("file=") {
        return None;
    }
    let parts: Vec<Vec<u8>> = yuzu_core::collect_include_specs(&page.source, md_opts)
        .iter()
        .filter(|inc| match inc.lang.as_deref() {
            // Markdown 断片は indexCode と無関係に常に索引される → 常にハッシュへ
            Some(yuzu_core::FRAGMENT_LANG) => true,
            // 特別レンダリング言語（openapi の file: 等）は索引に入らない（既存どおり）
            Some(lang) if yuzu_core::is_special_render_lang(lang, md_opts) => false,
            // コード引用は indexCode 有効時のみ索引される
            _ => index_code,
        })
        .map(|inc| match yuzu_core::resolve_include(root, &inc.spec) {
            Ok(text) => text.into_bytes(),
            // 読めない参照は固定マーカー（索引内容は失敗種別によらず「何も入らない」）
            Err(_) => b"<unresolved>".to_vec(),
        })
        .collect();
    if parts.is_empty() {
        return None;
    }
    let refs: Vec<&[u8]> = parts.iter().map(Vec::as_slice).collect();
    Some(BuildCache::sha256_hex_parts(&refs))
}

fn compute_sections(
    page: &Page,
    md_opts: &MarkdownOptions,
    tokenizer: &Tokenizer,
    index_code: bool,
    project_root: Option<&Path>,
) -> Result<Vec<CachedSection>, IndexError> {
    let sections = yuzu_core::extract_plain_sections(page, md_opts, index_code, project_root)?;
    let mut out = Vec::with_capacity(sections.len());
    for (sec_idx, section) in sections.iter().enumerate() {
        // token → (重み付き tf, 出現位置列)。位置はフィールド連結ストリーム上の
        // トークン添字で、重みは位置に影響しない（見出し語は tf +2 だが位置は 1 個）
        let mut tf: HashMap<String, (u32, Vec<u32>)> = HashMap::new();
        let mut doc_len = 0u32;
        let mut next_pos = 0u32;
        let mut add =
            |tokens: Vec<String>, weight: u32, tf: &mut HashMap<String, (u32, Vec<u32>)>| {
                for token in tokens {
                    let entry = tf.entry(token).or_insert((0, Vec::new()));
                    entry.0 += weight;
                    entry.1.push(next_pos);
                    doc_len += weight;
                    next_pos += 1;
                }
                // フィールド境界のギャップ（フレーズ照合の偽隣接防止）
                next_pos += FIELD_POS_GAP;
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
        let mut tf: Vec<(String, u32, Vec<u32>)> = tf
            .into_iter()
            .map(|(token, (count, positions))| (token, count, positions))
            .collect();
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
