# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## プロジェクト概要

yuzu は Markdown の設計書を静的 HTML ドキュメントサイトに変換する Rust 製ツール（Cargo workspace、MSRV 1.85 / edition 2024）。対話・コメント・ドキュメント・テスト名はすべて日本語で書く。コミットはユーザの指示があるまで行わない（push もユーザが行う運用）。

プロジェクトスキル（`.claude/skills/`）: 検証一式は `verify`、実機確認は `run`、tankan の図種追加は `tankan-add-diagram`、vendor 資産更新は `vendor-update`、開発コンテナ・apple container 操作は `apple-container` を使う。

## コマンド

```bash
cargo build --workspace
cargo test --workspace                        # insta スナップショットテストを含む
cargo test -p yuzu-core normalize             # 単一 crate・テスト名でフィルタ
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
```

CI（.github/workflows/ci.yml）は上記に加えて wasm32 チェック・e2e・docs サイト検証
（docs/ での check・build・SSR フォールバック検出）を実行する:

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
- **vendor 更新スクリプト**: `scripts/build-search-wasm.sh`（wasm-bindgen-cli は workspace の `wasm-bindgen = "=x.y.z"` と完全同一バージョン必須）/ `scripts/vendor-mermaid.sh` / `scripts/vendor-katex.sh` / `scripts/vendor-vaporetto-model.sh`
- CLI の終了コード規約: 0 = 成功 / 1 = 違反あり（lint・check・fmt --check）/ 2 = 実行エラー

## アーキテクチャ

ワークスペース構成と依存方向は**凍結**（逆方向依存を作らない）:

```
yuzu-cli → {yuzu-server, yuzu-render, yuzu-index, yuzu-core, yuzu-config}
yuzu-render → yuzu-core, tankan     yuzu-index → yuzu-core, yuzu-index-format
yuzu-search-wasm ↔ yuzu-index-format（native/wasm でトークナイザ・フォーマット共有）
tankan・yuzu-index-format・yuzu-search-wasm は他の yuzu crate 非依存の汎用ライブラリ
（将来 crates.io/npm へ分離可能な設計を維持。検索スタックの書き側集約は
yuzu-index-format::build、読み側クエリエンジンは SearchEngine にあり、
yuzu-index はページ抽出とファイル I/O だけの薄い呼び出し側）
```

- **yuzu-core**: comrak パース → Document/サイトモデル（nav・TOC・slug・sourcepos・lint・リンク検査）。パーサは内部に隠蔽し、公開 API は comrak 非依存
- **yuzu-render**: サイトモデル → HTML（minijinja テンプレート、syntect ハイライト、Mermaid 変換、数式は comrak math 出力を同梱 KaTeX がクライアント描画、baseUrl 解決）
- **yuzu-config**: `yuzu.jsonc` を cwd から上方向に探索してプロジェクトルートを確定 → 解決済み設定を `.yuzu/settings.json` に書き出す
- **yuzu-theme**: デフォルトテーマを rust-embed でバイナリ埋め込み。プロジェクトの `theme/` に同じ相対パスのファイルを置くとファイル単位で上書き
- **tankan**: Mermaid 互換 SSR（sequence / flowchart / class / state / ER / gantt / pie / mindmap / timeline → SVG）。render_svg が Err を返すと yuzu 側が自動でクライアント描画にフォールバックするため、図種追加は tankan の `kind.rs::is_supported` と `lib.rs` の `mod` 宣言＋match アーム接続だけでよい

### 凍結した設計判断（README「凍結した設計判断」参照。差し替えないこと）

comrak（Markdown）/ minijinja（テンプレート）/ syntect + two-face（ハイライト、CSS クラス出力）/ clap derive / serde + JSONC / rust-embed / axum + notify + WebSocket（dev サーバ）。comrak・syntect・two-face は onig（C 依存）を引かないよう **必ず `default-features = false`**（Cargo.toml のコメント参照）。

### 検索の最重要制約

index 時（ネイティブ）と query 時（wasm）で**同一トークナイザコード（yuzu-index-format）＋同一モデルバイト**を使うこと。抜粋生成・ハイライトのロジックも yuzu-index-format に 1 実装で native/wasm 共有する（別実装を作らない）。`yuzu search` はブラウザと同じエンジンを通るので整合検証に使える。検索 UI の動作確認は `yuzu preview` / `yuzu dev` 経由（`file://` では fetch が動かない）。

### インクリメンタルビルドの層構造

`RenderCtx` / `IndexCtx` の**全フィールド None = 従来のフルビルドと同一動作**（ライブラリ単体テストはこの形。キャッシュ配線は cli 層の責務）。

