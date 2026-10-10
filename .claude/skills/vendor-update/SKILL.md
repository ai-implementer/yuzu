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

- 前提ツール: wasm32 target（`rustup target add wasm32-unknown-unknown`）、wasm-bindgen-cli、binaryen（wasm-opt）、cargo-about（版は `scripts/third-party-licenses.sh` の `ABOUT_VERSION`。`--features cli` で入れる）。
- wasm と一緒に `THIRD-PARTY-LICENSES.txt`（wasm に入る crate とモデルのライセンス文。dist の `_search/` へ出る）を作り直す。wasm を変えずに文だけ作り直すなら `scripts/third-party-licenses.sh wasm`。
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
- npm の tarball（sha256 固定）から取る。mermaid.min.js は約 60 の npm パッケージを束ねているので、
  スクリプトが tarball 同梱の source map から束ねたパッケージを取り出し、各パッケージの
  LICENSE を集めて `crates/yuzu-theme/licenses/mermaid.txt` を書き、`THIRD-PARTY-LICENSES.txt`
  （dist の `_assets/vendor/`）を組み直す。**jq が要る**。LICENSE ファイルが無いパッケージは README の
  License 節から取り、著作権表示と許諾文がそろわなければ止まる（手で調べて対処する）
- 更新後は client 描画ページ（`run` スキル参照）で図が描画されることを確認。

## 3. KaTeX（crates/yuzu-theme/assets/static/vendor/katex/）

```bash
scripts/vendor-katex.sh
```

- katex.min.js / katex.min.css / fonts で約 600KB。**fonts は woff2 のみ同梱**（css は woff2 → woff → ttf の順で参照するが、モダンブラウザは woff2 しか取得しない）。
- css が `url(fonts/...)` を相対参照するため `katex/` のディレクトリ構造を崩さないこと。
- 更新後は `run` スキルで数式ページ（scaffold の getting-started「記法サンプル > 数式」）のライト/ダーク描画と、fonts が 404 なく取得されることを確認。
- 未取得でもビルド・テストは通り、数式は原文（TeX ソース）表示になるだけ。
- ライセンス文（本体の LICENSE とフォントの生成元 katex-fonts の LICENSE）を
  `crates/yuzu-theme/licenses/katex.txt` に書き、`THIRD-PARTY-LICENSES.txt` を組み直す。

## 4. vaporetto モデル（crates/mikan/assets/model/）

```bash
scripts/vendor-vaporetto-model.sh
```

- 現行: bccwj-suw_c1.0（圧縮 372KB、MIT OR Apache-2.0）。ライセンスが再配布可能なものだけを使う。
- アーカイブ同梱の `LICENSE-MIT` / `LICENSE-APACHE` もモデルの隣に保存する（mikan の crate と
  dist の `_search/THIRD-PARTY-LICENSES.txt` が使う）。モデルを替えたら `scripts/third-party-licenses.sh wasm` も流す。
- **モデルのバイト列が変わると索引（index 時）と検索（query 時）の整合が崩れる**。更新後は必ずサイトを再ビルドし、`yuzu search`（誤字クエリ込み）で確認する。ブラウザは初回検索時にモデルを遅延ダウンロードする設計。

## 5. 公式 toml-test（crates/kabosu/tests/toml-test/）

```bash
scripts/vendor-toml-test.sh
```

- 現行: `v2.2.0`（MIT）。884 ファイル・実バイト 190KB。`crates/kabosu/Cargo.toml` の
  `exclude` で配布物からは外している。
- **タグはテストスイートの版であって TOML 仕様の版ではない**。仕様の版で選ぶのは上流の
  `tests/files-toml-1.0.0` で、スクリプトはここに載っているファイルだけを取り込む。
  TOML 1.1 専用のケースを入れると「仕様どおり拒否したのに落ちる」テストになる。
- 更新後は `cargo test -p kabosu --test toml_test` を回す。落ちたケースは
  kabosu 側のバグか、上流がケースを追加したかのどちらか（差分を読んで判断する）。

## いつ再 vendor するか

- **`mikan` / `mikan-wasm` に手を入れたら検索 wasm を必ず再生成する**。索引側（ネイティブ）と
  クエリ側（wasm）が同一コードであることが検索の最重要制約なので、片方だけ新しい状態を作らない。
  実運用でも Phase 30 / 31 / 34 / 35 の 4 回とも、検索側の変更コミットの直後に
  `vendor: 検索 wasm 成果物を再生成` の独立コミットが入っている
- mermaid / KaTeX は上流のバージョン更新時のみ。tankan の SSR 対応が増えても mermaid は
  未対応図種のフォールバック用に同梱を続ける

## 共通の注意

- **vendor ディレクトリの README が provenance の一次情報**。スクリプトを流したら必ず更新する:
  - `crates/yuzu-index/assets/search/README.md` — 成果物のサイズ（`build-search-wasm.sh` 自身が「実行後に記録すること」と要求している）
  - `crates/mikan/assets/model/README.md` — 取得元 URL・ライセンス・取得日・**sha256**・圧縮サイズ
  - `crates/yuzu-theme/assets/static/vendor/README.md` — mermaid / KaTeX の取得バージョンと日付
- vendor 更新は生成物の差分が大きい。コミットは vendor 更新単独で分け、由来（スクリプト・バージョン）をコミットメッセージに書く。
- mermaid / KaTeX を更新したら `scripts/vendor-advisories.sh` で既知の勧告を照合する（Phase 82。
  照合する版は `crates/yuzu-theme/licenses/` の記録から読むので、vendor スクリプトの後に流す）。
  mermaid の版が変わると、束ねたパッケージの版も変わってスクリプト冒頭の `IGNORE`（勧告と
  パッケージ@版の組）が外れる。残った勧告を読み直し、影響しないものだけ理由を書いて足し直す。
  deps.yml が週次で同じ照合をする。
- 検索 wasm の依存（vaporetto・fst・serde_json など）の版が Cargo.lock で変わったら、
  wasm を作り直す（`scripts/third-party-licenses.sh check` が通知の版と Cargo.lock を照合して知らせる）。
- 最後に `verify` スキルの一式（特に wasm check と e2e の検索）を通す。
