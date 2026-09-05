---
title: 設定（yuzu.toml）
order: 1
description: yuzu.toml の全設定キー・型・既定値
---

# 設定（yuzu.toml）

設定は TOML で書きます。`yuzu.toml` のあるディレクトリが**プロジェクトルート**です
（コマンドは cwd から上方向に探索します）。すべてのキーは省略可能で、
省略したキーは既定値になります。キーは `snake_case` です。

**キーのタイポ・型違い・重複は設定エラーで止まります**（終了コード 2）。
未知のキー（`markdwon` など）が黙って無視されると「設定したのに効かない」ことに
なるため、行番号・列番号と、その階層で使えるキーの一覧を付けて報告します。
型の違う値（`port = "5173"`）や選択肢に無い値（`backend = "server"`）も同様で、
問題は 1 回の実行で**全件**まとめて出ます。同じキーを 2 回書くと TOML の
構文エラーです。

パーサは [kabosu](../development/kabosu.md)（依存ゼロの TOML ライブラリ）で、
対応範囲は TOML のサブセットです。yuzu の設定に必要な構文
（テーブル・文字列・整数・小数・真偽値・配列・コメント。文字列は複数行も可）は
すべて使えますが、インラインテーブル（`{ ... }`）・日時・テーブルの配列（`[[...]]`）は
未対応で、書くと「未対応の構文」として書き換え先のヒント付きで報告されます。
辞書のような入れ子は `[lint.terms]` のようにテーブルヘッダで書いてください。

全キーを載せた設定例:

```toml
[site]
title = "My Docs"
description = "..."
lang = "ja"
base_url = "/docs/"
logo = "/images/logo.svg"

[input]
dir = "content"
ignore = ["**/_drafts/**"]

[output]
dir = "dist"
clean = true

[theme]
name = "default"
dark = true

[theme.css_vars]
accent = "#0a6cff"

[theme.css_vars_dark]
accent = "#7fb2ff"

[theme.toc]
levels = "2-3"

[nav]
auto = true
collapse = true

[markdown]
gfm = true

[markdown.highlight]
enabled = true
theme_light = "InspiredGitHub"
theme_dark = "base16-ocean.dark"
line_numbers = false

[markdown.mermaid]
enabled = true
backend = "client"

[markdown.math]
enabled = true

[markdown.crossref]
numbering = "page" # "site" でサイト全体の通し番号

[markdown.glossary]
abbr = true
page = "glossary"
page_title = "用語集"

[markdown.glossary.terms]
SSG = "Static Site Generator"

[lint]
max_directory_depth = 1

[lint.terms]
"サーバ" = ["サーバー"]

[lint.rules]
katakana-choon = true # false でプロジェクト全体を無効化（error と抑制機構自身は不可）

[search]
enabled = true
page = "search" # 検索結果ページの route（空なら生成しない）
page_title = "検索"
page_size = 10
dictionary = "models/custom.model.zst"
synonyms = [["ログイン", "サインイン"]]
index_code = false

[search.typo_tolerance]
enabled = true
max_edits = 1

[search.shard]
max_terms_per_shard = 16384

[llms]
enabled = true
full = true

[build]
base_url = "/docs/"
watch_ignore = ["**/target", "**/node_modules"]

[dev]
host = "127.0.0.1"
port = 5173
live_reload = true
open = false

[git]
last_updated = true
edit_url = "https://github.com/me/docs/edit/main/content/{path}"
```

## site

| キー | 型 / 既定 | 説明 |
| --- | --- | --- |
| `title` | string / `"Documentation"` | サイトタイトル（ヘッダーと `<title>`） |
| `description` | string / なし | meta description |
| `lang` | string / `"ja"` | `<html lang>` |
| `base_url` | string / なし | サブパス配信時の基点（例 `"/docs/"`。[詳細](../guide/deploy.md)） |
| `logo` | string / なし | ヘッダーのロゴ画像（`public/` 配下のパス。未指定なら 🍊） |

## input / output

| キー | 型 / 既定 | 説明 |
| --- | --- | --- |
| `input.dir` | string / `"content"` | 原稿ディレクトリ（プロジェクトルート配下の相対パス） |
| `input.ignore` | string\[\] / `[]` | 除外する glob パターン |
| `output.dir` | string / `"dist"` | 出力ディレクトリ（プロジェクトルート配下の相対パス。制約は下記） |
| `output.clean` | bool / `true` | ビルド前に出力をクリーンする |

