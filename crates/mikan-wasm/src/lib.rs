//! yuzu のクライアント検索（wasm32-unknown-unknown）。
//!
//! **ロジックは持たない**薄い wasm-bindgen ラッパ。エンジン本体・トークナイザ・
//! フォーマットはすべて `mikan` にあり、ネイティブの `yuzu search` と
//! 同一コードを共有する（トークナイザ整合の保証）。
//!
//! fetch は JS 側（同梱の `js/search-client.js`。テーマの search-ui.js から
//! 利用される）の責務（Pagefind 方式）:
//! 1. manifest.json / terms.fst / model.zst を fetch（OPFS キャッシュ経由）して
//!    [`YuzuSearch`] を構築
//! 2. `needed_shards(query)` → 未取得シャードを fetch → `load_shard`
//! 3. `search(query, limit)` → 上位ヒットの fragment/<docId>.json を fetch して描画
//!
//! ビルドは `scripts/build-search-wasm.sh`（wasm-bindgen-cli + wasm-opt を直接叩き、
//! `js/` の手書き JS も同じ vendor 先へコピーする。rustwasm org サンセットのため
//! wasm-pack には寄せない）。

use wasm_bindgen::prelude::*;

use mikan::SearchEngine;

#[wasm_bindgen]
pub struct YuzuSearch {
    inner: SearchEngine,
}

#[wasm_bindgen]
impl YuzuSearch {
    /// manifest.json / terms.fst / model.zst の 3 点から構築する
    #[wasm_bindgen(constructor)]
    pub fn new(
        manifest_json: &[u8],
        terms_fst: &[u8],
        model_zst: &[u8],
    ) -> Result<YuzuSearch, JsError> {
        let inner = SearchEngine::new(manifest_json, terms_fst.to_vec(), model_zst)
            .map_err(|e| JsError::new(&e.to_string()))?;
        Ok(Self { inner })
    }

    /// クエリに必要でまだロードされていないシャード id 列
    #[wasm_bindgen(js_name = neededShards)]
    pub fn needed_shards(&self, query: &str) -> Vec<u32> {
        self.inner.needed_shards(query)
    }

    /// fetch 済みシャードを登録する
    #[wasm_bindgen(js_name = loadShard)]
    pub fn load_shard(&mut self, shard_id: u32, bytes: &[u8]) -> Result<(), JsError> {
        self.inner
            .load_shard(shard_id, bytes)
            .map_err(|e| JsError::new(&e.to_string()))
    }

    /// BM25 の上位 `limit` 件と総ヒット数を JSON 文字列で返す:
    /// `{"total":12,"hits":[{"docId":0,"score":1.2},…]}`
    ///
    /// ⚠️ **このシグネチャは変えない**。`search_bg.wasm` は `_search/` の固定 URL で
    /// 配信されるため HTTP キャッシュに旧版が残りうる。ここへ絞り込み引数を足すと、
    /// 旧 wasm が余分な引数を黙って無視して「絞り込んでいないのに絞り込んだ件数」を
    /// 表示する（静かに嘘をつく）。絞り込みは [`Self::search_in`] を新設して足した
    pub fn search(&self, query: &str, limit: usize) -> String {
        self.search_in(query, limit, "")
    }

    /// [`Self::search`] のグループ絞り込み版。`groups_json` は表示名の JSON 配列
    /// （`["ガイド"]`）で、空文字・空配列なら絞り込まない。
    /// 返す JSON に `totalUnfiltered` と `groupCounts` を足す（`{total,hits}` を読む
    /// 旧 UI からは無害な追加）
    #[wasm_bindgen(js_name = searchIn)]
    pub fn search_in(&self, query: &str, limit: usize, groups_json: &str) -> String {
        let names: Vec<String> = match groups_json.trim().is_empty() {
            true => Vec::new(),
            false => serde_json::from_str(groups_json).unwrap_or_default(),
        };
        let ids: Vec<u16> = names
            .iter()
            .filter_map(|name| self.inner.group_id(name))
            .collect();
        let outcome = self
            .inner
            .search_with_options(query, &mikan::SearchOptions::new(limit).with_groups(&ids));
        let hits: Vec<String> = outcome
            .hits
            .iter()
            .map(|h| format!(r#"{{"docId":{},"score":{}}}"#, h.doc_id, h.score))
            .collect();
        let counts: Vec<String> = outcome
            .group_counts
            .iter()
            .map(|n| n.to_string())
            .collect();
        format!(
            r#"{{"total":{},"totalUnfiltered":{},"groupCounts":[{}],"hits":[{}]}}"#,
            outcome.total,
            outcome.total_unfiltered,
            counts.join(","),
            hits.join(",")
        )
    }

    /// グループ（絞り込み単位）の表示名を JSON 配列で返す: `["ガイド","リファレンス"]`。
    /// 古いインデックスでは空配列 = UI が絞り込みを出さない
    pub fn groups(&self) -> String {
        serde_json::to_string(self.inner.groups()).unwrap_or_else(|_| "[]".into())
    }

    /// クエリをエンジンと同一の分かち書きで token 配列（JSON）にする: `["検索","エンジン"]`
    pub fn tokenize(&self, query: &str) -> String {
        serde_json::to_string(&self.inner.tokenize(query)).unwrap_or_else(|_| "[]".into())
    }

    /// text からクエリ一致箇所周辺の抜粋断片（JSON）を返す:
    /// `[{"text":"…文脈 ","mark":false},{"text":"検索","mark":true},…]`。
    /// mark = true の断片を <mark> で描画する（一致判定・正規化はエンジンと同一）
    pub fn excerpt(&self, text: &str, query: &str, max_chars: usize) -> String {
        serde_json::to_string(&self.inner.excerpt(text, query, max_chars))
            .unwrap_or_else(|_| "[]".into())
    }
}
