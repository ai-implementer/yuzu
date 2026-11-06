---
title: 他のツールとの違い
order: 2
description: ほかのドキュメントツールから移行するときの、書き慣れた書き方と yuzu での書き方の対応
---

# 他のツールとの違い

ほかのドキュメントツールで書いた原稿を yuzu へ移行するときに、書き方が違う所をまとめます。
yuzu はナビを設定ファイルに書かず、ファイルの置き場所と frontmatter からサイトを組み立てます。
yuzu が知らない書き方の多くはエラーにならずに文字のまま表示されるので、移したあとは
このページの表と見比べてください。

[既存のフォルダで始める](install.md#既存のフォルダで始める)手順で `yuzu.toml` を置いた後に
読むことを想定しています。

## ナビと URL

- ナビ（サイドバー）は**ディレクトリ階層から自動で作るだけ**です。設定ファイルにページを
  並べる方法や、ページをナビから外す設定はありません。公開しないページは `draft: true` に
  するか、`input.ignore` で外します
- 並び順は frontmatter の `order`（昇順）です。ディレクトリの表示名と並び順は、その配下の
  `index.md` から取ります（[ページとナビゲーション](writing.md#ページとナビゲーション)）
- ページのタイトルは frontmatter の `title`、無ければ本文の最初の h1、それも無ければ
  ファイル名です。ナビにだけ短い名前を出す設定はありません
- **トップページは原稿のディレクトリ直下の `index.md`** です。`README.md` は `/README/` の
  ページになります（トップページが無いと `yuzu build` が警告します）
- **ファイル名がそのまま URL** になります。小文字化や記号の置き換えはせず、frontmatter で
  URL を変える設定もありません。URL を変えるときはファイル名を変え、古い URL は `aliases` で
  新しいページへ転送します（[ページの移動とリダイレクト](writing.md#ページの移動とリダイレクトaliases)）
- 大文字小文字だけが違う URL（`Guide.md` と `guide/`）は、`yuzu check` が
  `route-case-conflict` で知らせます

## frontmatter と設定ファイル

- 設定ファイルは `yuzu.toml`（TOML）だけです。YAML・JSON・JavaScript の設定ファイルは読みません
- `yuzu.toml` のキーは `base_url` のような snake\_case、frontmatter のキーは `lintDisable` の
  ような camelCase です
- `yuzu.toml` に知らないキーがあると設定エラーになり、ビルドが止まります（終了コード 2）。
  frontmatter の知らないキーは無視され、`yuzu check` が warning `frontmatter-unknown-key` で
  知らせます

frontmatter で書き慣れたキーと、yuzu のキーの対応です（キーの一覧は
[frontmatter](writing.md#frontmatter)）:

| 書き慣れたキー | yuzu |
| --- | --- |
| `sidebar_position`・`weight`・`nav_order` | `order` |
| `slug`・`permalink` | 無し（ファイル名を変える。古い URL は `aliases`） |
| `sidebar_label`・`nav_title` | 無し（`title` を使う） |
| `published: false` | `draft: true` |
| ナビから外すキー（`nav_exclude` など） | 無し |
| `tags`・`categories` | 無し |
| 検索から外すキー | `search: false` |

## 記法

| 書き慣れた書き方 | yuzu での書き方 |
| --- | --- |
| `::: tip` 〜 `:::` の囲み | `> [!TIP]`（5 種類。[Admonition](writing.md#admonition)） |
| `??? note` などの折りたたみ | `> [!NOTE]-`（[折りたたみ](writing.md#折りたたみ)） |
| `=== "名前"` などのタブ | フェンスの `tab="名前"`（[タブ / コードグループ](code-and-math.md#タブ--コードグループ)） |
| 見出しの後ろの `{#id}` | 無し（id は見出しの文字から作ります。日本語はそのまま） |
| 別のファイルの取り込み | Markdown は ` ```include file="…" `（[Markdown 断片のインクルード](writing.md#markdown-断片のインクルード)）、ソースコードはフェンスの `file=`（[ソースファイルの埋め込み](code-and-math.md#ソースファイルの埋め込み)） |
| `{{ 変数 }}` などのテンプレートのタグ | 無し |
| 絵文字の短縮記法（`:smile:`） | 無し（絵文字をそのまま書く） |
| `[[ページ名]]` のリンク | `[ページ名](ページ名.md)` |
| MDX のコンポーネント | 無し |

この表で「無し」の書き方は、エラーにも警告にもならずに書いたまま出力されます。多くは文字の
まま表示され、MDX のコンポーネント（`<Tabs>` など）は知らない HTML の要素として中身だけが
表示されます。移したあとは `yuzu preview` でページを開いて確かめてください。

原稿のディレクトリにある `.md` 以外のファイルは、同伴ファイルとして `dist/` へそのまま
コピーされます。`.mdx` もページにならずにそのまま公開されるので、`.md` に直すか、
`input.ignore` で外します:

```toml
[input]
ignore = ["**/*.mdx"]
```

## リンク

- ページへのリンクは、**原稿のファイルへの相対パスに `.md` を付けて**書きます
  （`[設定](../reference/config.md)`）。yuzu がページの URL に書き換え、`yuzu check` が
  リンク切れを調べます
- `.md` を付けない相対リンク（`[設定](../reference/config/)`）は書き換えも検査もしません。
  ページは `/guide/writing/` のようなディレクトリの URL で配信されるので、原稿の場所から
  数えた相対パスでは 1 階層ずれることがあります
- サイトの中を絶対パスで指すときは URL で書きます（`/guide/writing/`）。`base_url` が前に
  付き、`yuzu check` も調べます。`/guide/writing.md` のように `.md` を付けた絶対パスは
  ページに解決されず、`yuzu check` がリンク切れにします
- `content/` の外（`../src/main.rs`・リポジトリの `README.md`）を指す相対リンクは、外の
  ファイルがサイトに出ないので書いたまま残り、`yuzu check` が warning
  `link-outside-content` で知らせます

## 生 HTML と画像の大きさ

本文には HTML をそのまま書けます（`<details>`・`<kbd>`・`<img>` など）。ただし、
**HTML の中の URL は書き換えも検査もしません**。`base_url` が前に付かず、`.md` もページの
URL に解決されないので、サブパスで配信するとリンクや画像が切れます。リンクと画像は、
できるだけ Markdown の記法で書いてください。

Markdown の画像の記法には大きさを指定する書き方がありません。表示する大きさで画像を
用意するか、`<img width="…">` を使います。`<img>` の `src` は書いたまま出力されるので、
ページの URL から数えた相対パスで書きます。`guide/writing.md` の横にある `diagram.png` なら、
ページの URL は `/guide/writing/` なので `src="../diagram.png"` です（`index.md` のページでは
`src="diagram.png"`）。この参照は `yuzu check` が調べないので、ページを開いて確かめてください。

## サイトの見た目

| 書き慣れたもの | yuzu |
| --- | --- |
| ロゴ・サイト名 | `site.logo`・`site.title` |
| タブのアイコン（favicon） | `site.favicon`（書かなくても `public/favicon.ico` があれば使います） |
| フッターの文言 | `site.footer`（インラインの Markdown） |
| ヘッダーのリンク | `[[site.header_links]]`（`text` と `url`） |
| 静的ファイルの置き場所（`static/` など） | `public/`（`dist/` の直下へそのままコピー） |
| テンプレート・CSS の上書き | `theme/` に同じ相対パスのファイルを置く |

設定の書き方は[見た目の調整](appearance.md)と[設定リファレンス](../reference/config.md)を
参照してください。

## 移したあとに確かめること

1. `yuzu check --no-fmt` で、リンク切れ・frontmatter の知らないキー・`content/` の外を
   指すリンクを確かめます（整形差分の検査は後回しにできます）
2. `yuzu build` の警告と最後の行（出したページの数）を確かめます（[品質チェック](quality.md#yuzu-check--統合チェック)）
3. `yuzu preview` でページを開き、記法の表で「無し」の書き方がそのまま出ていないかを確かめます
4. 原稿を整形してよくなったら `yuzu fmt` をかけ、CI の `--no-fmt` を外します

GitHub Pages へ公開するワークフローと、プルリクエストを検査するワークフローの全文は
[配信とデプロイ](deploy.md#ワークフローの全文)と[品質チェック](quality.md#ci-に組み込む)にあります。
