---
title: 図（Mermaid）
order: 4
description: Mermaid 記法の図。ブラウザで描くか、ビルド時に SVG にするかを選べる
---

# 図（Mermaid）

` ```mermaid ` ブロックで図が描けます。描き方は 2 つあり、`yuzu.toml` で選びます:

```toml
[markdown.mermaid]
backend = "ssr" # 既定は "client"（ブラウザで描く）。"ssr" はビルド時に SVG にする
```

- **`client`（既定）** — 同梱の mermaid.js がブラウザで図を描きます
- **`ssr`** — yuzu が**ビルド時に SVG にします**。ブラウザの JavaScript は使わず、
  ダークモードの切り替えにも描き直しなしで追従します

## client と ssr の選び方

|  | client（既定） | ssr |
| --- | --- | --- |
| ブラウザの JavaScript | 必要（無効だと図の代わりに Mermaid の原文が出る） | 不要 |
| 読み込むファイル | 全ページで mermaid.min.js（約 3.5 MB）を読み込む（2 ページ目からはブラウザのキャッシュ） | 図の SVG が本文に入るだけ |
| 描ける図 | Mermaid のすべての図種 | 下の 10 図種。それ以外の図は client と同じ描き方になる |
| ダークモードの切り替え | 図を描き直す | 描き直さずに配色が変わる |
| 印刷 | ダークモードのまま印刷すると図だけ暗い配色になる | 常にライトの配色で印刷される |

閲覧する環境で JavaScript が制限されている・ページを軽くしたい・印刷して配る、の
どれかに当てはまるなら `ssr` を選んでください。10 図種以外の図（`journey`・
`gitGraph` など）を多く使うなら `client` のままで構いません（`ssr` にしても、その図は
client と同じ描き方になり、そのページだけ mermaid.min.js を読み込みます）。

このサイトは `ssr` で運用しており、以下の図はすべてビルド時に生成された SVG です。
`ssr` で SVG にできるのは **sequence・flowchart・class・state・ER・gantt・pie・
mindmap・timeline・packet の 10 図種**です。

## flowchart（フローチャート）

`classDef` / `class` / `:::` / `style` のスタイル指定も SSR に反映されます。
指定した色はダークモードでも意図どおり固定され、色付きボックスの文字色は
背景の明度から読みやすい側が自動で選ばれます。

```mermaid
flowchart TD
    A[Markdown を書く] --> B{図の種類は?}
    B -->|対応 10 図種| C[tankan がビルド時に SVG 化]:::ssr
    B -->|それ以外| D[mermaid.js でクライアント描画]
    classDef ssr fill:#d5e7fe,stroke:#014ba5
```

## sequence（シーケンス図）

```mermaid
sequenceDiagram
    autonumber
    actor W as 執筆者
    participant Y as yuzu dev
    participant B as ブラウザ

    W->>Y: content/*.md を保存
    Note over Y: 変更を検知して自動再ビルド
    Y-)B: 再読み込みの通知
    B->>Y: 再読み込み
    Y-->>B: 更新された HTML
```

## state（状態遷移図）

```mermaid
stateDiagram-v2
    [*] --> 下書き
    下書き --> レビュー中 : 提出
    レビュー中 --> 公開済み : 承認
    レビュー中 --> 下書き : 差し戻し
    公開済み --> [*]
```

## class（クラス図）

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

## ER（ER 図）

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

## gantt（ガントチャート）

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

## pie（円グラフ）

```mermaid
pie showData title コンテンツの内訳（例）
    "ガイド" : 12
    "リファレンス" : 8
    "リリースノート" : 5
```

## mindmap（マインドマップ）

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

## timeline（タイムライン）

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

## packet（パケット図）

ネットワークプロトコルのヘッダーなど、ビット列のフィールド構造を描けます。
frontmatter の `config.packet`（`bitsPerRow` など）にも対応しています。

```mermaid
---
title: "TCP Packet"
---
packet
0-15: "Source Port"
16-31: "Destination Port"
32-63: "Sequence Number"
64-95: "Acknowledgment Number"
96-99: "Data Offset"
100-105: "Reserved"
106: "URG"
107: "ACK"
108: "PSH"
109: "RST"
110: "SYN"
111: "FIN"
112-127: "Window"
128-143: "Checksum"
144-159: "Urgent Pointer"
160-191: "(Options and Padding)"
192-255: "Data (variable length)"
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
