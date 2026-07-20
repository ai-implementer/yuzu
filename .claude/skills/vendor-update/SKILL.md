---
name: vendor-update
description: vendor 資産（検索 wasm 成果物・mermaid.min.js・KaTeX・vaporetto 分かち書きモデル）の更新手順。wasm-bindgen のバージョンピン照合を含む。依存更新やアセット差し替えのときに使う。
---

# vendor 資産の更新手順

4 種類の vendor 資産があり、それぞれ更新スクリプトが `scripts/` にある。

## 1. 検索 wasm 成果物（crates/yuzu-index/assets/search/）

```bash
scripts/build-search-wasm.sh
```

- 前提ツール: wasm32 target（`rustup target add wasm32-unknown-unknown`）、wasm-bindgen-cli、binaryen（wasm-opt）。
- **最重要: wasm-bindgen-cli は workspace の `wasm-bindgen = "=x.y.z"`（Cargo.toml でピン留め）と完全同一バージョン必須**。スクリプトが照合して不一致なら失敗する。crate 側を上げるときは
  ```bash
  cargo install wasm-bindgen-cli --version <同一バージョン>
  ```
  を併せて実行し、Cargo.toml の `=` ピンとスクリプトの整合を保つ。
- 更新後の検証: `yuzu build` → `yuzu search <クエリ>`。ネイティブと wasm は `mikan` の**同一トークナイザコード＋同一モデルバイト**を使う制約があり、`yuzu search` はブラウザと同じエンジンを通るので整合検証になる。

## 2. mermaid.min.js（crates/yuzu-theme/assets/static/vendor/）

```bash
scripts/vendor-mermaid.sh
```

- 約 3.4MB。`backend: "ssr"` 運用でも未対応図種のフォールバック用に同梱は継続する。
- 更新後は client 描画ページ（`run` スキル参照）で図が描画されることを確認。

## 3. KaTeX（crates/yuzu-theme/assets/static/vendor/katex/）

```bash
scripts/vendor-katex.sh
```

- katex.min.js / katex.min.css / fonts で約 600KB。**fonts は woff2 のみ同梱**（css は woff2 → woff → ttf の順で参照するが、モダンブラウザは woff2 しか取得しない）。
- css が `url(fonts/...)` を相対参照するため `katex/` のディレクトリ構造を崩さないこと。
- 更新後は `run` スキルで数式ページ（scaffold の getting-started「記法サンプル > 数式」）のライト/ダーク描画と、fonts が 404 なく取得されることを確認。
- 未取得でもビルド・テストは通り、数式は原文（TeX ソース）表示になるだけ。

## 4. vaporetto モデル（crates/mikan/assets/model/）

```bash
scripts/vendor-vaporetto-model.sh
```

- 現行: bccwj-suw_c1.0（圧縮 372KB、MIT OR Apache-2.0）。ライセンスが再配布可能なものだけを使う。
- **モデルのバイト列が変わると索引（index 時）と検索（query 時）の整合が崩れる**。更新後は必ずサイトを再ビルドし、`yuzu search`（誤字クエリ込み）で確認する。ブラウザは初回検索時にモデルを遅延ダウンロードする設計。

## 共通の注意

- vendor 更新は生成物の差分が大きい。コミットは vendor 更新単独で分け、由来（スクリプト・バージョン）をコミットメッセージに書く。
- 最後に `verify` スキルの一式（特に wasm check と e2e の検索）を通す。
