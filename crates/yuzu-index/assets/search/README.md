# vendor 物の記録: 検索 wasm 成果物

`scripts/build-search-wasm.sh` が生成する `search.js` / `search_bg.wasm` を置く。
`yuzu build` 時に rust-embed 経由で `dist/_search/` へコピーされる。

- 成果物が無い場合、インデックス生成は警告を出して wasm のコピーだけスキップする
  （ビルドは失敗させない。`yuzu search` のネイティブ検索は wasm なしで動く）
- 更新手順: `rustup target add wasm32-unknown-unknown`、
  `cargo install wasm-bindgen-cli --version <crates/yuzu-search-wasm の wasm-bindgen と同一>`、
  binaryen（wasm-opt）を用意して `scripts/build-search-wasm.sh` を実行し、
  本ファイルにサイズを記録する

## 現在の成果物

- 生成日: 2026-07-18（wasm-bindgen 0.2.126 / binaryen version_130 / wasm-opt -Oz。
  インデックスフォーマット v3 = postings の出現位置＋`"..."` フレーズ照合対応）
- `search_bg.wasm`: 492KB（vaporetto + fst + BM25 エンジン + フレーズ隣接照合 +
  動的抜粋 + 同義語クエリ拡張 + 文字単位 Levenshtein DFA（levenshtein_automata）込み。
  gzip 転送で概ね半分以下）
- `search.js`: 12KB（wasm-bindgen --target web の ES module グルー。
  tokenize / excerpt API 追加）
