//! yuzu のクライアント検索（wasm32-unknown-unknown）。
//!
//! **ロジックは持たない**薄い wasm-bindgen ラッパ。エンジン本体・トークナイザ・
//! フォーマットはすべて `yuzu-index-format` にあり、ネイティブの `yuzu search` と
//! 同一コードを共有する（トークナイザ整合の保証）。
//!
//! fetch は JS 側（テーマの search-ui.js）の責務（Pagefind 方式）:
//! 1. manifest.json / terms.fst / model.zst を fetch して [`YuzuSearch`] を構築
//! 2. `needed_shards(query)` → 未取得シャードを fetch → `load_shard`
//! 3. `search(query, limit)` → 上位ヒットの fragment/<docId>.json を fetch して描画
//!
//! ビルドは `scripts/build-search-wasm.sh`（wasm-bindgen-cli + wasm-opt を直接叩く。
//! rustwasm org サンセットのため wasm-pack には寄せない）。

use wasm_bindgen::prelude::*;

use yuzu_index_format::SearchEngine;

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
    pub fn search(&self, query: &str, limit: usize) -> String {
        let (hits, total) = self.inner.search_with_total(query, limit);
        let hits: Vec<String> = hits
            .into_iter()
            .map(|h| format!(r#"{{"docId":{},"score":{}}}"#, h.doc_id, h.score))
            .collect();
        format!(r#"{{"total":{total},"hits":[{}]}}"#, hits.join(","))
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
