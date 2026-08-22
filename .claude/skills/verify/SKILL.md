---
name: verify
description: yuzu の変更を検証する。CI 相当（fmt / clippy / test / package / wasm check / docs サイト検証）＋ CLI 実機 e2e を、既知の罠を回避した正しい順序で実行する。コード変更後の検証・コミット前チェックで使う。
---

# yuzu 検証手順

CI（.github/workflows/ci.yml）と同等＋実機 e2e。上から順に実行する。

## 1. 静的チェック

```bash
cargo fmt --all --check
cargo machete   # 未使用依存の検出（要 cargo install cargo-machete。CI にもある）
cargo clippy --workspace --all-targets -- -D warnings
```

- machete の false positive は該当 crate の Cargo.toml に
  `[package.metadata.cargo-machete] ignored = ["<crate>"]` を書いて抑制する。

## 2. テスト

```bash
cargo test --workspace --exclude yuzu-server
cargo test -p yuzu-server   # ← サンドボックス外で実行する
```

- **yuzu-server はサンドボックス外必須**: serve テストが TCP バインドするため、サンドボックス内では PermissionDenied で落ちる（コード起因ではない）。
- **insta スナップショット差分が出たら**: 差分が意図どおりか必ず目視 → `INSTA_UPDATE=always cargo test -p <crate>` で更新 → `git diff` で更新内容を再確認。意図しない差分は変更側のバグを疑う。CI は `INSTA_UPDATE=no` で未承認を失敗にする。

## 3. ビルドと crates.io パッケージ検証

```bash
cargo build --workspace
cargo package --locked -p tankan -p mikan -p kabosu
```

- `cargo package` は公開対象 3 crate のメタデータ・同梱内容の回帰を検出する（CI にもある）。
  kabosu は加えて package 後 manifest の依存ゼロ検査（CI）と
  `cargo check -p kabosu --target thumbv7em-none-eabi`（no_std 担保）がある。
  **作業ツリーが dirty だと拒否される**ので、コミット後に走らせるか意図を確認して `--allow-dirty`。

## 4. wasm32 チェック

```bash
cargo check -p mikan-wasm --target wasm32-unknown-unknown
cargo check -p tankan --target wasm32-unknown-unknown
```

## 5. docs サイト検証（このリポジトリ自身のドキュメントサイト）

**`docs/` の原稿・テーマ・記法まわりを触ったら必須。** CI（ci.yml の docs ステップ）と同じ内容。

```bash
cargo build -p yuzu-cli
cd docs
<repo>/target/debug/yuzu check      # fmt 崩れ・壊れリンク・include エラーをまとめて検出
<repo>/target/debug/yuzu build
test -f dist/index.html && test -f dist/_search/manifest.json
# 機能ごとの配信ゲート（CI と同じ。新機能を足したら 1 行増やす）
grep -q 'http-equiv="refresh"' dist/guide/lint/index.html                      # エイリアス
grep -q '<figcaption>yuzu.toml:25-45</figcaption>' dist/guide/code-and-math/index.html   # インクルード
grep -q 'id="fig:deps"' dist/development/index.html                            # 図表番号
grep -q '<a href="#fig:deps">図 1</a>' dist/development/index.html             # 参照の自動補完
grep -q '<details class="markdown-alert markdown-alert-tip">' dist/guide/writing/index.html  # 折りたたみ
grep -q 'js/details-target.js' dist/guide/writing/index.html                   # 折りたたみ自動展開 JS
grep -q 'yuzu-sidebar-scroll' dist/index.html                                  # サイドバー位置維持
grep -q 'class="tab-label"' dist/guide/code-and-math/index.html                # タブ / コードグループ
grep -q '取り込まれた Markdown 断片です' dist/guide/writing/index.html          # Markdown 断片
grep -q '<abbr title="Server-Side Rendering' dist/guide/writing/index.html     # 用語集（本文の abbr 化）
grep -q 'id="ssr"' dist/glossary/index.html                                    # 用語集ページの自動生成
grep -q 'href="/glossary/"' dist/index.html                                    # 生成ページが nav に載る
! grep -q '<abbr' dist/glossary/index.html                                     # 用語集ページ自身は abbr 化しない
grep -q '<strong>「重要」</strong>' dist/guide/writing/index.html              # 約物に隣接した強調
grep -q '<dl>' dist/guide/writing/index.html                                   # 定義リスト
grep -q '"docGroups"' dist/_search/manifest.json                               # 検索の絞り込み区分
<repo>/target/debug/yuzu search --section 開発 "キャッシュ" | grep -q '/development/'  # エンジン側の絞り込み
# SSR フォールバック検出: backend:ssr のサイトで mermaid.js が読まれたら tankan の回帰
grep -rlE 'src="[^"]*vendor/mermaid\.min\.js"' dist/ --include="*.html" && echo "NG: フォールバック発生"
```

