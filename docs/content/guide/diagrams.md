---
title: 図（Mermaid / SSR）
order: 3
description: Mermaid 互換記法の図。9 図種は tankan がビルド時に SVG 化する
---

# 図（Mermaid / SSR）

` ```mermaid ` ブロックで図が描けます。描画方法は 2 つあります:

- **`backend: "client"`（既定）** — 同梱の mermaid.js がブラウザで描画します
- **`backend: "ssr"`** — Mermaid 互換の自前レンダラ **tankan** が
  **ビルド時に SVG 化**します。クライアント JS は不要で、ダークモード切替にも
  再描画なしで追従します

このサイトは `"ssr"` で運用しており、以下の図はすべてビルド時に生成された
SVG です。SSR 対応は **sequence・flowchart・class・state・ER・gantt・pie・
mindmap・timeline の 9 図種**。未対応の図種は自動でクライアント描画に
フォールバックし、フォールバックが発生したページだけ mermaid.js が
読み込まれます。

## flowchart

`classDef` / `class` / `:::` / `style` のスタイル指定も SSR に反映されます。
指定した色はダークモードでも意図どおり固定され、色付きボックスの文字色は
背景の明度から読みやすい側が自動で選ばれます。

```mermaid
flowchart TD
    A[Markdown を書く] --> B{図の種類は?}
    B -->|対応 9 図種| C[tankan がビルド時に SVG 化]:::ssr
    B -->|それ以外| D[mermaid.js でクライアント描画]
    classDef ssr fill:#d5e7fe,stroke:#014ba5
```

## sequence

```mermaid
sequenceDiagram
    autonumber
    actor W as 執筆者
    participant Y as yuzu dev
    participant B as ブラウザ

    W->>Y: content/*.md を保存
    Note over Y: 変更を検知して自動再ビルド
    Y-)B: reload 通知（WebSocket /__livereload）
    B->>Y: 再読み込み
    Y-->>B: 更新された HTML
```

## state

```mermaid
stateDiagram-v2
    [*] --> 下書き
    下書き --> レビュー中 : 提出
    レビュー中 --> 公開済み : 承認
    レビュー中 --> 下書き : 差し戻し
    公開済み --> [*]
```

## class

```mermaid
classDiagram
    class ページ {
        +String title
        +int order
        +本文() String
    }
    ページ <|-- 下書きページ : draft
    サイト "1" *-- "many" ページ : contains
```

## ER

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

## gantt

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

## pie

```mermaid
pie showData title コンテンツの内訳（例）
    "ガイド" : 12
    "リファレンス" : 8
    "リリースノート" : 5
```

## mindmap

インデント階層で書き、中央のルートから左右へ展開されます。

```mermaid
mindmap
  root((ドキュメント計画))
    構成
      ガイド
      リファレンス
    運用
      レビュー
      更新サイクル
```

## timeline

ロードマップや沿革に向いています。

```mermaid
timeline
    title リリースのあゆみ（例）
    section 立ち上げ
        4月 : 企画 : 要件整理
        5月 : プロトタイプ
    section 公開
        6月 : v1.0 リリース
```

## フォールバックの挙動

SSR が扱えない図（未対応の図種や `linkStyle` / `click` などの構文）は、
エラーにせず**自動でクライアント描画へフォールバック**します。その場合も
ページ全体のビルドは成功し、mermaid.js はフォールバックが発生したページに
だけ読み込まれます。クライアント描画の図もダークモード切替を監視して
再描画されるため、見た目の追従はどちらの経路でも保たれます。

> [!TIP]
> tankan は yuzu に依存しない汎用ライブラリで、単体でも
> [crates.io で公開](https://crates.io/crates/tankan)しています
> （`cargo add tankan`。I/O なし・時刻 / 乱数非依存・wasm32 対応）。
> 設計の詳細は[アーキテクチャ](../development/index.md)を参照してください。
