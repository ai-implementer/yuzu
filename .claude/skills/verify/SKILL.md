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
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo deny check bans licenses sources   # 要 cargo-deny（版は下記）。CI の deny ジョブと同じ
```

- machete の false positive は該当 crate の Cargo.toml に
  `[package.metadata.cargo-machete] ignored = ["<crate>"]` を書いて抑制する。
- CI の cargo は `--locked` 付き（Phase 82）。Cargo.lock の更新が要る変更は、手元で lock を
  更新してコミットに含める（CI では lock を書き換えられず失敗する）。
- cargo-deny は CI の `EmbarkStudios/cargo-deny-action` が使う版に合わせる
  （v2.1.1 → cargo-deny 0.20.2。`cargo install cargo-deny --version 0.20.2 --locked`）。
  設定は `deny.toml`。依存（Cargo.toml・Cargo.lock）を触ったら `cargo deny check advisories` と
  `scripts/vendor-advisories.sh` も流す（CI では deps.yml が PR で動く）。
  失敗したときの扱いは `dependency-update` スキル。

## 2. テスト

```bash
cargo test --locked --workspace --exclude yuzu-server
cargo test --locked -p yuzu-server   # ← サンドボックス外で実行する
```

- **yuzu-server はサンドボックス外必須**: serve テストが TCP バインドするため、サンドボックス内では PermissionDenied で落ちる（コード起因ではない）。
- **insta スナップショット差分が出たら**: 差分が意図どおりか必ず目視 → `INSTA_UPDATE=always cargo test -p <crate>` で更新 → `git diff` で更新内容を再確認。意図しない差分は変更側のバグを疑う。CI は `INSTA_UPDATE=no` で未承認を失敗にする。

## 3. ビルドと crates.io パッケージ検証

```bash
cargo build --locked --workspace
cargo package --locked -p tankan -p mikan -p kabosu
scripts/third-party-licenses.sh check
```

- `cargo package` は公開対象 3 crate のメタデータ・同梱内容の回帰を検出する（CI にもある）。
  CI は加えて `cargo package --list` で 3 crate とも `LICENSE-MIT` / `LICENSE-APACHE` を含み、
  tankan に `tests/corpus` が入らないことを見る。
- `third-party-licenses.sh check` は two-face の一覧の版（Phase 81）に加えて、使ってよい
  ライセンスの一覧が `licenses/about.toml` と `deny.toml` で同じか、検索 wasm の通知に載った
  crate と版の集合が mikan-wasm の wasm32 向け依存（`cargo tree`）と一致するか
  （= wasm が今の依存で作られているか）を見る（Phase 82）。
  kabosu は加えて package 後 manifest の依存ゼロ検査（CI）と
  `cargo check -p kabosu --target thumbv7em-none-eabi`（no_std 担保）がある。
  **作業ツリーが dirty だと拒否される**ので、コミット後に走らせるか意図を確認して `--allow-dirty`。

## 4. wasm32 チェック

```bash
cargo check --locked -p mikan-wasm --target wasm32-unknown-unknown
cargo check --locked -p tankan --target wasm32-unknown-unknown
```

## 5. docs サイト検証（このリポジトリ自身のドキュメントサイト）

**`docs/` の原稿・テーマ・記法まわりを触ったら必須。** CI（ci.yml の docs ステップ）と同じ内容。

リポジトリルートから `--root docs` で実行する（`cd docs` しない。Phase 75 の dogfooding。
出力は `docs/dist/`）。

```bash
cargo build -p yuzu-cli
<repo>/target/debug/yuzu check --root docs      # fmt 崩れ・壊れリンク・include エラーをまとめて検出
<repo>/target/debug/yuzu build --root docs
test -f docs/dist/index.html && test -f docs/dist/_search/manifest.json
# 機能ごとの配信ゲート（CI と同じ。新機能を足したら 1 行増やす）
grep -q 'http-equiv="refresh"' docs/dist/guide/lint/index.html                      # エイリアス
grep -q '<figcaption>yuzu.toml:25-45</figcaption>' docs/dist/guide/code-and-math/index.html   # インクルード
grep -q 'id="fig:deps"' docs/dist/development/index.html                            # 図表番号
grep -q '<a href="#fig:deps">図 1</a>' docs/dist/development/index.html             # 参照の自動補完
grep -q '<details class="markdown-alert markdown-alert-tip">' docs/dist/guide/writing/index.html  # 折りたたみ
grep -q 'js/details-target.js' docs/dist/guide/writing/index.html                   # 折りたたみ自動展開 JS
grep -q 'yuzu-sidebar-scroll' docs/dist/index.html                                  # サイドバー位置維持
grep -q 'class="tab-label"' docs/dist/guide/code-and-math/index.html                # タブ / コードグループ
grep -q '取り込まれた Markdown 断片です' docs/dist/guide/writing/index.html          # Markdown 断片
grep -q '<abbr title="Server-Side Rendering' docs/dist/guide/writing/index.html     # 用語集（本文の abbr 化）
grep -q 'id="ssr"' docs/dist/glossary/index.html                                    # 用語集ページの自動生成
grep -q 'href="/glossary/"' docs/dist/index.html                                    # 生成ページが nav に載る
! grep -q '<abbr' docs/dist/glossary/index.html                                     # 用語集ページ自身は abbr 化しない
grep -q '<strong>「重要」</strong>' docs/dist/guide/writing/index.html              # 約物に隣接した強調
grep -q '<dl>' docs/dist/guide/writing/index.html                                   # 定義リスト
grep -q '"docGroups"' docs/dist/_search/manifest.json                               # 検索の絞り込み区分
grep -q 'パーセントエンコード' docs/dist/guide/writing/index.html                    # URL エンコード（Phase 64）
test -f docs/dist/guide/internal/index.html                                         # 社内で公開する（Phase 83）
grep -q 'page_md' docs/dist/guide/llms/index.html                                   # .md の配信を止める設定（Phase 83）
grep -q 'allowed_hosts' docs/dist/reference/config/index.html                       # Host の検査（Phase 83）
grep -q '追随する' docs/dist/guide/deploy/index.html                                # テーマ上書きの契約（Phase 65）
grep -q 'シンボリックリンクを辿りません' docs/dist/reference/cli/index.html          # 配信のリンク遮断（Phase 65）
grep -q -- '--root' docs/dist/reference/cli/index.html                              # グローバルフラグ（Phase 72）
grep -q -- '--root' docs/dist/reference/config/index.html                           # 上方向探索の但し書き（Phase 72）
grep -q -- '--quiet' docs/dist/reference/cli/index.html                             # 静粛モード -q / -v（Phase 73）
grep -q 'id="シェル補完"' docs/dist/reference/cli/index.html                          # シェル補完の節（Phase 74。コード内文字列は span で割れる）
grep -q '<meta property="og:title"' docs/dist/index.html                          # 共有カード（Phase 76。canonical はフル URL 時だけ）
grep -q 'id="共有カードogpと-canonical"' docs/dist/guide/deploy/index.html         # OGP の節（Phase 76）
grep -q 'frontmatter-unrecognized' docs/dist/reference/rules/index.html             # frontmatter の読み違い（Phase 77）
grep -q 'yuzu と同じ版に' docs/dist/guide/deploy/index.html                          # 雛形 deploy.yml の版固定（Phase 77）
grep -q '<h2 id="frontmatter">frontmatter<a class="anchor" href="#frontmatter" aria-label=' docs/dist/guide/writing/index.html  # 見出しのパーマリンク（Phase 78）
! grep -rl 'aria-hidden="true" class="anchor"' docs/dist/ --include="*.html"        # 旧形式のアンカーが残っていない（Phase 78）
grep -q '.anchor:focus-visible' docs/dist/_assets/css/theme.css                     # フォーカスで # を出す（Phase 78）
grep -q '<p class="page-reading">約 [0-9]* 分で読めます' docs/dist/guide/writing/index.html  # 読了時間（Phase 78）
grep -q '<html lang="ja">' docs/dist/index.html                                    # toggle では data-theme を付けない（Phase 79）
grep -q 'prefers-color-scheme: dark' docs/dist/_assets/css/theme.css               # OS ダーク追従（Phase 79）
grep -q 'prefers-color-scheme: dark' docs/dist/_assets/css/syntect.css             # コードの配色も OS 追従（Phase 79）
test -f docs/dist/images/og.png                                                     # 共有カードの画像（Phase 80。og:image タグはフル URL の公開ビルドだけ）
grep -q 'css/syntect.css' docs/dist/index.html && test -f docs/dist/_assets/css/syntect.css  # syntect.css は有効時だけ
<repo>/target/debug/yuzu search --root docs --section 開発 "キャッシュ" | grep -q '/development/'  # エンジン側の絞り込み
# SSR フォールバック検出: backend:ssr のサイトで mermaid.js が読まれたら tankan の回帰
grep -rlE 'src="[^"]*vendor/mermaid\.min\.js"' docs/dist/ --include="*.html" && echo "NG: フォールバック発生"
```

**`docs/yuzu.toml` の 25-45 行目**（`[markdown]` から `[markdown.glossary.terms]` の末尾まで）は
インクルードの `lines=` で引用されている。この範囲を動かすと原稿の中身とゲートが同時に壊れるので、
`docs/content/guide/code-and-math.md` の `lines=` 3 箇所と ci.yml の grep を同時に直す。

## 6. e2e（CLI 実機）

**罠: `cargo test --workspace` は `target/debug/yuzu` を更新しない。必ず先にビルドする。**

```bash
cargo build -p yuzu-cli
./target/debug/yuzu new "<scratchpad>/e2e-docs"
# --root（Phase 72）は cwd がプロジェクト外の状態で見る = cd より前に置く。
# 前後どちらでも効く / 上方向探索をしない / ルート自身がリンクでも通る / new は拒否
./target/debug/yuzu --root "<scratchpad>/e2e-docs" build
./target/debug/yuzu build --root "<scratchpad>/e2e-docs"
mkdir -p "<scratchpad>/root-empty"
./target/debug/yuzu check --root "<scratchpad>/root-empty"   # exit 2・文言に「上方向」を含まない
mkdir -p "<scratchpad>/e2e-docs/sub" && ./target/debug/yuzu check --root "<scratchpad>/e2e-docs/sub"  # exit 2
ln -s "<scratchpad>/e2e-docs" "<scratchpad>/root-link" && ./target/debug/yuzu build --root "<scratchpad>/root-link"
./target/debug/yuzu new --root "<scratchpad>/e2e-docs" "<scratchpad>/root-new"   # exit 2
cd "<scratchpad>/e2e-docs"
test -f .github/workflows/deploy.yml   # Pages デプロイ雛形の同梱
# 雛形 deploy.yml は yuzu new した版のタグでインストールする（Phase 77）
grep -q -- "--tag v$(<repo>/target/debug/yuzu --version | awk '{print $2}') yuzu-cli" .github/workflows/deploy.yml && echo "OK deploy tag"
<repo>/target/debug/yuzu build
test -f dist/index.html && test -f dist/_search/manifest.json && test -f dist/_search/search_bg.wasm
# 第三者ライセンスの通知を資産の隣に配り、vendor の開発用メモは配らない（Phase 81）
test -s dist/_assets/vendor/THIRD-PARTY-LICENSES.txt && test -s dist/_search/THIRD-PARTY-LICENSES.txt
test ! -e dist/_assets/vendor/README.md
# 原稿の .md の配信（Phase 83）: 既定は配信、llms.page_md = false で全ページ止める
# （残っていた .md は孤児掃除で消え、data-md-url も消え、llms.txt は HTML を指す）。
# frontmatter の pageMd: false はそのページだけ。リダイレクトは絶対パスで書く（相対はフックが止める）
test -f dist/index.md && grep -q 'data-md-url' dist/index.html
printf '\n[llms]\npage_md = false\n' >> "<scratchpad>/e2e-docs/yuzu.toml" && <repo>/target/debug/yuzu build
find dist -name '*.md' | grep -q . && echo "NG: .md が残った"
grep -q 'data-md-url' dist/index.html && echo "NG: data-md-url"
grep -q '(/guide/getting-started/)' dist/llms.txt && echo "OK llms は HTML"
sed -i 's/^page_md = false$/page_md = true/' yuzu.toml
sed -i '0,/^order: 1$/s//order: 1\npageMd: false/' content/guide/getting-started.md && <repo>/target/debug/yuzu build
test -f dist/index.md && test ! -e dist/guide/getting-started.md && echo "OK pageMd"
sed -i '/^pageMd: false$/d' content/guide/getting-started.md && <repo>/target/debug/yuzu build
<repo>/target/debug/yuzu search "はじめに" | grep "はじめに"
# タイポトレランス（出力の有無だけでなくヒット内容まで見る）とフレーズ検索の正/逆順
<repo>/target/debug/yuzu search "ダーくモード" | grep -q "ダークモード"
<repo>/target/debug/yuzu search '"ライブリロード"' | grep -q "ライブリロード"
<repo>/target/debug/yuzu search '"リロードライブ"' | grep -q "一致するページはありませんでした"
# -q / -v / search --format（Phase 73）: -q は RUST_LOG より優先して info 以下を黙らせる
RUST_LOG=debug <repo>/target/debug/yuzu build --force -q 2>&1 | wc -l        # 0
# grep -q は最初の一致で読み手を閉じる = stderr の EPIPE でビルドが落ちない検査も兼ねる
<repo>/target/debug/yuzu build --force -v 2>&1 | grep -q DEBUG && echo "OK verbose"
test "${PIPESTATUS[0]}" -eq 0 && test -f dist/_search/manifest.json && echo "OK 完走"
<repo>/target/debug/yuzu build -q -v                                          # exit 2（同時指定は矛盾）
<repo>/target/debug/yuzu -q build -v                                          # exit 2（前後に分けても弾く）
<repo>/target/debug/yuzu search --format json "はじめに" | head -1 | grep -q '^\[$' && echo "OK search json"
<repo>/target/debug/yuzu search --json "はじめに" | head -1 | grep -q '^\[$' && echo "OK 旧表記"
<repo>/target/debug/yuzu search --json --format json "はじめに"              # exit 2（旧表記との同時指定）
# シェル補完（Phase 74）: 全シェルで生成でき、グローバル引数が候補に入る。bash は構文検査まで
for sh in bash zsh fish powershell elvish; do <repo>/target/debug/yuzu completions $sh | grep -q quiet || echo "NG $sh"; done
<repo>/target/debug/yuzu completions bash > "<scratchpad>/yuzu.bash" && bash -n "<scratchpad>/yuzu.bash" && echo "OK bash -n"
<repo>/target/debug/yuzu completions bash --root .                            # exit 2（プロジェクトを読まない）
# --base-url は設定より優先（deploy.yml が configure-pages の base_path を渡す契約）
<repo>/target/debug/yuzu build --base-url /docs/ && grep -q '/docs/_assets/' dist/index.html
# 共有カード（Phase 76）: パスだけの base では og:image が出ず、フル URL なら canonical / og:url / og:image が絶対 URL で出る
sed -i 's|^title = "My Docs"$|title = "My Docs"\nimage = "/images/yuzu-logo.svg"|' yuzu.toml
<repo>/target/debug/yuzu build --base-url /docs/ && grep -q 'og:image' dist/index.html && echo "NG: パス base で og:image"
<repo>/target/debug/yuzu build --base-url https://example.com/docs/
grep -q 'rel="canonical" href="https://example.com/docs/guide/getting-started/"' dist/guide/getting-started/index.html && echo "OK canonical"
grep -q 'og:image" content="https://example.com/docs/images/yuzu-logo.svg"' dist/index.html && echo "OK og:image"
<repo>/target/debug/yuzu build   # 後続の検査は既定 base_url に戻してから
# 監視（Phase 77）: 何も編集せず build --watch を数秒動かして再ビルド 0 回（Linux の inotify は
# 開いただけのイベントも届ける。macOS では起きないので開発コンテナで見る）
timeout 6 <repo>/target/debug/yuzu build --watch --port 5199 2>&1 | grep -c '変更を検知'   # 0
# frontmatter の閉じ忘れは check で error（exit 1）・区切り線で始まる文書は誤検出しない（Phase 77）
printf -- '---\ntitle: 閉じ忘れ\n\n本文\n' > content/unclosed.md
printf -- '---\n\n# 区切り線で始まる\n\n本文\n' > content/hr-first.md
<repo>/target/debug/yuzu check --format json 2>/dev/null | grep -B2 '"path": "content/' | grep -c frontmatter-unrecognized   # 1（unclosed.md だけ）
rm content/unclosed.md content/hr-first.md
<repo>/target/debug/yuzu fmt --check && <repo>/target/debug/yuzu lint && <repo>/target/debug/yuzu check
# 異常系: 壊れリンクを注入して check が終了コード 1 を返すこと（CI と同じ）
echo '[壊れリンク](missing.md)' >> content/index.md
<repo>/target/debug/yuzu check && echo "NG: 検出漏れ" || echo "OK"