`output.dir` は**プロジェクトルート配下の相対パス**でなければなりません。
`output.clean` がこのディレクトリを丸ごと削除するため、次はエラーになります。

- 絶対パス（`"/var/www"`）
- `..` でルートの外へ出るパス（`"../site"`）
- プロジェクトルート自身（`""` / `"."`）
- `input.dir` / `public/` / `theme/` / `.yuzu` と**重なる**パス
  （同じ・親・子のいずれも。`"content/sub"` のような子も対象です）

ルートから出力先までの**経路にシンボリックリンクがある**場合もビルドを中断します
（`dist` 自身でも途中のディレクトリでも同じ）。リンク先が原稿やプロジェクト外を
指していると、書き込みや `output.clean` の削除がそこへ届いてしまうためです。
この検査は `output.clean` の設定やインクリメンタルビルドに関係なく毎回行います。

出力先をリポジトリの外へ置きたいときは、`yuzu.toml` をその親ディレクトリへ移すか、
ビルド後に成果物をコピーしてください。

`input.dir` がルートの外を指す場合はエラーにしませんが、`yuzu lint` / `check` が
`config-path-outside-root` の警告を出します（診断のパス表示と `input.ignore` の
glob 評価が想定外になるため）。

## theme

| キー | 型 / 既定 | 説明 |
| --- | --- | --- |
| `name` | string / `"default"` | テーマ名 |
| `dark` | bool / `true` | ダークモード切替ボタンを出す |
| `css_vars` | table / `{}` | テーマ CSS 変数の上書き（キーは `--` 省略可）。`[theme.css_vars]` のテーブルで書く |
| `css_vars_dark` | table / `{}` | ダークモード時のみの上書き |
| `toc.levels` | string / `"2-3"` | ページ内 TOC に載せる見出しレベルの範囲（`"2-4"` / `"2"` の形。h1〜h6 = 1〜6） |

## nav

| キー | 型 / 既定 | 説明 |
| --- | --- | --- |
| `auto` | bool / `true` | ディレクトリ階層からサイドバーを自動生成（現在は自動生成のみ対応。`false` は将来の手動ナビ定義用の予約で、指定しても効果はありません） |
| `collapse` | bool / `true` | サイドバーで現在ページの祖先セクションだけを開き、他を折りたたむ（クリックでその場展開できます）。`false` で従来の全展開 |

## markdown

