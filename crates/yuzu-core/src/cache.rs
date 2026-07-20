//! インクリメンタルビルドのページ単位キャッシュ（`.yuzu/cache/`）。
//!
//! 方針: **高価なページ派生物（メタ・本文 HTML・検索 tf・llms 正規化 md）だけ**を
//! キャッシュし、安価なテンプレート合成・集約（nav/pager/fst/llms 連結）は毎回
//! 全実行する。クロスページ依存はテンプレート段に閉じているため、この分離で
//! 依存解析なしに正しさを保てる。
//!
//! キーは 2 段構成:
//! - **envKey**: 設定・yuzu バージョン・トークナイザモデル等の全入力。
//!   不一致なら全キャッシュを破棄してフルビルドへ縮退する
//! - **routesKey**: 非 draft ページの rel→route 集合。`.md` リンク解決の入力なので、
//!   ページの追加・削除・改名時は本文 HTML キャッシュだけを安全側で全破棄する
//!   （メタ・検索 tf・llms はページ単体の派生物なので温存できる）
//!
//! 不整合・破損は常に「キャッシュなし = フルビルド」へ縮退する。
//! `.yuzu/cache/` の削除はいつでも安全。

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::model::{Frontmatter, TocEntry};

/// キャッシュフォーマットのバージョン。ビルドロジックが変わって
/// キャッシュ内容の意味が変わるときに上げる（安全弁）
/// - v2: openapi/jsonschema ブロックの SSR 追加（本文 HTML の生成ロジック変更）
/// - v3: flowchart スタイル構文の SSR ＋ OpenAPI ファイル間 $ref（同上）
/// - v4: content 同伴アセットの相対参照を絶対 URL へ書き換え（同上）
/// - v5: state / ER / class 図のスタイル構文 SSR（従来フォールバックが SSR 成功へ）
/// - v6: OpenAPI の schemas 一覧＋Swagger 2.0 対応（components を持つ既存ページの
///   本文 HTML も変わる）
/// - v7: mindmap / timeline の SSR 追加（従来フォールバックが SSR 成功へ）
/// - v8: 検索 tf に出現位置を追加（インデックスフォーマット v3・フレーズ検索の土台）
/// - v9: コードブロックの行 span 化＋表示メタ（title / 行ハイライト / 行番号）
/// - v10: frontmatter に aliases を追加（CachedMeta の Frontmatter に載る）
pub const CACHE_FORMAT_VERSION: u32 = 10;

/// パス1（extract_meta）の結果
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CachedMeta {
    pub frontmatter: Frontmatter,
    /// frontmatter → h1 → ファイル名で解決済みのタイトル
    pub title: String,
    pub toc: Vec<TocEntry>,
}

/// パス2（render_body_html）の結果
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CachedBody {
    /// syntect クラス・SSR SVG 込みの本文 HTML
    pub html: String,
    /// mermaid SSR がフォールバックしたか（mermaid.js 読込要否の判定に使う）
    pub mermaid_fallback: bool,
}

/// パス4（検索）の 1 セクション。tf はタイトル・見出し重み適用済み
/// （全ヒット時はトークナイザの構築自体をスキップできる）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CachedSection {
    pub anchor: Option<String>,
    pub heading: Option<String>,
    /// fragment 用に空白折り畳み済みのセクション全文
    pub text: String,
    /// 重み付き文書長
    pub doc_len: u32,
    /// token → (重み付き出現数, 出現位置列)。位置はフィールド連結ストリーム
    /// （body → heading → title、境界にギャップ）上のトークン添字・昇順・絶対値
    /// （delta 化はシャードエンコード時に行う）
    pub tf: Vec<(String, u32, Vec<u32>)>,
}

/// 1 ページぶんのキャッシュエントリ（`pages/<sha256(rel)>.json`）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PageCacheEntry {
    /// content 相対パス（デバッグ用に平文で保持）
    pub rel: String,
    /// ページキー: sha256(source)
    pub source_sha256: String,
    pub meta: Option<CachedMeta>,
    pub body: Option<CachedBody>,
    pub search: Option<Vec<CachedSection>>,
    /// llms-full 用の正規化 Markdown
    pub llms: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GlobalMeta {
    format_version: u32,
    env_key: String,
    routes_key: String,
}

/// キャッシュヒット数の統計（ビルドログ用）
#[derive(Debug, Default, Clone, Copy)]
pub struct CacheStats {
    pub meta_hits: usize,
    pub body_hits: usize,
    pub body_misses: usize,
    pub search_hits: usize,
    pub llms_hits: usize,
}