# 機械可読出力（診断が出ている状態のまま検証する）
# json は単一オブジェクトで、標準出力に他の行を混ぜない
<repo>/target/debug/yuzu check --format json | head -1 | grep -q '^{$' && echo "OK json"
# github は注釈行を出す。GITHUB_WORKSPACE を差し替えると相対パスが付け替わる
# （これが崩れると PR に注釈が紐づかない。--root 指定で cwd がプロジェクト外でも同じ =
#   docs.yml / docs-links.yml がリポジトリルートから --root docs で実行する前提）
<repo>/target/debug/yuzu check --format github | grep '^::error file='
GITHUB_WORKSPACE="$(dirname "$PWD")" <repo>/target/debug/yuzu check --format github | grep '^::error file=e2e-docs/'
# cwd をプロジェクト外へ出してから --root（相対）で指定する（サブシェルなので後続に影響しない）
( cd .. && GITHUB_WORKSPACE="$PWD" <repo>/target/debug/yuzu check --root e2e-docs --format github | grep '^::error file=e2e-docs/' )
# lint --fix と併用しても標準出力は JSON のまま（進捗は stderr へ逃げる）
<repo>/target/debug/yuzu lint --fix --format json 2>/dev/null | head -1 | grep -q '^{$' && echo "OK fix+json"
# preview のリンク遮断（Phase 65）: dist にリンクを置いて 404 と内容非漏洩を見る
mkdir -p /tmp/outside && echo '<html>secret</html>' > /tmp/outside/secret.html
ln -s /tmp/outside dist/link && ln -s /tmp/outside/secret.html dist/leaf.html
<repo>/target/debug/yuzu preview --port 48124 >/dev/null 2>&1 & GUARD_PID=$!
sleep 1
test "$(curl -s -o /dev/null -w '%{http_code}' http://127.0.0.1:48124/link/secret.html)" = 404 && echo "OK link 404"
! curl -s http://127.0.0.1:48124/leaf.html | grep -q secret && echo "OK 非漏洩"
# Host の検査（Phase 83。DNS リバインディング対策）: ホスト名は 403、localhost は 200
test "$(curl -s -o /dev/null -w '%{http_code}' -H 'Host: evil.example' http://127.0.0.1:48124/)" = 403 && echo "OK host 403"
test "$(curl -s -o /dev/null -w '%{http_code}' -H 'Host: localhost:48124' http://127.0.0.1:48124/)" = 200 && echo "OK localhost"
kill $GUARD_PID; rm dist/link dist/leaf.html
# 外部リンク検査（opt-in・Phase 66）: ネットワークへは出ず、自分の preview を相手にする
<repo>/target/debug/yuzu preview --port 48123 >/dev/null 2>&1 & PREVIEW_PID=$!
sleep 1
printf '\n[ok](http://127.0.0.1:48123/)\n\n[missing](http://127.0.0.1:48123/no-such-file)\n\n[down](http://127.0.0.1:1/)\n' >> content/index.md
<repo>/target/debug/yuzu check --external-links --format json > /tmp/extlink.json; kill $PREVIEW_PID
grep -q '"rule": "external-link-broken"' /tmp/extlink.json && grep -q '"skipped": 1' /tmp/extlink.json && echo "OK extlink"
<repo>/target/debug/yuzu check --format json | grep -q '"skipped": 0' && echo "OK 既定は触れない"
```

終了コード規約: 0 = 成功 / 1 = 違反あり / 2 = 実行エラー。

検索・OpenAPI・アセット周りを触ったときは CI の e2e にある以下も再現する:
**OpenAPI の `file:` 参照が仕様ファイルの変更だけで再ビルドに反映されること**、
**content 同伴アセット（ページ横の画像）が dist へコピーされ `src` が絶対 URL へ解決されること**。

## 7. UI・テーマ・scaffold の変更がある場合

`run` スキル（プロジェクト版）でブラウザ配信まで実機確認する。