| キー | 型 / 既定 | 説明 |
| --- | --- | --- |
| `gfm` | bool / `true` | GFM 拡張（表・打ち消し線・autolink・タスクリスト） |
| `highlight.enabled` | bool / `true` | ビルド時シンタックスハイライト（`false` でも `file=` の引用・キャプション・行番号は機能します。止まるのは配色だけです） |
| `highlight.theme_light` | string / `"InspiredGitHub"` | ライトモードの配色 |
| `highlight.theme_dark` | string / `"base16-ocean.dark"` | ダークモードの配色 |
| `highlight.line_numbers` | bool / `false` | コードブロックの行番号表示のサイト既定（ブロック単位の `showLineNumbers` / `noLineNumbers` が優先。[詳細](../guide/code-and-math.md)） |
| `mermaid.enabled` | bool / `true` | ` ```mermaid ` ブロックの描画 |
| `mermaid.backend` | `"client"` \| `"ssr"` / `"client"` | [SSR にすると 10 図種をビルド時 SVG 化](../guide/diagrams.md) |
| `math.enabled` | bool / `true` | 数式（同梱 KaTeX でクライアント描画） |
| `crossref.numbering` | `"page"` \| `"site"` / `"page"` | [図表番号](../guide/writing.md#図表番号と相互参照)の採番単位（ページ内連番 / サイト全体の通し番号） |
| `glossary.terms` | table / `{}` | [用語集](../guide/writing.md#用語集と略語)の辞書（略語 → 説明文）。`[markdown.glossary.terms]` のテーブルで書く。空なら機能ごと無効 |
| `glossary.abbr` | bool / `true` | 本文の初出を `<abbr title="説明">` にする（`false` で用語集ページだけ生成） |
| `glossary.page` | string / `"glossary"` | 用語集ページの URL（`content` 相対・拡張子なし）。`""` で生成しない |
| `glossary.page_title` | string / `"用語集"` | 用語集ページのタイトル（h1 とサイドバーの表示名） |

## lint

| キー | 型 / 既定 | 説明 |
| --- | --- | --- |
| `max_directory_depth` | integer / なし | `content` 配下のディレクトリ深さ制限（直下 = 0。未設定は無制限） |
| `terms` | table / `{}` | 用語統一の辞書（正しい表記 → ゆれ表記の配列）。`[lint.terms]` のテーブルで書く |
| `rules` | table / 全ルール有効 | ルール ID → bool のマップ。`false` でプロジェクト全体無効化（対象は warning のルールのみ。書かない ID は有効。一覧は[診断ルール](rules.md)）。無効化できない ID・タイポは設定エラー |

## search

| キー | 型 / 既定 | 説明 |
| --- | --- | --- |
| `enabled` | bool / `true` | 全文検索（インデックス生成と検索 UI） |
| `page` | string / `""` | [検索結果ページ](../guide/search.md#検索結果ページ)の route。**空なら生成しない**（既存の `content/search.md` 等と衝突しないための既定） |
| `page_title` | string / `"検索"` | 検索結果ページのタイトル |
| `page_size` | integer / `10` | 検索結果ページで 1 回に表示する件数（「さらに表示」で追加） |
| `dictionary` | string / なし | vaporetto 分かち書きモデルの差し替え（プロジェクト相対パス） |
| `typo_tolerance.enabled` | bool / `true` | タイポトレランス |
| `typo_tolerance.max_edits` | integer / `1` | 許容する編集距離 |
| `shard.max_terms_per_shard` | integer / `16384` | インデックスのシャード分割単位 |
| `synonyms` | string\[\]\[\] / `[]` | 同義語グループ（クエリ拡張。`lint.terms` と合成） |
| `index_code` | bool / `false` | フェンスコードブロックを検索対象に含める |

## llms

| キー | 型 / 既定 | 説明 |
| --- | --- | --- |
| `enabled` | bool / `true` | llms.txt の生成 |
| `full` | bool / `true` | llms-full.txt（全文連結）の生成 |

## build / dev

| キー | 型 / 既定 | 説明 |
| --- | --- | --- |
| `build.base_url` | string / なし | `site.base_url` より優先する基点（CI 注入用。`--base-url` はさらに優先） |
| `build.watch_ignore` | string\[\] / `["**/target", "**/node_modules"]` | `yuzu dev` / `build --watch` の監視から除外する glob（プロジェクトルート相対）。**指定すると既定値を置き換えます**（追記ではありません） |
| `dev.host` | string / `"127.0.0.1"` | dev / preview のバインド先 |
| `dev.port` | integer / `5173` | ポート |
| `dev.live_reload` | bool / `true` | WebSocket ライブリロード |
| `dev.open` | bool / `false` | `yuzu dev` 起動時にブラウザを開く |

監視は**プロジェクトルート全体**が対象です（コンテンツインクルードの参照先が
`content/` の外にもあるため）。出力ディレクトリと隠しディレクトリ（`.git` /
`.yuzu`）は `build.watch_ignore` の指定に関係なく常に除外されます。
ビルド生成物を大量に書くツールを同居させている場合は、ここへ足してください。

パターンはパス自身と**祖先ディレクトリ**に対して評価します。つまり
`"**/target"` と書けば `target/` 配下すべてが除外されます（ディレクトリの
作成イベント自体も含む）。`"**/target/**"` と書いても配下のファイルは除外
されますが、`target/` が作られた瞬間の 1 回は再ビルドが走ります。

`yuzu.toml` を保存すると設定を読み直してから再ビルドします。ただし監視と
配信の前提になっている `output.dir` / `base_url` / `dev.host` / `dev.port` /
`dev.live_reload` / `build.watch_ignore` は起動時の値のままで、変更すると
「再起動しないと反映されません」と警告します。

## git

| キー | 型 / 既定 | 説明 |
| --- | --- | --- |
| `last_updated` | bool / `false` | ページフッターに最終コミット日（git 不在時は自動で非表示） |
| `edit_url` | string / なし | 「このページを編集」リンク（`{path}` が content 相対パスに置換） |
