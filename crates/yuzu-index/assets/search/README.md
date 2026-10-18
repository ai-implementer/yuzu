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
  - 通知の crate と版は、wasm を作ったときの依存の版。`scripts/third-party-licenses.sh check`
    （CI）が今の依存（mikan-wasm の wasm32 向け通常依存）と照合し、依存が変わったのに
    作り直していなければ失敗する（Phase 82）

- `wasm-inputs.sha256`: wasm の入力（mikan・mikan-wasm のソースと Cargo.toml、wasm32 向けの
  依存の木と features、ルート Cargo.toml の wasm のプロファイル、`build-search-wasm.sh` 自身）の
  ハッシュ。`build-search-wasm.sh` だけが書き、CI が `build-search-wasm.sh --check` で今の入力と
  照合する（作り直し忘れの検出。Phase 89）。dist へは出さない（`copy_wasm_assets` の一覧に無い）

## 現在の成果物

- 生成日: 2026-10-18（wasm-bindgen 0.2.126 / binaryen version_131 / wasm-opt -Oz / rustc 1.99.1。
  開発コンテナ（Linux aarch64）で生成）。Phase 89 の検索の順位（同義語をまとめて 1 語として
  数える・タイトル一致の倍率）に伴う再 vendor。エクスポート API は変わらない（`search.js` は
  前回とバイト同一）。この回から `--remap-path-prefix` でビルドした人の `$CARGO_HOME` を
  `/cargo` に置き換えたので、wasm に絶対パスが入らない（以前は `~/.cargo/registry` が 45 か所）
- 以前の生成（新しい順。ツールの版はどれも同じ）
  - 2026-10-18: MSRV 1.99 で上げられるようになった globset 0.4.20 を依存ごと上げたとき、wasm に
    入る memchr（2.8.3）と serde の derive が使う proc-macro2・quote・syn・unicode-ident の版が
    変わったことに追随
  - 2026-10-18: MSRV を 1.99 へ上げたときの clippy の修正で mikan の `shard.rs` が
    `chunks_exact(4)` から `as_chunks::<4>()` になったことに追随
  - 2026-10-18: mikan の版を 0.2.2 に上げた（ソースは 0.2.1 と同一）ことに追随。版から
    作られるハッシュの差で 236 バイトが変わった
  - 2026-10-18: ruzstd 0.8.2 → 0.9.1（RUSTSEC-2026-0344。分かち書きモデルの伸長に使う）に追随
  - 2026-10-10: dependabot の cargo の組（serde 1.0.229・serde_json 1.0.151・thiserror 2.0.21）に追随
  - 2026-08-01: Phase 53 の検索結果グループ絞り込み。エクスポート API に `searchIn` / `groups` を
    **追加**した（既存の `search(query, limit)` は据え置き ＝ 固定 URL の HTTP キャッシュに残った
    旧 wasm が引数を黙って無視して「絞り込んでいないのに絞り込んだ件数」を出す事故を避けるため）。
    インデックスフォーマットは v3 のまま（`docGroups` / `groups` は `serde(default)` の後方互換フィールド）
- `search_bg.wasm`: 513KB（524,793 bytes。vaporetto + fst + BM25 エンジン + フレーズ隣接照合 +
  近接ブースト + 動的抜粋 + 同義語クエリ拡張 + 文字単位 Levenshtein DFA
  （levenshtein_automata）+ グループ絞り込み + 同義語のまとめ + タイトル一致の倍率込み。gzip 転送で概ね半分以下）
- `search.js`: 14KB（wasm-bindgen --target web の ES module グルー）
- `search-client.js`: 6.1KB（フェッチ ＋ OPFS キャッシュ ＋ wasm 起動
  オーケストレーション。`groups()` は絞り込み非対応の wasm では空配列を返す）
- `opfs-cache.js`: 2.7KB（汎用 OPFS ブロブキャッシュ）
