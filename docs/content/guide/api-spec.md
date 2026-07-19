---
title: API 仕様の描画
order: 4
description: OpenAPI / JSON Schema をビルド時に静的 HTML 化する
---

# API 仕様の描画

` ```openapi ` / ` ```jsonschema ` ブロックに API 仕様（YAML / JSON）を書くと、
**ビルド時に静的 HTML 化**されます。クライアント JS はゼロで、テーマと
ダークモードに統合された見た目になります。パースに失敗した場合は
エラーボックスを表示してビルド自体は継続します。

## JSON Schema

単体のスキーマは ` ```jsonschema ` ブロックで描画します:

```jsonschema
title: ページ
type: object
required: [route, title]
properties:
  route:
    type: string
    description: サイト内のルート（例 guide/search）
  title:
    type: string
    description: frontmatter の title
  draft:
    type: boolean
    description: 下書きフラグ（通常ビルドから除外）
  tags:
    type: array
    items:
      type: string
    description: タグ一覧
```

## OpenAPI（インライン）

OpenAPI 3.x / Swagger 2.0 の文書全体は ` ```openapi ` ブロックで描画します。
info・paths・パラメータ・レスポンスが操作ごとの開閉式パネルになります:

```openapi
openapi: 3.0.3
info:
  title: 検索 API（サンプル）
  version: 1.0.0
paths:
  /search:
    get:
      summary: 全文検索
      parameters:
        - name: q
          in: query
          required: true
          description: クエリ（`"..."` で囲むとフレーズ検索）
          schema:
            type: string
      responses:
        "200":
          description: 成功
          content:
            application/json:
              schema:
                type: array
                items:
                  $ref: "#/components/schemas/Hit"
components:
  schemas:
    Hit:
      type: object
      required: [url, title]
      properties:
        url:
          type: string
          description: ヒットしたセクションへの URL（アンカー付き）
        title:
          type: string
          description: ページ › 見出し
        excerpt:
          type: string
          description: 一致箇所周辺の抜粋（ハイライト付き）
```

## ファイル参照（file:）

ブロックの中身を `file: <パス>` の 1 行だけにすると、**プロジェクトルート
相対**の仕様ファイルを参照できます。仕様ファイル側の変更は次のビルドで
必ず反映されます（参照ページはキャッシュ対象外）。

下の描画は、このリポジトリの `specs/sample-api.yaml` を
` ```openapi ` ブロックから `file: specs/sample-api.yaml` で
参照したものです:

```openapi
file: specs/sample-api.yaml
```

## 対応範囲のメモ

- **`$ref` の解決**: 文書内参照（`#/components/schemas/...`）と、
  **プロジェクト内の別ファイル**への参照（`schemas/common.yaml#/...`。
  仕様ファイル内はファイル相対・インラインブロック内はルート相対）を
  解決します。HTTP 参照とプロジェクトルート外は拒否し、循環参照は
  参照名の表示に縮退します
- **Swagger 2.0**: `definitions` / `in: body` のリクエストボディ /
  `produces`・`consumes` のメディアタイプ表示に対応しています
- **スキーマ一覧**: 文書末尾の「スキーマ」に `components/schemas`
  （2.0 は `definitions`）の**全スキーマ**が折りたたみで並びます。
  上の例の `ApiError` のように、どの操作からも参照されないスキーマも
  ここから読めます
