# vendor 物の記録: 検索 wasm 成果物

`scripts/build-search-wasm.sh` が生成する `search.js` / `search_bg.wasm`、および
`crates/mikan-wasm/js/` の手書き JS クライアント（`search-client.js` /
`opfs-cache.js`）を置く。`yuzu build` 時に rust-embed 経由で `dist/_search/` へ
コピーされる。

- 成果物が無い場合、インデックス生成は警告を出して wasm のコピーだけスキップする
  （ビルドは失敗させない。`yuzu search` のネイティブ検索は wasm なしで動く）
- 更新手順: `rustup target add wasm32-unknown-unknown`、
  `cargo install wasm-bindgen-cli --version <crates/mikan-wasm の wasm-bindgen と同一>`、
  binaryen（wasm-opt）・cargo-about を用意して `scripts/build-search-wasm.sh` を実行し、
  本ファイルにサイズを記録する
- `THIRD-PARTY-LICENSES.txt`: wasm に入る第三者 crate（mikan-wasm の wasm32 向け通常依存）と
  分かち書きモデルのライセンス文。`build-search-wasm.sh` が wasm と一緒に作り直す
  （文だけなら `scripts/third-party-licenses.sh wasm`）。`dist/_search/` へ一緒に出る
  - 2026-10-10 に追加したときは wasm を作り直さず、その時点の Cargo.lock から生成した。
    wasm の生成時（2026-08-01）と依存の版が一部違う可能性がある（crate の顔ぶれと
    ライセンスは同じ）。次に wasm を作り直すと揃う

## 現在の成果物

- 生成日: 2026-08-01（wasm-bindgen 0.2.126 / binaryen version_131 / wasm-opt -Oz。
  Phase 53 の検索結果グループ絞り込みに伴う再 vendor。エクスポート API に
  `searchIn` / `groups` を**追加**した（既存の `search(query, limit)` は据え置き
  ＝ 固定 URL の HTTP キャッシュに残った旧 wasm が引数を黙って無視して
  「絞り込んでいないのに絞り込んだ件数」を出す事故を避けるため）。
  インデックスフォーマットは v3 のまま（`docGroups` / `groups` は
  `serde(default)` の後方互換フィールド））
- `search_bg.wasm`: 503KB（vaporetto + fst + BM25 エンジン + フレーズ隣接照合 +
  近接ブースト + 動的抜粋 + 同義語クエリ拡張 + 文字単位 Levenshtein DFA
  （levenshtein_automata）+ グループ絞り込み込み。gzip 転送で概ね半分以下）
- `search.js`: 14KB（wasm-bindgen --target web の ES module グルー）
- `search-client.js`: 6.1KB（フェッチ ＋ OPFS キャッシュ ＋ wasm 起動
  オーケストレーション。`groups()` は絞り込み非対応の wasm では空配列を返す）
- `opfs-cache.js`: 2.7KB（汎用 OPFS ブロブキャッシュ）
