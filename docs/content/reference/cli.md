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
| `yuzu completions <shell>` | シェル補完スクリプトを標準出力へ出す |

## 終了コード規約

すべてのコマンドで共通です。CI の判定にそのまま使えます。

| コード | 意味 |
| --- | --- |
| `0` | 成功（lint / check / `fmt --check` は「違反なし」） |
| `1` | 違反あり（lint 警告・リンク切れ・fmt 差分） |
| `2` | 実行エラー（設定の不備・入出力エラーなど） |

## グローバルフラグ

サブコマンドをまたいで効くフラグです。

| フラグ | 説明 |
| --- | --- |
| `--root <DIR>` | プロジェクトルート（`yuzu.toml` のあるディレクトリ）を指定する |
| `-q`, `--quiet` | 進捗ログ（info 以下）を出さない。警告とエラーは出る |
| `-v`, `--verbose` | 詳細ログ（debug）も出す |

`-q` / `-v` はログの量だけを変えます。診断・検索結果・集計行など標準出力の内容は
変わらず、終了コードも同じです。詳細は[ビルドの進捗ログ](#ビルドの進捗ログ)を参照してください。

`--root` の挙動:

- サブコマンドの**前後どちらにも**書けます（`yuzu --root docs build` と
  `yuzu build --root docs` は同じ）
- 指定すると**上方向探索をしません**。指定先に `yuzu.toml` が無ければ
  親を探さずに終了コード 2 で止まります（指定したつもりで親の設定を拾う事故を防ぐため）
- パスは絶対パスへ正規化されます（相対指定・シンボリックリンク経由でも同じ結果）
- `yuzu new` と `yuzu completions` では使えません。どちらも既存プロジェクトを
  読まないコマンドで、指定すると終了コード 2 で止まります（黙って無視しません）
- `yuzu fmt --diff` が出すパスは**そのプロジェクトルート相対**です。
  `patch -p1` はプロジェクトルートで当ててください

`--root` を使わない場合は、従来どおり cwd から上方向に `yuzu.toml` を探します。

## 主要フラグ

### yuzu build

| フラグ | 説明 |
| --- | --- |
| `--watch` | 監視ビルド＋配信＋ポーリング式オートリフレッシュ（WebSocket が使えない環境向け） |
| `--base-url <URL>` | `base_url` を上書き（`site` / `build` の設定より優先。CI からの注入用） |
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

配信は**シンボリックリンクを辿りません**（`yuzu build` がリンクへの書き込みを
拒否するのと同じ規律で、検査はプロジェクトルートから要求ファイルまでの経路全体です）。
リンクを含む経路への要求は存在しないものとして 404 になり、理由はターミナルに
警告として出ます。

### yuzu search

| フラグ | 説明 |
| --- | --- |
| `--limit <件数>` | 表示件数（既定 10） |
| `--section <名前>` | セクション（サイドバーの第 1 階層）で絞り込む。複数指定でいずれか |
| `--format <形式>` | 出力形式（`human` / `json`。既定 `human`） |

絞り込みを指定しないときは `セクション: ガイド 2 / リファレンス 3 / 開発 6` の行が出ます。
ブラウザの検索と同じエンジンを通るので、絞り込みの結果と件数の整合はここで確かめられます。

`--format json` は検索結果の JSON 配列だけを標準出力へ出します（`lint` / `check` の
`--format json` と同じ「標準出力に JSON 以外を書かない」規律）。v0.17 より前の
`--json` も同じ意味で引き続き使えますが、`--format` と同時には指定できません。

### yuzu fmt / lint / check / llms

| フラグ | 説明 |
| --- | --- |
| `fmt --check` | 書き換えず差分のあるファイルを列挙して終了コード 1（CI 用） |
| `fmt --diff` | 書き換えず unified diff を標準出力へ（`--check` を含意。`patch -p1` に通る形） |
| `lint --fix` | 表記ゆれの変換候補をソースへ自動適用（修正できない違反は報告のまま残る） |
| `lint --format <形式>` | 出力形式（`human` / `json` / `github`。既定 `human`） |
| `check --format <形式>` | 同上 |
| `check --external-links` | 外部リンク（`http` / `https`）の到達性も検査する（HTTP は `curl` に委譲。HTTP 4xx を warning `external-link-broken` で報告し、到達不能・5xx・429 はスキップ件数に数える。[品質チェック](../guide/quality.md#外部リンクの検査opt-in)参照） |
| `llms --full` | llms-full.txt（全ページの正規化 Markdown 連結）を出力 |

## シェル補完

`yuzu completions <shell>` が補完スクリプトを標準出力へ出します。対応シェルは
`bash` / `zsh` / `fish` / `powershell` / `elvish` です。サブコマンド・フラグ・
`--format` の値（`human` / `json`）まで補完され、スクリプトはバイナリの定義から
その場で生成するので、yuzu を更新すれば新しいオプションも自動で入ります。

シェルの設定ファイルに 1 行足すのが最短です:

```bash
# bash（~/.bashrc）
eval "$(yuzu completions bash)"

# zsh（~/.zshrc。compinit より後に置く）
eval "$(yuzu completions zsh)"

# fish（~/.config/fish/config.fish）
yuzu completions fish | source
```

```powershell
# PowerShell（$PROFILE）
yuzu completions powershell | Out-String | Invoke-Expression
```

起動を速くしたいときはファイルへ保存して読み込みます（yuzu 更新時に再生成）:

```bash
yuzu completions bash > ~/.local/share/bash-completion/completions/yuzu
yuzu completions zsh > ~/.zfunc/_yuzu    # fpath に ~/.zfunc を入れておく
yuzu completions fish > ~/.config/fish/completions/yuzu.fish
```

`--section` のセクション名や検索クエリのような、ビルド結果に依存する候補は
補完しません（補完のたびに `dist/_search` を読むことになるため）。

## ビルドの進捗ログ

`yuzu build` / `yuzu dev` は処理中のページを 1 ページ 1 行で標準エラーへ
表示します（本文の `レンダ` と検索の `索引`。インクリメンタルビルドで
再計算を省いたページは `（キャッシュ）` 付き）。並列処理のため行の順序は
実行ごとに変わりますが、生成物は同一です。監視中の再ビルドでは
「変更を検知」の行に変更されたファイル（先頭 5 件）も出ます。

ログの量はグローバルフラグ `-q` / `-v` で調整できます（既定 `info`）:

```bash
yuzu build -q   # 進捗を含む info ログを出さない（警告とエラーは出る）
yuzu dev -v     # 出力先パスなどの debug ログも表示
```

環境変数 `RUST_LOG` でも同じ調整ができ、`yuzu_render=debug` のようにクレート単位で
絞ることもできます。`-q` / `-v` を付けたときは **`RUST_LOG` より優先**します
（シェルに残った環境変数に黙って負けないため）。`-q` と `-v` の同時指定はエラーです。

```bash
RUST_LOG=warn yuzu build   # -q と同じ
RUST_LOG=debug yuzu dev    # -v と同じ
```

ログはすべて標準エラーに出るため、標準出力のパイプやリダイレクトは
影響を受けません。`yuzu build 2>&1 | head` のように読み手が先に閉じても、
ビルドは最後まで完走します（書けなかったログは捨てます）。

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
  "summary": { "errors": 1, "warnings": 0, "pages": 12, "suppressed": 0, "disabled": 0, "skipped": 0 }
}
```

- `path` はプロジェクトルート相対で、区切りは常に `/` です
- `line` と `column` はファイル単位の診断では `null` になります（キー自体は必ずあります）
- `fixable` は `yuzu lint --fix` で自動修正できるかを表します
- `summary.suppressed` は frontmatter の `lintDisable` で抑制した診断の件数です
- `summary.disabled` は `lint.rules` のプロジェクト全体無効化で落とした診断の件数です
- `summary.skipped` は `check --external-links` で検査できなかった外部 URL の数です
  （到達不能・5xx・429。環境依存の失敗を診断に混ぜないための逃がし先で、
  フラグなしでは常に 0 です）
- キーは追加されることがありますが、削除・改名はしません

### github

GitHub Actions の注釈（ワークフローコマンド）を出します。プルリクエストの diff 行に
直接コメントが付きます。

```text
::error file=docs/content/guide/x.md,line=12,col=1,title=yuzu[broken-link]::リンク先 `missing.md` が見つかりません
```

パスは `GITHUB_WORKSPACE` からの相対に自動で付け替わります。ワークフローが
`--root docs` でサブディレクトリを指定しても、`cd docs` してから実行しても、
注釈がリポジトリの正しいファイルに紐づきます。

> [!NOTE]
> 注釈として画面に表示されるのは 1 ステップあたり 10 件までです（残りはログに出ます）。
> 列は yuzu 内部の都合でバイト単位のため、日本語の行では GitHub の列表示と
> ずれることがあります（行の紐づけは正確です）。

`yuzu fmt --check` は診断ではなくファイル名を列挙するコマンドなので、`--format` の対象外です。

> [!TIP]
> キャッシュ起因の不具合を疑ったときは `--force` が最短です。
> `.yuzu/cache/` はいつ削除しても安全で、次のビルドがフルビルドに
> 縮退するだけです。
