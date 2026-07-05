---
title: はじめに
order: 1
description: yuzu プロジェクトの基本操作
---

# はじめに

## ビルドする

```bash
yuzu build
```

`content/**/*.md` がテーマ HTML になり、`dist/` に出力されます。

## 開発サーバで書く

```bash
yuzu dev
```

`content/` と `theme/` を監視して自動再ビルドし、WebSocket で
ブラウザを即リロードします（執筆はこれ 1 コマンド）。
`yuzu.jsonc` の `dev.open: true` で起動時にブラウザも開きます。

WebSocket が使えない環境では `yuzu build --watch`（ポーリング式）が退避先です。

## プレビューする

```bash
yuzu preview
```

ビルド済みの `dist/` を `http://127.0.0.1:5173/` で配信します。

## frontmatter

各ページの先頭に YAML frontmatter を書けます。

```yaml
---
title: ページタイトル # ナビの表示名（未指定は h1 → ファイル名）
order: 1 # ナビの並び順（未指定はファイル名順で最後尾）
draft: true # ビルドから除外する
description: 説明 # meta description
---
```

## ナビゲーション

`content/` のディレクトリ階層がそのままサイドバーの階層になります。
並び順は `order` 昇順、未指定はファイル名順です。

### ページ内目次（TOC）

h2 / h3 見出しは右側の「このページ」に自動で載ります。

### ダークモード

ヘッダー右上の ◐ ボタンで切り替えられます（`theme.dark: false` で無効化）。

## 図（Mermaid）

` ```mermaid ` ブロックで図が描けます。既定は同梱 mermaid.js によるクライアント描画です。

`yuzu.jsonc` で `"backend": "ssr"` にすると、**sequence・flowchart・state・
ER・gantt の 5 図種はビルド時に SVG 化**されます（JS 不要・ダークモードに即追従）。
未対応の図種は自動でクライアント描画にフォールバックし、
フォールバックが発生したページだけ mermaid.js が読み込まれます。

```mermaid
flowchart TD
    A[Markdown を書く] --> B{図の種類は?}
    B -->|対応 5 図種| C[tankan がビルド時に SVG 化]
    B -->|それ以外| D[mermaid.js でクライアント描画]
```

ガントチャート（gantt）:

```mermaid
gantt
    title ドキュメント整備の計画（例）
    dateFormat YYYY-MM-DD
    excludes weekends
    section 執筆
    構成を決める : done, plan, 2026-07-06, 2d
    本文を書く   : active, write, after plan, 5d
    section 公開
    レビュー     : review, after write, 3d
    公開         : milestone, after review, 1d
```

ER 図（erDiagram）:

```mermaid
erDiagram
    "サイト" ||--|{ "ページ" : contains
    "ページ" ||--o{ "見出し" : has
    "ページ" {
        string title "frontmatter の title"
        int order
        bool draft
    }
```

## 全文検索

ヘッダーの検索ボックス（`/` または `Cmd/Ctrl+K` でフォーカス）から日本語で検索できます。
1 文字の誤字にも寛容です。サーバは不要で、静的ホスティングだけで動きます。

ターミナルからも同じエンジンで検索できます:

```bash
yuzu search "検索したい言葉"
```

`search.enabled: false` で機能ごと無効化できます。