struct PageState {
    entry: PageCacheEntry,
    dirty: bool,
}

struct Inner {
    pages: HashMap<String, PageState>,
    /// このビルドで参照・更新された rel（save 時にこれ以外を purge）
    touched: HashSet<String>,
    routes_key: String,
    stats: CacheStats,
}

/// ビルド全体で共有するページキャッシュ。`&self` API（内部 Mutex）
pub struct BuildCache {
    dir: PathBuf,
    env_key: String,
    inner: Mutex<Inner>,
}

impl BuildCache {
    /// `.yuzu/cache/` を読み込む。formatVersion / envKey 不一致・破損は
    /// 空キャッシュ（= フルビルド）として開始する
    pub fn load(dir: &Path, env_key: &str) -> Self {
        let mut pages = HashMap::new();
        let mut routes_key = String::new();

        let global: Option<GlobalMeta> = fs::read(dir.join("global.json"))
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok());
        let valid = global
            .as_ref()
            .is_some_and(|g| g.format_version == CACHE_FORMAT_VERSION && g.env_key == env_key);

        if valid {
            routes_key = global.expect("valid 判定済み").routes_key;
            if let Ok(entries) = fs::read_dir(dir.join("pages")) {
                for entry in entries.flatten() {
                    let Ok(bytes) = fs::read(entry.path()) else {
                        continue;
                    };
                    let Ok(page): Result<PageCacheEntry, _> = serde_json::from_slice(&bytes) else {
                        continue; // 破損エントリは無視（そのページだけフルビルド）
                    };
                    pages.insert(
                        page.rel.clone(),
                        PageState {
                            entry: page,
                            dirty: false,
                        },
                    );
                }
            }
        }

