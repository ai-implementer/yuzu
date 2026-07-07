# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## プロジェクト概要

yuzu は Markdown の設計書を静的 HTML ドキュメントサイトに変換する Rust 製ツール（Cargo workspace、MSRV 1.85 / edition 2024）。対話・コメント・ドキュメント・テスト名はすべて日本語で書く。コミットはユーザの指示があるまで行わない（push もユーザが行う運用）。

プロジェクトスキル（`.claude/skills/`）: 検証一式は `verify`、実機確認は `run`、tankan の図種追加は `tankan-add-diagram`、vendor 資産更新は `vendor-update` を使う。

## コマンド

```bash
cargo build --workspace
cargo test --workspace                        # insta スナップショットテストを含む
cargo test -p yuzu-core normalize             # 単一 crate・テスト名でフィルタ
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
```

CI（.github/workflows/ci.yml）は上記に加えて wasm32 チェックと e2e を実行する:

```bash
rustup target add wasm32-unknown-unknown
cargo check -p yuzu-search-wasm --target wasm32-unknown-unknown
cargo check -p tankan --target wasm32-unknown-unknown

# e2e（CLI 実機確認）— cargo test は target/debug/yuzu を更新しないので必ず先にビルドする
cargo build -p yuzu-cli
./target/debug/yuzu new /tmp/e2e-docs && cd /tmp/e2e-docs
yuzu build && yuzu check && yuzu search "はじめに"
```

- **insta スナップショット**: 差分が出たら `cargo insta review` で確認して更新。新規スナップショットの一括生成は `INSTA_UPDATE=always cargo test -p tankan`。CI は `INSTA_UPDATE=no` で未承認を失敗にする
- **vendor 更新スクリプト**: `scripts/build-search-wasm.sh`（wasm-bindgen-cli は workspace の `wasm-bindgen = "=x.y.z"` と完全同一バージョン必須）/ `scripts/vendor-mermaid.sh` / `scripts/vendor-vaporetto-model.sh`
- CLI の終了コード規約: 0 = 成功 / 1 = 違反あり（lint・check・fmt --check）/ 2 = 実行エラー

## アーキテクチャ

ワークスペース構成と依存方向は**凍結**（逆方向依存を作らない）:

```
yuzu-cli → {yuzu-server, yuzu-render, yuzu-index, yuzu-core, yuzu-config}
yuzu-render → yuzu-core, tankan     yuzu-index → yuzu-core
yuzu-search-wasm ↔ yuzu-index-format（native/wasm でトークナイザ共有）
tankan は yuzu-* 非依存の汎用ライブラリ（将来 crates.io へ分離可能な設計を維持）
```

- **yuzu-core**: comrak パース → Document/サイトモデル（nav・TOC・slug・sourcepos・lint・リンク検査）。パーサは内部に隠蔽し、公開 API は comrak 非依存
- **yuzu-render**: サイトモデル → HTML（minijinja テンプレート、syntect ハイライト、Mermaid 変換、baseUrl 解決）
- **yuzu-config**: `yuzu.jsonc` を cwd から上方向に探索してプロジェクトルートを確定 → 解決済み設定を `.yuzu/settings.json` に書き出す
- **yuzu-theme**: デフォルトテーマを rust-embed でバイナリ埋め込み。プロジェクトの `theme/` に同じ相対パスのファイルを置くとファイル単位で上書き
- **tankan**: Mermaid 互換 SSR（sequence / flowchart / state / ER / gantt → SVG）。render_svg が Err を返すと yuzu 側が自動でクライアント描画にフォールバックするため、図種追加は tankan の `kind.rs::is_supported` と `lib.rs` の match アーム接続だけでよい

### 凍結した設計判断（README「凍結した設計判断」参照。差し替えないこと）

comrak（Markdown）/ minijinja（テンプレート）/ syntect + two-face（ハイライト、CSS クラス出力）/ clap derive / serde + JSONC / rust-embed / axum + notify + WebSocket（dev サーバ）。comrak・syntect・two-face は onig（C 依存）を引かないよう **必ず `default-features = false`**（Cargo.toml のコメント参照）。

### 検索の最重要制約

index 時（ネイティブ）と query 時（wasm）で**同一トークナイザコード（yuzu-index-format）＋同一モデルバイト**を使うこと。`yuzu search` はブラウザと同じエンジンを通るので整合検証に使える。検索 UI の動作確認は `yuzu preview` / `yuzu dev` 経由（`file://` では fetch が動かない）。

### tankan の設計原則

I/O なし・時刻/乱数非依存（wasm32 担保のため。gantt の today 線は意図的に描かない）。日付演算は `common/date.rs`（依存なし）。corpus テストは `crates/tankan/tests/corpus/<図種>/*.mmd` 全件受理＋代表例の insta スナップショット。SVG のテーマ追従は `<style>`＋CSS 変数方式（SVG 属性内の var() は仕様上不可）。

## 罠・注意点

- `cargo test --workspace` は `target/debug/yuzu` を**更新しない**。CLI の実機確認前に `cargo build -p yuzu-cli` を忘れない
- yuzu-server の serve テストは TCP バインドするため、サンドボックス内では PermissionDenied で落ちる（コード起因ではない）
- rust-embed は debug ビルドだとテーマをファイルシステムから読む（テーマ編集が再コンパイル不要で反映される一方、debug バイナリ単体を別マシンへ持ち出すとアセットを見失う）。リリースビルドは常に埋め込み
- minijinja はデフォルトで属性中の `/` をエスケープするため、テンプレートの URL 値には `| safe` を通している
- comrak 0.53 API: `render.r#unsafe`（unsafe_ ではない）/ `header_id_prefix`（header_ids は deprecated）/ `format_html` は fmt::Write（String）出力
- fmt/lint/check は **draft ページも対象**（build_source_pages）。build_site_model は従来どおり draft を除外する
- `yuzu fmt` の不変条件: 本文は format_commonmark の正規形・**frontmatter は生テキストをバイト温存**・冪等・差分なしなら書き込まない（mtime 温存）
- `docs/design/` は git 管理外のローカル設計ノート。公開物（コード・README・コミット）から参照しない
