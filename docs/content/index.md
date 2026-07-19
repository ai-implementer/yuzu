---
title: yuzu とは
order: 1
description: Markdown の設計書を静的 HTML ドキュメントサイトへ変換する Rust 製ツール
---

# yuzu とは

yuzu 🍊 は、Markdown で書いた設計書を**プロダクション品質の静的 HTML
ドキュメントサイト**へ変換する Rust 製のツールです。

このサイト自体が yuzu でビルドされています（リポジトリの
[docs/](https://github.com/ai-implementer/yuzu/tree/main/docs) が原稿で、
push のたびに GitHub Pages へ自動デプロイ）。左のサイドバー・右の目次・
ヘッダーの検索ボックス・ダークモード切替・各ページの「Markdown をコピー」は、
すべて yuzu が生成したものです。

## 特徴

- **書くことに集中できる**: `content/**/*.md` を置くだけでナビ・目次・
  前後ページリンク・パンくずが付いたサイトになります。`yuzu dev` は
  保存から約 1 秒でブラウザを自動リロードします
- **設計書のための表現力**: シンタックスハイライト・数式（KaTeX）・
  Mermaid 互換の図（9 図種はビルド時に SVG 化）・OpenAPI / JSON Schema の
  静的レンダリングを標準装備
- **日本語のための検索**: 分かち書き＋BM25 の全文検索が静的ホスティングだけで
  動きます。誤字に寛容で、フレーズ検索・同義語展開・コードブロック検索にも対応
- **品質を保つ道具**: `yuzu fmt`（決定的整形）・`yuzu lint`（表記ゆれ検出と
  自動修正）・`yuzu check`（リンク切れ検査）を CI にそのまま組み込めます
- **LLM 連携**: llms.txt / llms-full.txt と、ページ単位の Markdown 配信・
  コピーボタンを自動生成します
- **速い**: インクリメンタルビルド＋ページ並列化で、編集のたびの再ビルドは
  変更ページ分だけです

## まずはここから

| 目的 | ページ |
| --- | --- |
| インストールして動かす | [ガイド](guide/index.md) |
| 記法を知る | [執筆の基本](guide/writing.md) |
| 図を描く | [図（Mermaid / SSR）](guide/diagrams.md) |
| 検索を使いこなす | [全文検索](guide/search.md) |
| 設定を調べる | [設定リファレンス](reference/config.md) |
| 内部設計を読む | [アーキテクチャ](development/index.md) |

> [!TIP]
> ヘッダーの検索ボックスは <kbd>/</kbd> または <kbd>Cmd</kbd>+<kbd>K</kbd> で
> フォーカスできます。試しに「インクリメンタル」や `"フレーズ検索"` で
> 検索してみてください。

## ライセンス

MIT または Apache-2.0 のデュアルライセンスです。ソースコードは
[GitHub](https://github.com/ai-implementer/yuzu) にあります。
