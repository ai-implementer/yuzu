---
name: tankan-add-diagram
description: tankan（Mermaid 互換 SSR ライブラリ）に新しい図種のサポートを追加するレシピ。pie / journey 等の図種追加や既存図種の構文拡張のときに使う。
---

# tankan 図種追加レシピ

tankan は yuzu 非依存の汎用ライブラリ（crates/tankan）。yuzu 側は `render_svg` が Err を返すと自動でクライアント描画にフォールバックするため、**yuzu 側の接続作業は不要**。tankan 内の 2 箇所をつなぐだけでよい。

## 手順

1. **モジュール実装**: `crates/tankan/src/<図種>/` を新設。既存実装を参考にする — sequence（独立エンジン）、flowchart（Sugiyama 法）、state（flowchart エンジン共用の例）、er / gantt。
2. **図種判定**: `crates/tankan/src/kind.rs` の `is_supported` に図種キーワードを追加。
3. **接続**: `crates/tankan/src/lib.rs` の match アームに新モジュールをつなぐ。
4. **corpus テスト**: `crates/tankan/tests/corpus/<図種>/*.mmd` を追加。形式は「全件受理＋代表例の insta スナップショット」。番号付きファイル名（`01-basic.mmd` …）で mermaid 公式ドキュメントの構文例をカバーする。新規スナップショットは:
   ```bash
   INSTA_UPDATE=always cargo test -p tankan
   ```
5. **wasm 担保**: `cargo check -p tankan --target wasm32-unknown-unknown`（CI でも検査される）。

## 守るべき設計原則

- **I/O なし・時刻/乱数非依存**（wasm32 担保のため）。gantt の today 線は意図的に描かない（`todayMarker off` のみ受理）。`Date.now()` 相当が必要な機能は入れない。
- 日付演算は `crates/tankan/src/common/date.rs`（Howard Hinnant civil calendar、依存なし）を使う。
- SVG のテーマ追従は `<style>` ＋ CSS 変数方式。**SVG 属性内の `var()` は仕様上不可**なので属性に直接色を書かない。
- SVG の well-formed 検証は roxmltree（dev 用テスト）。
- yuzu-* クレートに依存しない（将来 crates.io へ分離できる設計を維持）。

## 参考挙動（mermaid 互換で注意した点）

- gantt の開始日省略はセクションを**跨いでも**直前タスクの終了に続く。
