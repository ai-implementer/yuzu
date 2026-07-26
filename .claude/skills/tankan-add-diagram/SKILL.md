---
name: tankan-add-diagram
description: tankan（Mermaid 互換 SSR ライブラリ）に新しい図種のサポートを追加するレシピ。journey 等の図種追加や既存図種の構文拡張のときに使う。tankan 内の実装に加えて yuzu 側（キャッシュ bump・スナップショット・対応表）の追随も含む。
---

# tankan 図種追加レシピ

tankan は yuzu 非依存の汎用ライブラリ（`crates/tankan`）で、crates.io 公開物でもある。
yuzu 側は `render_svg` が Err を返すと自動でクライアント描画へフォールバックするため、
**未対応のままでも壊れない**。ただし図種を足すと「今までフォールバックしていたページが
SSR 成功へ変わる」＝本文 HTML が変わるので、**yuzu 側にも追随作業がある**。

## A. tankan 内（5 箇所）

1. **モジュール実装**: `crates/tankan/src/<図種>/` を新設。参考にする既存実装は目的別に:
   - `sequence` — 独立エンジン
   - `flowchart` — Sugiyama 法。`state` がこのエンジンを共用する例
   - `mindmap` / `timeline` — tidy tree 系（Sugiyama とは別系統）
   - `pie` / `er` / `gantt` — 単純レイアウト
2. **図種判定**: `crates/tankan/src/kind.rs` の `is_supported` に追加。同ファイルの
   `is_supported_の対応図種` テストにも追記する
3. **接続**: `crates/tankan/src/lib.rs` の `mod` 宣言と match アーム
4. **corpus テスト**:
   - `crates/tankan/tests/corpus/<図種>/*.mmd` を番号付き（`01-basic.mmd` …）で追加。
     mermaid 公式ドキュメントの構文例をカバーする
   - **`crates/tankan/tests/corpus_test.rs` の `CORPORA` 定数へ代表例を登録する**
     （`("mindmap", &["01-basic", "02-shapes", "05-japanese"])` の形。
     **登録しないとスナップショットが生成されない**）
   - `corpus/fallback/` に「未対応構文はフォールバックする」ネガティブ例も足すのが定石
   - 生成: `INSTA_UPDATE=always cargo test -p tankan`
5. **README の対応状況表**: `crates/tankan/README.md` は crates.io ユーザ向けの一次情報。
   CI の `cargo package` は中身の陳腐化を検出できないので手で直す

## B. yuzu 側（忘れると静かに壊れる）

1. **`CACHE_FORMAT_VERSION` を上げる**（`crates/yuzu-core/src/cache.rs`）。
   従来フォールバックだったページが SSR 成功へ変わる＝**本文 HTML の意味が変わる**ため。
   doc コメントの履歴にも 1 行足す（前例: `v7: mindmap / timeline の SSR 追加`）
2. `crates/yuzu-render/src/highlight.rs` と `tests/render_snapshot.rs` の追随
   （SSR 対象が増えるとスナップショットが動く）
3. `README.md` と `CLAUDE.md` の対応図種の列挙、`crates/yuzu-cli/scaffold/`（「9 図種」等の表記）
4. `cargo check -p tankan --target wasm32-unknown-unknown`（CI でも検査される）

## 共通ヘルパ（`crates/tankan/src/common/`）— 先に読む

- **`style.rs` が最重要**。ユーザ指定色（classDef / `class` / `:::` / `style`）の
  パース・マージ・解決・属性生成・**fill 明度からの文字色自動選択**を 1 実装で集約している
  （`Style` / `StyleCollector` / `box_attr` / `line_attr` / `text_attr`）。
  各図種パーサは**薄いアダプタとして呼ぶだけ**にする。自前で色処理を書かない
- `text.rs` — テキスト計測・折り返し（日本語幅を含む）
- `date.rs` — 日付演算（Howard Hinnant civil calendar、依存なし）
- `layered.rs` / `geom.rs` / `path.rs` / `svg.rs` — レイアウトと SVG 出力の下回り

## 守るべき設計原則

- **I/O なし・時刻/乱数非依存**（wasm32 担保のため）。gantt の today 線は意図的に描かない
  （`todayMarker off` のみ受理）。`Date.now()` 相当が必要な機能は入れない
- **テーマ追従は `<style>` ＋ CSS 変数方式**。SVG 属性内の `var()` は仕様上効かないので
  属性へ色を直書きしない
- **ユーザ指定色はインライン style 属性で直接埋める**（テーマ非追従が正）。
  `<style>` へ追記する方式は、同一ページに複数 SVG があるとルールが衝突するため不可
- SVG の well-formed 検証は roxmltree（dev 用テスト）
- yuzu-* クレートに依存しない（crates.io 単独公開を維持）

## 参考挙動（mermaid 互換で注意した点）

- gantt の開始日省略はセクションを**跨いでも**直前タスクの終了に続く
