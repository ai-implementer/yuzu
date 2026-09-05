---
title: 配信とデプロイ
order: 8
description: base_url・GitHub Pages・404 ページ・テーマ上書き・印刷 / PDF・git 連携メタ
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

ワークフローは `actions/configure-pages` が返す base path を
`yuzu build --base-url` へ渡すため、project pages のサブパス
（`/<リポジトリ名>/`）も設定なしで正しく配信されます。

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

## テーマのカスタマイズ

デフォルトテーマはバイナリに埋め込まれており、プロジェクトの `theme/` に
**同じ相対パスのファイルを置くだけ**でファイル単位に上書きできます
（テンプレート・CSS・JS のどれでも）。

> [!IMPORTANT]
> 上書きしたファイルは、yuzu を更新したときに**デフォルトテーマ側の変更へ追随する
> 責任が利用者側にあります**。テンプレートに渡す変数・生成するアセット・JS が
> 期待する DOM は、版ごとに変わることがあります（例: `syntect.css` は
> `markdown.highlight.enabled` が有効なときだけ生成され、`base.jinja` も
> `highlight_enabled` で `<link>` を出し分けます。古い `base.jinja` を
> 上書きしたまま無効化すると、存在しないファイルへの `<link>` が残ります）。
> 上書きは必要なファイルだけに絞り、更新後は差分を見直してください。

色だけ変えたい場合は、設定の CSS 変数上書きが手軽です:

```toml
[theme.css_vars]
accent = "#0a6cff"

[theme.css_vars_dark] # ダークモード時のみの上書き
accent = "#7fb2ff"
```

## 印刷と PDF

ブラウザの印刷（「PDF に保存」）が、そのまま配布できる紙面になるよう
デフォルトテーマが調整してあります:

- サイドバー・目次・検索・コピーボタン等の画面用 UI は紙に載りません
  （パンくずと最終更新日は残ります）
- ダークモードで表示していても、印刷は常にライト配色です
  （本文・コードハイライト・SSR の図とも）
- 閉じた折りたたみとタブは全展開されます（タブはラベルを小見出しにして縦に並びます）
- 表はページをまたぐと見出し行が再掲され、コードは横に切れず折り返します
- 外部リンクは URL が括弧書きで併記されます

テーマを `theme/static/css/theme.css` で丸ごと上書きしている場合は、
この印刷対応も上書き側の内容に従います。

> [!NOTE]
> `markdown.mermaid.backend: "client"` のサイトをダークモードのまま印刷すると、
> 図だけダーク配色で紙に載ります（mermaid.js が色を SVG へ焼き込むため）。
> 印刷前にライトへ切り替えるか、既定の `"ssr"` を使ってください
> （SSR の図はテーマ変数参照なので自動でライトになります）。

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

## 静的ファイルの配信

`public/` 配下はそのまま `dist/` へコピーされます（画像・favicon・
`llms.txt` の手書き上書きなど）。ページ専用の画像は
[content 同伴アセット](writing.md#画像と添付ファイル)としても置けます。
