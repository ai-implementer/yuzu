# vendor 物の記録: 検索 wasm 成果物

`scripts/build-search-wasm.sh` が生成する `search.js` / `search_bg.wasm`、および
`crates/yuzu-search-wasm/js/` の手書き JS クライアント（`search-client.js` /
`opfs-cache.js`）を置く。`yuzu build` 時に rust-embed 経由で `dist/_search/` へ
コピーされる。

- 成果物が無い場合、インデックス生成は警告を出して wasm のコピーだけスキップする
  （ビルドは失敗させない。`yuzu search` のネイティブ検索は wasm なしで動く）
- 更新手順: `rustup target add wasm32-unknown-unknown`、
  `cargo install wasm-bindgen-cli --version <crates/yuzu-search-wasm の wasm-bindgen と同一>`、
  binaryen（wasm-opt）を用意して `scripts/build-search-wasm.sh` を実行し、
  本ファイルにサイズを記録する

## 現在の成果物

- 生成日: 2026-07-18（wasm-bindgen 0.2.126 / binaryen version_130 / wasm-opt -Oz。
  インデックスフォーマット v3 = postings の出現位置＋`"..."` フレーズ照合＋近接ブースト対応。
  manifest に `contentHash` 追加、OPFS キャッシュ層の JS クライアント同梱）
- `search_bg.wasm`: 494KB（vaporetto + fst + BM25 エンジン + フレーズ隣接照合 +
  近接ブースト + 動的抜粋 + 同義語クエリ拡張 + 文字単位 Levenshtein DFA
  （levenshtein_automata）込み。gzip 転送で概ね半分以下。OPFS 対応前後で実質不変
  ＝ `yuzu-search-wasm` の Cargo 依存・エクスポート API は変えていない）
- `search.js`: 12KB（wasm-bindgen --target web の ES module グルー。
  tokenize / excerpt API 追加）
- `search-client.js`: 4.9KB（フェッチ ＋ OPFS キャッシュ ＋ wasm 起動オーケストレーション。新規）
- `opfs-cache.js`: 2.7KB（汎用 OPFS ブロブキャッシュ。新規）
