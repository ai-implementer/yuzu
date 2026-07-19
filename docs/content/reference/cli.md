---
title: CLI コマンド
order: 2
description: yuzu の全コマンド・主要フラグ・終了コード規約
---

# CLI コマンド

## コマンド一覧

| コマンド | 役割 |
| --- | --- |
| `yuzu new <dir>` | サンプル docs プロジェクトを生成する |
| `yuzu build` | `content/` をビルドして `dist/` に静的サイトを出力する |
| `yuzu preview` | `dist/` を配信する最小静的サーバ |
| `yuzu dev` | 開発サーバ（監視ビルド＋配信＋WebSocket ライブリロード） |
| `yuzu search <クエリ>` | ビルド済みサイトをブラウザと同じエンジンで検索する |
| `yuzu llms` | llms.txt をその場で生成して標準出力へ（`dist/` 不要） |
| `yuzu fmt` | Markdown を正規形へ整形する（既定はその場で書き換え） |
| `yuzu lint` | 文書規約と表記ゆれの診断 |
| `yuzu check` | lint ＋ リンク切れ検査 ＋ fmt 差分検出の統合チェック |

## 終了コード規約

すべてのコマンドで共通です。CI の判定にそのまま使えます。

| コード | 意味 |
| --- | --- |
| `0` | 成功（lint / check / `fmt --check` は「違反なし」） |
| `1` | 違反あり（lint 警告・リンク切れ・fmt 差分） |
| `2` | 実行エラー（設定の不備・入出力エラーなど） |

## 主要フラグ

### yuzu build

| フラグ | 説明 |
| --- | --- |
| `--watch` | 監視ビルド＋配信＋ポーリング式オートリフレッシュ（WebSocket が使えない環境向け） |
| `--base-url <URL>` | baseUrl を上書き（`site` / `build` の設定より優先。CI からの注入用） |
| `--force` | インクリメンタルキャッシュ（`.yuzu/cache/`）を破棄してフルビルド |
| `--drafts` | `draft: true` のページも含めてビルド（下書きバナー付き） |

### yuzu dev / preview

| フラグ | 説明 |
| --- | --- |
| `--port <番号>` | ポート番号（既定は設定の `dev.port`） |
| `--host <アドレス>` | バインドアドレス（コンテナ内からは `--host 0.0.0.0`） |
| `--force`（dev のみ） | キャッシュを破棄してフルビルド |
| `--drafts`（dev のみ） | 下書きページも表示 |

### yuzu search

| フラグ | 説明 |
| --- | --- |
| `--limit <件数>` | 表示件数（既定 10） |
| `--json` | JSON で出力 |

### yuzu fmt / lint / llms

| フラグ | 説明 |
| --- | --- |
| `fmt --check` | 書き換えず差分のあるファイルを列挙して終了コード 1（CI 用） |
| `lint --fix` | 表記ゆれの変換候補をソースへ自動適用（修正できない違反は報告のまま残る） |
| `llms --full` | llms-full.txt（全ページの正規化 Markdown 連結）を出力 |

> [!TIP]
> キャッシュ起因の不具合を疑ったときは `--force` が最短です。
> `.yuzu/cache/` はいつ削除しても安全で、次のビルドがフルビルドに
> 縮退するだけです。