**`docs/yuzu.toml` の 25-45 行目**（`[markdown]` から `[markdown.glossary.terms]` の末尾まで）は
インクルードの `lines=` で引用されている。この範囲を動かすと原稿の中身とゲートが同時に壊れるので、
`docs/content/guide/code-and-math.md` の `lines=` 3 箇所と ci.yml の grep を同時に直す。

## 6. e2e（CLI 実機）

**罠: `cargo test --workspace` は `target/debug/yuzu` を更新しない。必ず先にビルドする。**

```bash
cargo build -p yuzu-cli
./target/debug/yuzu new "<scratchpad>/e2e-docs"
cd "<scratchpad>/e2e-docs"
test -f .github/workflows/deploy.yml   # Pages デプロイ雛形の同梱
<repo>/target/debug/yuzu build
test -f dist/index.html && test -f dist/_search/manifest.json && test -f dist/_search/search_bg.wasm
<repo>/target/debug/yuzu search "はじめに" | grep "はじめに"
# タイポトレランス（出力の有無だけでなくヒット内容まで見る）とフレーズ検索の正/逆順
<repo>/target/debug/yuzu search "ダーくモード" | grep -q "ダークモード"
<repo>/target/debug/yuzu search '"ライブリロード"' | grep -q "ライブリロード"
<repo>/target/debug/yuzu search '"リロードライブ"' | grep -q "一致するページはありませんでした"
# --base-url は設定より優先（deploy.yml が configure-pages の base_path を渡す契約）
<repo>/target/debug/yuzu build --base-url /docs/ && grep -q '/docs/_assets/' dist/index.html
<repo>/target/debug/yuzu build   # 後続の検査は既定 base_url に戻してから
<repo>/target/debug/yuzu fmt --check && <repo>/target/debug/yuzu lint && <repo>/target/debug/yuzu check
# 異常系: 壊れリンクを注入して check が終了コード 1 を返すこと（CI と同じ）
echo '[壊れリンク](missing.md)' >> content/index.md
<repo>/target/debug/yuzu check && echo "NG: 検出漏れ" || echo "OK"

# 機械可読出力（診断が出ている状態のまま検証する）
# json は単一オブジェクトで、標準出力に他の行を混ぜない
<repo>/target/debug/yuzu check --format json | head -1 | grep -q '^{$' && echo "OK json"
# github は注釈行を出す。GITHUB_WORKSPACE を差し替えると相対パスが付け替わる
# （CI が cd docs している状況の再現。これが崩れると PR に注釈が紐づかない）
<repo>/target/debug/yuzu check --format github | grep '^::error file='
GITHUB_WORKSPACE="$(dirname "$PWD")" <repo>/target/debug/yuzu check --format github | grep '^::error file='
# lint --fix と併用しても標準出力は JSON のまま（進捗は stderr へ逃げる）
<repo>/target/debug/yuzu lint --fix --format json 2>/dev/null | head -1 | grep -q '^{$' && echo "OK fix+json"
```

終了コード規約: 0 = 成功 / 1 = 違反あり / 2 = 実行エラー。

検索・OpenAPI・アセット周りを触ったときは CI の e2e にある以下も再現する:
**OpenAPI の `file:` 参照が仕様ファイルの変更だけで再ビルドに反映されること**、
**content 同伴アセット（ページ横の画像）が dist へコピーされ `src` が絶対 URL へ解決されること**。

## 7. UI・テーマ・scaffold の変更がある場合

`run` スキル（プロジェクト版）でブラウザ配信まで実機確認する。
