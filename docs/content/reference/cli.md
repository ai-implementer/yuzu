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
| `--port <番号>` | `--watch` のときの配信ポート（既定は設定の `dev.port`）。`yuzu dev` と並走させるとき |
| `--host <アドレス>` | `--watch` のときの配信アドレス（既定は設定の `dev.host`） |

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
| `--section <名前>` | セクション（サイドバーの第 1 階層）で絞り込む。複数指定でいずれか |
| `--json` | JSON で出力 |

絞り込みを指定しないときは `セクション: ガイド 2 / リファレンス 3 / 開発 6` の行が出ます。
ブラウザの検索と同じエンジンを通るので、絞り込みの結果と件数の整合はここで確かめられます。

### yuzu fmt / lint / check / llms

| フラグ | 説明 |
| --- | --- |
| `fmt --check` | 書き換えず差分のあるファイルを列挙して終了コード 1（CI 用） |
| `fmt --diff` | 書き換えず unified diff を標準出力へ（`--check` を含意。`patch -p1` に通る形） |
| `lint --fix` | 表記ゆれの変換候補をソースへ自動適用（修正できない違反は報告のまま残る） |
| `lint --format <形式>` | 出力形式（`human` / `json` / `github`。既定 `human`） |
| `check --format <形式>` | 同上 |
| `llms --full` | llms-full.txt（全ページの正規化 Markdown 連結）を出力 |

## 診断の出力形式

`yuzu lint` と `yuzu check` は `--format` で出力形式を選べます。ルール ID の一覧は
[診断ルール](rules.md)を参照してください。終了コードは形式によらず同じです。

### human（既定）

```text
content/guide/x.md:12:1: warning[duplicate-h1] 本文に h1 が 2 個以上あります
エラー 0 件・警告 1 件
```

ファイル単位の診断（`fmt` など）は `:行:列` が付きません。

### json

単一の JSON オブジェクトを標準出力へ出します。**標準出力には JSON 以外を出さない**ので、
そのままパイプで機械処理できます（`lint --fix` の進捗は標準エラー出力へ回ります）。

```json
{
  "diagnostics": [
    {
      "rule": "broken-link",
      "severity": "error",
      "path": "content/guide/x.md",
      "line": 12,
      "column": 1,
      "message": "リンク先 `missing.md` が見つかりません",
      "fixable": false
    }
  ],
  "summary": { "errors": 1, "warnings": 0, "pages": 12, "suppressed": 0, "disabled": 0 }
}
```

- `path` はプロジェクトルート相対で、区切りは常に `/` です
- `line` と `column` はファイル単位の診断では `null` になります（キー自体は必ずあります）
- `fixable` は `yuzu lint --fix` で自動修正できるかを表します
- `summary.suppressed` は frontmatter の `lintDisable` で抑制した診断の件数です
- `summary.disabled` は `lint.rules` のプロジェクト全体無効化で落とした診断の件数です
- キーは追加されることがありますが、削除・改名はしません

### github

GitHub Actions の注釈（ワークフローコマンド）を出します。プルリクエストの diff 行に
直接コメントが付きます。

```text
::error file=docs/content/guide/x.md,line=12,col=1,title=yuzu[broken-link]::リンク先 `missing.md` が見つかりません
```

パスは `GITHUB_WORKSPACE` からの相対に自動で付け替わります。ワークフローが
サブディレクトリへ移動してから実行しても（例: `cd docs`）、注釈がリポジトリの
正しいファイルに紐づきます。

> [!NOTE]
> 注釈として画面に表示されるのは 1 ステップあたり 10 件までです（残りはログに出ます）。
> 列は yuzu 内部の都合でバイト単位のため、日本語の行では GitHub の列表示と
> ずれることがあります（行の紐づけは正確です）。

`yuzu fmt --check` は診断ではなくファイル名を列挙するコマンドなので、`--format` の対象外です。

> [!TIP]
> キャッシュ起因の不具合を疑ったときは `--force` が最短です。
> `.yuzu/cache/` はいつ削除しても安全で、次のビルドがフルビルドに
> 縮退するだけです。