        Self {
            dir: dir.to_path_buf(),
            env_key: env_key.to_string(),
            inner: Mutex::new(Inner {
                pages,
                touched: HashSet::new(),
                routes_key,
                stats: CacheStats::default(),
            }),
        }
    }

    /// ビルド開始時に呼ぶ（watch セッションでの再利用のため stats/touched をリセット）
    pub fn begin_build(&self) {
        let mut inner = self.inner.lock().unwrap();
        inner.touched.clear();
        inner.stats = CacheStats::default();
    }

    /// サイトモデル確定後に呼ぶ。routes（rel→route 集合）が前回と違えば
    /// 本文 HTML キャッシュだけを全破棄する（メタ・検索・llms は温存）
    pub fn set_routes_key(&self, key: String) {
        let mut inner = self.inner.lock().unwrap();
        if inner.routes_key != key {
            for state in inner.pages.values_mut() {
                if state.entry.body.take().is_some() {
                    state.dirty = true;
                }
            }
            inner.routes_key = key;
        }
    }

    /// sha256 hex（ページキー計算用の共有ヘルパ）
    pub fn source_hash(source: &str) -> String {
        hex(&Sha256::digest(source.as_bytes()))
    }

    /// 複数パートを連結した sha256 hex（envKey / routesKey 計算用）
    pub fn sha256_hex_parts(parts: &[&[u8]]) -> String {
        let mut hasher = Sha256::new();
        for part in parts {
            hasher.update(part);
            hasher.update([0u8]); // パート境界（連結の曖昧さ防止）
        }
        hex(&hasher.finalize())
    }

    pub fn meta(&self, rel: &str, source_hash: &str) -> Option<CachedMeta> {
        let mut inner = self.inner.lock().unwrap();
        inner.touched.insert(rel.to_string());
        let hit = inner
            .pages
            .get(rel)
            .filter(|s| s.entry.source_sha256 == source_hash)
            .and_then(|s| s.entry.meta.clone());
        if hit.is_some() {
            inner.stats.meta_hits += 1;
        }
        hit
    }

    pub fn store_meta(&self, rel: &str, source_hash: &str, meta: CachedMeta) {
        self.store(rel, source_hash, |entry| entry.meta = Some(meta));
    }

    pub fn body(&self, rel: &Path, source: &str) -> Option<CachedBody> {
        let rel = rel_str(rel);
        let hash = Self::source_hash(source);
        let mut inner = self.inner.lock().unwrap();
        inner.touched.insert(rel.clone());
        let hit = inner
            .pages
            .get(&rel)
            .filter(|s| s.entry.source_sha256 == hash)
            .and_then(|s| s.entry.body.clone());
        if hit.is_some() {
            inner.stats.body_hits += 1;
        } else {
            inner.stats.body_misses += 1;
        }
        hit
    }

    pub fn store_body(&self, rel: &Path, source: &str, body: CachedBody) {
        self.store(&rel_str(rel), &Self::source_hash(source), |entry| {
            entry.body = Some(body)
        });
    }

    pub fn search(&self, rel: &Path, source: &str) -> Option<Vec<CachedSection>> {
        let rel = rel_str(rel);
        let hash = Self::source_hash(source);
        let mut inner = self.inner.lock().unwrap();
        inner.touched.insert(rel.clone());
        let hit = inner
            .pages
            .get(&rel)
            .filter(|s| s.entry.source_sha256 == hash)
            .and_then(|s| s.entry.search.clone());
        if hit.is_some() {
            inner.stats.search_hits += 1;
        }
        hit
    }

    pub fn store_search(&self, rel: &Path, source: &str, sections: Vec<CachedSection>) {
        self.store(&rel_str(rel), &Self::source_hash(source), |entry| {
            entry.search = Some(sections)
        });
    }

    pub fn llms(&self, rel: &Path, source: &str) -> Option<String> {
        let rel = rel_str(rel);
        let hash = Self::source_hash(source);
        let mut inner = self.inner.lock().unwrap();
        inner.touched.insert(rel.clone());
        let hit = inner
            .pages
            .get(&rel)
            .filter(|s| s.entry.source_sha256 == hash)
            .and_then(|s| s.entry.llms.clone());
        if hit.is_some() {
            inner.stats.llms_hits += 1;
        }
        hit
    }

    pub fn store_llms(&self, rel: &Path, source: &str, normalized: String) {
        self.store(&rel_str(rel), &Self::source_hash(source), |entry| {
            entry.llms = Some(normalized)
        });
    }

    /// source_hash が変わっていたら他の派生物を巻き添えにせず**エントリごと作り直す**
    /// （古い source の派生物を混在させない）
    fn store(&self, rel: &str, source_hash: &str, update: impl FnOnce(&mut PageCacheEntry)) {
        let mut inner = self.inner.lock().unwrap();
        inner.touched.insert(rel.to_string());
        let state = inner
            .pages
            .entry(rel.to_string())
            .or_insert_with(|| PageState {
                entry: PageCacheEntry {
                    rel: rel.to_string(),
                    source_sha256: source_hash.to_string(),
                    meta: None,
                    body: None,
                    search: None,
                    llms: None,
                },
                dirty: true,
            });
        if state.entry.source_sha256 != source_hash {
            state.entry = PageCacheEntry {
                rel: rel.to_string(),
                source_sha256: source_hash.to_string(),
                meta: None,
                body: None,
                search: None,
                llms: None,
            };
        }
        update(&mut state.entry);
        state.dirty = true;
    }

    pub fn stats(&self) -> CacheStats {
        self.inner.lock().unwrap().stats
    }

    /// ビルド**成功時のみ**呼ぶ。dirty エントリを書き出し、
    /// このビルドで touched に現れなかったエントリ（削除ページ）を purge する
    pub fn save(&self) -> std::io::Result<()> {
        let mut inner = self.inner.lock().unwrap();
        let pages_dir = self.dir.join("pages");
        fs::create_dir_all(&pages_dir)?;

        // 削除ページのエントリを落とす
        let stale: Vec<String> = inner
            .pages
            .keys()
            .filter(|rel| !inner.touched.contains(*rel))
            .cloned()
            .collect();
        for rel in stale {
            inner.pages.remove(&rel);
        }

        // dirty のみ書き出し
        let mut expected: HashSet<String> = HashSet::new();
        for state in inner.pages.values_mut() {
            let file = format!("{}.json", Self::source_hash(&state.entry.rel));
            expected.insert(file.clone());
            if state.dirty {
                fs::write(pages_dir.join(&file), serde_json::to_vec(&state.entry)?)?;
                state.dirty = false;
            }
        }

        // pages/ 内の未知・stale ファイルを掃除
        if let Ok(entries) = fs::read_dir(&pages_dir) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if !expected.contains(&name) {
                    let _ = fs::remove_file(entry.path());
                }
            }
        }

        let global = GlobalMeta {
            format_version: CACHE_FORMAT_VERSION,
            env_key: self.env_key.clone(),
            routes_key: inner.routes_key.clone(),
        };
        fs::write(
            self.dir.join("global.json"),
            serde_json::to_vec_pretty(&global)?,
        )?;
        Ok(())
    }
}

