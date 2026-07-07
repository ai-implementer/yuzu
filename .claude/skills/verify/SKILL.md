---
name: verify
description: yuzu の変更を検証する。CI 相当（fmt / clippy / test / wasm check）＋ CLI 実機 e2e を、既知の罠を回避した正しい順序で実行する。コード変更後の検証・コミット前チェックで使う。
---

# yuzu 検証手順

CI（.github/workflows/ci.yml）と同等＋実機 e2e。上から順に実行する。

## 1. 静的チェック

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
```

## 2. テスト

```bash
cargo test --workspace --exclude yuzu-server
cargo test -p yuzu-server   # ← サンドボックス外で実行する
```

- **yuzu-server はサンドボックス外必須**: serve テストが TCP バインドするため、サンドボックス内では PermissionDenied で落ちる（コード起因ではない）。
- **insta スナップショット差分が出たら**: 差分が意図どおりか必ず目視 → `INSTA_UPDATE=always cargo test -p <crate>` で更新 → `git diff` で更新内容を再確認。意図しない差分は変更側のバグを疑う。CI は `INSTA_UPDATE=no` で未承認を失敗にする。

## 3. wasm32 チェック

```bash
cargo check -p yuzu-search-wasm --target wasm32-unknown-unknown
cargo check -p tankan --target wasm32-unknown-unknown
```

## 4. e2e（CLI 実機）

**罠: `cargo test --workspace` は `target/debug/yuzu` を更新しない。必ず先にビルドする。**

```bash
cargo build -p yuzu-cli
./target/debug/yuzu new "<scratchpad>/e2e-docs"
cd "<scratchpad>/e2e-docs"
<repo>/target/debug/yuzu build
test -f dist/index.html && test -f dist/_search/manifest.json && test -f dist/_search/search_bg.wasm
<repo>/target/debug/yuzu search "はじめに" | grep "はじめに"
<repo>/target/debug/yuzu fmt --check && <repo>/target/debug/yuzu lint && <repo>/target/debug/yuzu check
# 異常系: 壊れリンクを注入して check が終了コード 1 を返すこと（CI と同じ）
echo '[壊れリンク](missing.md)' >> content/index.md
<repo>/target/debug/yuzu check && echo "NG: 検出漏れ" || echo "OK"
```

終了コード規約: 0 = 成功 / 1 = 違反あり / 2 = 実行エラー。

## 5. UI・テーマ・scaffold の変更がある場合

`run` スキル（プロジェクト版）でブラウザ配信まで実機確認する。
