---
title: LLM 連携
order: 6
description: llms.txt / llms-full.txt の自動生成とページ単位 Markdown の配信
---

# LLM 連携

設計書を LLM に読ませる用途を最初から想定しています。ビルドのたびに
次の成果物が自動生成されます。

## llms.txt / llms-full.txt

[llms.txt 仕様](https://llmstxt.org/)に準拠した索引と全文を `dist/` 直下へ
出力します:

- [`/llms.txt`](/llms.txt) — サイトの説明と全ページへのリンク索引
  （リンク先は各ページの `.md`。`.md` を配信しないページはページの HTML）
- [`/llms-full.txt`](/llms-full.txt) — 全ページの正規化 Markdown を
  連結した 1 ファイル

llms-full.txt の本文は原文そのままではなく、`yuzu fmt` と同じ基盤による
**正規形の Markdown**（見出しの ATX 化・箇条書きの `-` 統一など）です。

- frontmatter に `llms: false` を書いたページは両方から除外されます
- `public/llms.txt` を手書きで置くと生成版を上書きできます
- `yuzu llms` / `yuzu llms --full` で、`dist/` を作らずに標準出力へも出せます

> [!TIP]
> 公開サイトでは `build.base_url` にフル URL（`https://…/docs/` など）を
> 設定すると、llms.txt のリンクが絶対 URL になります（llms.txt の慣行に
> 合います）。このサイトは CI が `--base-url` でフル URL を注入しています。

## ページ単位の Markdown 配信

各ページの原文 Markdown（frontmatter 込み・バイトそのまま）が
`dist/<ルート>.md` として配信されます。たとえばこのページの原文は
`/guide/llms.md` で開けます。

ページ右上の「**Markdown をコピー**」ボタンで、その原文をそのまま
クリップボードへコピーできます（LLM に貼る用途。`.md` を開くリンク付き。
JS 無効時はボタンが現れないプログレッシブエンハンスメントです）。

### 原稿の `.md` を配信しない

原文はバイトそのままなので、frontmatter や HTML コメントに書いたメモも読者に見えます。
配信したくないときは止められます（既定は配信する）。

```toml
[llms]
page_md = false # 全ページで .md を配信しない
```

ページ単位なら frontmatter に `pageMd: false` を書きます。止めたページでは

- `dist/<ルート>.md` を出しません（前回の build で出したものは次の build で消えます）
- 「Markdown をコピー」ボタンを出しません
- llms.txt のリンクはページの HTML を指します

llms-full.txt と検索インデックスには、止めたページの本文も今までどおり入ります
（frontmatter は入りません）。載せたくないページは `llms: false` も書いてください。

> [!WARNING]
> HTML コメント（`<!-- … -->`）は `.md` を止めても公開されます。生の HTML として
> そのまま出力 HTML に残り、llms-full.txt にも入ります。公開したくないメモは原稿に
> 書かないでください。社内で公開するときの注意は[社内で公開する](internal.md)にまとめています。
