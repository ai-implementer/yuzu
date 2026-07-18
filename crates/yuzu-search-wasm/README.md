# yuzu-search-wasm

yuzu のクライアント検索クエリエンジン（`wasm32-unknown-unknown` の wasm-bindgen ラッパ）。
**ロジックは持たない**薄い層で、エンジン本体・トークナイザ・フォーマットはすべて
[`yuzu-index-format`](../yuzu-index-format) にあり、ネイティブの `yuzu search` と
同一コードを共有する（トークナイザ整合の保証。README「検索まわりの実装メモ」参照）。

## `js/`: 同梱の JS クライアント

`js/search-client.js` / `js/opfs-cache.js` は、この wasm ラッパを **フェッチ ＋ OPFS
キャッシュ ＋ wasm 起動**込みでブラウザから呼び出すための手書きの対になる実装。
DOM やテーマの知識は一切持たない（yuzu-theme の `search-ui.js` はこれを import して
使うだけの UI 層に純化されている）:

- **`search-client.js`**: `createSearchClient({ searchBase })` が
  `{ ensureEngine, search, fetchFragment, tokenize, excerpt }` を返す。
  manifest.json は毎回ネットワークから取得し、`contentHash`（OPFS に保存済みの
  前回 manifest との比較）が一致すれば `terms.fst`/`model.zst`/シャードを
  OPFS から読んでネットワーク往復を省く。不一致・未対応環境では従来どおり
  常にフェッチする経路へ自然にフォールバックする
- **`opfs-cache.js`**: 検索フォーマットを一切知らない汎用 OPFS ブロブキャッシュ。
  `navigator.storage.getDirectory` が無い、または操作が一度でも失敗したら
  そのページセッションでは恒久的に無効化し、呼び出し側は透過的にフェッチのみへ縮退する

`scripts/build-search-wasm.sh` が `search.js`/`search_bg.wasm`（wasm-bindgen 生成物）と
一緒にこの2ファイルを `crates/yuzu-index/assets/search/`（`dist/_search/` へ rust-embed
経由でコピーされる vendor 先）へコピーする。

## 使い方（JS 側）

```js
import { createSearchClient } from "./search-client.js";

const client = createSearchClient({ searchBase: "/_search/" });
const { total, hits } = await client.search("検索", 10);
const fragments = await Promise.all(hits.map((h) => client.fetchFragment(h.docId)));
```