- **yuzu-core**: `cache.rs`（ページ派生物キャッシュ。envKey / routesKey / sourceHash の 3 層無効化）＋ `output.rs`（compare-before-write・出力マニフェスト・孤児掃除）
- **yuzu-render**: `RenderCtx`（cache / outputs / shared）と `RenderShared`（watch 間で再利用する minijinja Env・syntect）
- **yuzu-index**: `IndexCtx` と `IndexSession`（vaporetto トークナイザの遅延構築・再利用）
- **yuzu-cli** `commands/build.rs`: `BuildSession` が上記を束ね、envKey 計算・routesKey 設定・マニフェスト保存を行う唯一の場所

キャッシュするのは高価なページ派生物（メタ・本文 HTML・検索 tf・llms 正規化 md）だけで、nav / fst / llms 連結などの集約は毎回全実行する（クロスページ依存を依存解析なしで正しく保つための分離。README「インクリメンタルビルドの実装メモ」参照）。

### tankan の設計原則

I/O なし・時刻/乱数非依存（wasm32 担保のため。gantt の today 線は意図的に描かない）。日付演算は `common/date.rs`（依存なし）。corpus テストは `crates/tankan/tests/corpus/<図種>/*.mmd` 全件受理＋代表例の insta スナップショット。SVG のテーマ追従は `<style>`＋CSS 変数方式（SVG 属性内の var() は仕様上不可）。ユーザ指定色（flowchart / state / ER / class の classDef / class(cssClass) / `:::` / style）はインライン style 属性で直接埋める（テーマ非追従が正。`<style>` 追記方式は同一ページの複数 SVG でルールが衝突するため不可）。パース・マージ・解決・属性生成・fill 明度からの文字色自動選択は `common/style.rs`（`Style` / `StyleCollector` / `box_attr` / `line_attr` / `text_attr`）に 1 実装で集約し、各図種パーサは薄いアダプタで呼ぶ。

## 罠・注意点

- `cargo test --workspace` は `target/debug/yuzu` を**更新しない**。CLI の実機確認前に `cargo build -p yuzu-cli` を忘れない
- `yuzu build` / `dev` は常時インクリメンタル（`.yuzu/cache/`）。キャッシュ起因の不具合を疑うときは `--force`（または `.yuzu/cache/` 削除。いつでも安全）。**キャッシュ内容の意味が変わる変更**（本文 HTML の生成ロジック・検索 tf の重み等）では `yuzu-core/src/cache.rs` の `CACHE_FORMAT_VERSION` を上げる
- yuzu-server の serve テストは TCP バインドするため、サンドボックス内では PermissionDenied で落ちる（コード起因ではない）
- rust-embed は debug ビルドだとテーマをファイルシステムから読む（テーマ編集が再コンパイル不要で反映される一方、debug バイナリ単体を別マシンへ持ち出すとアセットを見失う）。リリースビルドは常に埋め込み。**埋め込みフォルダへの新規ファイル追加は cargo の再コンパイル判定に載らない**ため、yuzu-theme は build.rs の `rerun-if-changed=assets` で監視している（これが無いと「debug では動くのに release が古い埋め込みを使い回して template not found」になる。埋め込み crate を増やすときは同じ build.rs を付けること）
- minijinja はデフォルトで属性中の `/` をエスケープするため、テンプレートの URL 値には `| safe` を通している
- comrak 0.53 API: `render.r#unsafe`（unsafe_ ではない）/ `header_id_prefix`（header_ids は deprecated）/ `format_html` は fmt::Write（String）出力
- fmt/lint/check は **draft ページも対象**（build_source_pages）。build_site_model は従来どおり draft を除外する
- `yuzu fmt` の不変条件: 本文は format_commonmark の正規形・**frontmatter は生テキストをバイト温存**・冪等・差分なしなら書き込まない（mtime 温存）
- `docs/` はこのリポジトリ自身のドキュメントサイト（yuzu プロジェクト。`docs/yuzu.jsonc` がルート）。main push で `.github/workflows/docs.yml` が GitHub Pages へデプロイし、ci.yml でも check・build・SSR フォールバック検出を検証する。原稿は `yuzu fmt` の正規形・表記は長音符なし（`lint.terms` 準拠）で書く
- `docs/design/` は git 管理外のローカル設計ノート。公開物（コード・README・コミット）から参照しない
- 開発コンテナ内（`.devcontainer/`）は `CARGO_TARGET_DIR=/cargo-target` のため、CLI 実機確認は `"$CARGO_TARGET_DIR/debug/yuzu"` を使う（`./target/debug/yuzu` は**存在しない**）。環境定義は `.devcontainer/Dockerfile` が唯一で、devcontainer.json とラッパーの不変条件は `.devcontainer/README.md` の表を参照
