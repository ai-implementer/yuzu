---
name: publish-crate
description: 汎用ライブラリ（tankan / mikan / kabosu）を crates.io へ公開する手順。yuzu のリリースとは非同期で、変更が溜まったときだけ行う。kabosu は publish 前の fuzz 必須。バージョンを上げて publish するときに使う。
---

# 汎用ライブラリの crates.io 公開

対象は monorepo 内の 3 crate。バージョンは workspace と独立で、各 `Cargo.toml` の
`version` を明示指定している（公開済み: tankan 0.2.0 / mikan 0.2.0 / kabosu 0.1.0）。

| crate | 役割 | 備考 |
| --- | --- | --- |
| tankan | Mermaid 互換 SSR | |
| mikan | 検索エンジン（旧 yuzu-index-format） | native / wasm で 1 実装共有 |
| kabosu | TOML パーサ（依存ゼロ・no_std + alloc） | **publish 前に fuzz を必ず一度回す** |

公開しない crate: **mikan-wasm**（wasm 成果物を作るビルド用。`publish = false`）と
yuzu 本体側の crate（名前 `yuzu`・`yuzu-core` が別プロジェクトに取得済み。
将来構想は ROADMAP.md）。

## 手順

```bash
# 1. 対象 crate の Cargo.toml の version を上げ、Cargo.lock を追随させる
cargo build
# 2. コミット → push → CI green を確認（ci.yml の cargo package --locked が
#    メタデータ・同梱内容の回帰を検出する。作業ツリーが dirty だと package は拒否される）
# 3. kabosu だけ: fuzz を一度回す（手動 workflow fuzz.yml、または手元で）
cd crates/kabosu && cargo +nightly fuzz run parse -- -max_total_time=60   # roundtrip / decode も
# 4. dry-run → publish（要 cargo login。公開は取り消し不可・yank のみ可能）
cargo publish --dry-run -p <crate>
cargo publish -p <crate>
```

## 罠

- **公開は取り消せない**（yank で新規取得を止められるだけ）。dry-run と CI green を飛ばさない
- kabosu は `[dependencies]` セクションを意図的に書かない = 依存ゼロ。CI が package 後の
  manifest を検査する（依存を足すと落ちる）
- yuzu 側の `Cargo.lock` はバンプの `cargo build` で追随する。yuzu のリリースタグとは
  無関係に進めてよい（非同期）
- 公開後は CLAUDE.md / README / docs（`development/kabosu.md` 等）の版表記を追随させる