/// rel（PathBuf）→ `/` 区切り文字列（キャッシュキー。OS 差を吸収）
fn rel_str(rel: &Path) -> String {
    rel.iter()
        .map(|c| c.to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn body(html: &str) -> CachedBody {
        CachedBody {
            html: html.to_string(),
            mermaid_fallback: false,
        }
    }

    #[test]
    fn 保存と再読込のラウンドトリップ() {
        let dir = tempfile::tempdir().unwrap();
        let cache = BuildCache::load(dir.path(), "env1");
        cache.set_routes_key("r1".to_string());
        cache.store_body(Path::new("guide/a.md"), "src", body("<p>a</p>"));
        cache.save().unwrap();

        let cache2 = BuildCache::load(dir.path(), "env1");
        cache2.set_routes_key("r1".to_string());
        let hit = cache2.body(Path::new("guide/a.md"), "src");
        assert_eq!(hit.unwrap().html, "<p>a</p>");
    }

    #[test]
    fn env_key_不一致は空キャッシュとして開始() {
        let dir = tempfile::tempdir().unwrap();
        let cache = BuildCache::load(dir.path(), "env1");
        cache.store_body(Path::new("a.md"), "src", body("x"));
        cache.save().unwrap();

        let cache2 = BuildCache::load(dir.path(), "env2");
        assert!(cache2.body(Path::new("a.md"), "src").is_none());
    }

    #[test]
    fn routes_key_不一致は_body_のみ破棄して他は温存() {
        let dir = tempfile::tempdir().unwrap();
        let cache = BuildCache::load(dir.path(), "env1");
        cache.set_routes_key("r1".to_string());
        cache.store_body(Path::new("a.md"), "src", body("x"));
        cache.store_llms(Path::new("a.md"), "src", "md".to_string());
        cache.save().unwrap();

        let cache2 = BuildCache::load(dir.path(), "env1");
        cache2.set_routes_key("r2".to_string()); // ページ追加/削除相当
        assert!(cache2.body(Path::new("a.md"), "src").is_none());
        assert_eq!(cache2.llms(Path::new("a.md"), "src").as_deref(), Some("md"));
    }

    #[test]
    fn source_変更は全派生物を巻き添えにする() {
        let dir = tempfile::tempdir().unwrap();
        let cache = BuildCache::load(dir.path(), "env1");
        cache.store_body(Path::new("a.md"), "v1", body("x"));
        cache.store_llms(Path::new("a.md"), "v1", "md".to_string());
        // source が変わって body だけ新しく入った → llms は旧 source の値を返さない
        cache.store_body(Path::new("a.md"), "v2", body("y"));
        assert!(cache.llms(Path::new("a.md"), "v2").is_none());
    }

    #[test]
    fn touched_外のエントリは_save_で_purge_される() {
        let dir = tempfile::tempdir().unwrap();
        let cache = BuildCache::load(dir.path(), "env1");
        cache.store_body(Path::new("a.md"), "src", body("x"));
        cache.store_body(Path::new("deleted.md"), "src", body("y"));
        cache.save().unwrap();

        let cache2 = BuildCache::load(dir.path(), "env1");
        cache2.begin_build();
        // a.md だけ touch（deleted.md は走査に現れなかった想定）
        let _ = cache2.body(Path::new("a.md"), "src");
        cache2.save().unwrap();

        let cache3 = BuildCache::load(dir.path(), "env1");
        assert!(cache3.body(Path::new("a.md"), "src").is_some());
        assert!(cache3.body(Path::new("deleted.md"), "src").is_none());
    }

    #[test]
    fn 破損した_global_json_は空キャッシュとして開始() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("global.json"), b"{ broken").unwrap();
        let cache = BuildCache::load(dir.path(), "env1");
        assert!(cache.body(Path::new("a.md"), "src").is_none());
    }

    #[test]
    fn stats_はヒットとミスを数える() {
        let dir = tempfile::tempdir().unwrap();
        let cache = BuildCache::load(dir.path(), "env1");
        let _ = cache.body(Path::new("a.md"), "src"); // miss
        cache.store_body(Path::new("a.md"), "src", body("x"));
        let _ = cache.body(Path::new("a.md"), "src"); // hit
        let stats = cache.stats();
        assert_eq!(stats.body_misses, 1);
        assert_eq!(stats.body_hits, 1);
    }
}
