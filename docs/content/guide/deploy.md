---
title: 配信とデプロイ
order: 10
description: base_url・GitHub Pages・404 ページ・git 連携メタ・sitemap・共有カード
---

# 配信とデプロイ

`yuzu build` の出力（`dist/`）は純粋な静的サイトです。Web サーバや CDN に
そのまま置けます。

## サブパス配信（`base_url`）

サイトをサブパス（`https://example.com/docs/` や GitHub Pages の
`https://<user>.github.io/<リポジトリ名>/`）で配信する場合は、
リンク・アセット参照の解決先を `base_url` で指定します:

```toml
[site]
base_url = "/docs/"
```

CI から注入する場合はコマンドラインの `--base-url` が設定より優先されます:

```bash
yuzu build --base-url /docs/
yuzu build --base-url "https://example.com/docs/"  # フル URL も可
```

フル URL を渡すと llms.txt のリンクが絶対 URL になります。

## GitHub Pages

`yuzu new` が生成する `.github/workflows/deploy.yml` を push するだけで、
GitHub Pages への自動デプロイが動きます。必要な操作はリポジトリの
**Settings \> Pages \> Source を「GitHub Actions」にする**ことだけです。

GitHub Pages のサイトは、リポジトリを非公開にしてもインターネットに公開されます。
社内だけに見せる設計書は、[社内で公開する](internal.md)を読んでから置き場所を決めてください。

ワークフローは `actions/configure-pages` が返すホスト名と base path から公開先の
フル URL（`https://<ホスト名>/<リポジトリ名>/`）を組み立てて `yuzu build --base-url`
へ渡します。そのため project pages のサブパスも設定なしで正しく配信され、
[canonical と共有カード](#共有カードogpと-canonical)・[sitemap.xml](#sitemapxml) も
自動で出ます。

ワークフローが使う yuzu は、`yuzu new` を実行した yuzu と同じ版に固定されています。
その版のリリースから Linux x64 向けのアーカイブをダウンロードし、リリースに添付された
`SHA256SUMS` で照合してから使います（Rust のインストールやコンパイルは行いません）。
版を固定するのは、リリース前の変更がデプロイに混ざらないためです。新しい版に上げる
ときは、ワークフローの `YUZU_VERSION` を書き換えてください。ソースからビルドして
入れる手順も、ワークフローのコメントにあります。

ワークフローは `main` への push で動きます。既定のブランチが別の名前なら、
ワークフローの `branches` を書き換えてください。

> [!NOTE]
> このサイト自身も同じ仕組みで公開されています。リポジトリの
> [.github/workflows/docs.yml](https://github.com/ai-implementer/yuzu/blob/main/.github/workflows/docs.yml)
> が、リポジトリ内の最新の yuzu をビルドし、`yuzu check` を通してから
> `yuzu build --base-url` で `docs/dist` を生成して Pages へ配置します。

## 404 ページ

ビルド時に `404.html` を自動生成します（テーマ統合・検索ボックスと
サイドバー付き。GitHub Pages はこのファイルを自動で使います）。
`public/404.html` を置けばそちらが優先されます。`yuzu preview` / `yuzu dev`
も存在しないパスへ同じ 404 ページを 404 ステータスで返します。

## git 連携メタ

`git` セクションを有効にすると、ページフッターに最終コミット日と
「このページを編集」リンクが出ます（このサイトでも有効です。
ページ下部を見てください）:

```toml
[git]
last_updated = true # 最終コミット日（git が無い環境では自動で非表示）
edit_url = "https://github.com/me/docs/edit/main/content/{path}" # {path} は content 相対パス
```

git が無い環境・未コミットのページでは、日付を出さずに自動で縮退します。

> [!TIP]
> GitHub Actions で `last_updated` を使う場合は、checkout を
> `fetch-depth: 0` にしてください。浅いクローンでは全ページの最終コミット日が
> 直近のコミットに揃ってしまいます。

## sitemap.xml

`base_url` が**フル URL**（`https://…/`）のとき、全ページを列挙した
`sitemap.xml` を自動生成します（sitemap の `<loc>` は絶対 URL が仕様の
ため。パスだけの `base_url` では生成しません）。`git.last_updated` が有効なら
各ページに `<lastmod>` も付きます。リダイレクトページ（aliases）は
載りません。`public/sitemap.xml` を置けばそちらが優先されます。

このサイトも CI が `--base-url` にフル URL を渡しているため、
`/sitemap.xml` が自動生成されています。

## 共有カード（OGP）と canonical

全ページの `<head>` に、SNS やチャットでリンクを貼ったときのカード用のメタ
（[OGP](https://ogp.me/)）と `<meta name="twitter:card" content="summary">` が入ります。
`og:title` はページタイトル、`og:description` はページの `description`
（無ければ `site.description`）、`og:site_name` は `site.title` です。

`base_url` が**フル URL** のときは、sitemap と同じ条件で次も出ます:

- `<link rel="canonical">` と `og:url` — ページの絶対 URL
- `og:image` — `site.image` で指定した画像（`public/` 配下のパス）を絶対 URL にしたもの

画像は `site.image` で指定します:

```toml
[site]
image = "/images/og.png"   # 1200×630 前後の PNG / JPEG を public/images/ に置く
```

`site.image` にフル URL（`https://cdn.example.com/og.png`）を書けば、`base_url` が
パスだけでも `og:image` を出します。SVG はカードに使われないことが多いので、
ロゴとは別に PNG / JPEG を用意してください。`og:locale` は `site.lang` が
地域付き（`"ja-JP"`）のときだけ出ます（`"ja"` から地域は推測しません）。

canonical は相対 URL も規格上は許されていますが、yuzu は絶対 URL のときだけ出す方針です
（ホストが分からない状態の canonical は同一性の宣言として弱く、`og:url` とも揃わないため）。

## 静的ファイルの配信

`public/` 配下はそのまま `dist/` へコピーされます（画像・favicon・
`llms.txt` の手書き上書きなど）。ページ専用の画像は
[content 同伴アセット](writing.md#画像と添付ファイル)としても置けます。
