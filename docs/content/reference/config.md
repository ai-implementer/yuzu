---
title: 設定（yuzu.jsonc）
order: 1
description: yuzu.jsonc の全設定キー・型・既定値
---

# 設定（yuzu.jsonc）

設定は JSONC（コメント・トレーリングカンマ可）で書きます。
`yuzu.jsonc` のあるディレクトリが**プロジェクトルート**です
（コマンドは cwd から上方向に探索します）。デフォルトをマージした
解決済み設定は `.yuzu/settings.json` に書き出されます。

**キーのタイポと重複は検出されます。** 未知のキー（`markdwon` など）は
黙って無視されると「設定したのに効かない」ことになるため、キーのパスと
行番号つきで報告します。同じキーを 2 回書いた場合（JSONC は後勝ち）も同様です。
`yuzu lint` / `yuzu check` では診断として出るので、CI のゲートに使えます
（[診断ルール](rules.md)の `config-unknown-key` / `config-duplicate-key`）。

なお未知のキーがあっても設定の読み込み自体は続きます。将来のバージョンで
追加されたキーを古いバイナリが読んでも壊れないようにするためです。

全キーを載せた設定例:

```jsonc
{
  "site": { "title": "My Docs", "description": "...", "lang": "ja", "baseUrl": "/docs/", "logo": "/images/logo.svg" },
  "input": { "dir": "content", "ignore": ["**/_drafts/**"] },
  "output": { "dir": "dist", "clean": true },
  "theme": {
    "name": "default",
    "dark": true,
    "cssVars": { "accent": "#0a6cff" },
    "cssVarsDark": { "accent": "#7fb2ff" }
  },
  "nav": { "auto": true },
  "markdown": {
    "gfm": true,
    "highlight": { "enabled": true, "themeLight": "InspiredGitHub", "themeDark": "base16-ocean.dark", "lineNumbers": false },
    "mermaid": { "enabled": true, "backend": "client" },
    "math": { "enabled": true },
    "crossref": { "numbering": "page" } // "site" でサイト全体の通し番号
  },
  "lint": {
    "maxDirectoryDepth": 1,
    "terms": { "サーバ": ["サーバー"] },
    "rules": { "fullwidthAlphanumeric": true, "halfwidthKana": true, "katakanaChoon": true }
  },
  "search": {
    "enabled": true,
    "dictionary": "models/custom.model.zst",
    "typoTolerance": { "enabled": true, "maxEdits": 1 },
    "shard": { "maxTermsPerShard": 16384 },
    "synonyms": [["ログイン", "サインイン"]],
    "indexCode": false
  },
  "llms": { "enabled": true, "full": true },
  "build": { "baseUrl": "/docs/", "watchIgnore": ["**/target", "**/node_modules"] },
  "dev": { "host": "127.0.0.1", "port": 5173, "liveReload": true, "open": false },
  "git": { "lastUpdated": true, "editUrl": "https://github.com/me/docs/edit/main/content/{path}" }
}
```

## site

| キー | 型 / 既定 | 説明 |
| --- | --- | --- |
| `title` | string / `"My Docs"` | サイトタイトル（ヘッダーと `<title>`） |
| `description` | string / なし | meta description |
| `lang` | string / `"ja"` | `<html lang>` |
| `baseUrl` | string / なし | サブパス配信時の基点（例 `"/docs/"`。[詳細](../guide/deploy.md)） |
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

出力先をリポジトリの外へ置きたいときは、`yuzu.jsonc` をその親ディレクトリへ移すか、
ビルド後に成果物をコピーしてください。

`input.dir` がルートの外を指す場合はエラーにしませんが、`yuzu lint` / `check` が
`config-path-outside-root` の警告を出します（診断のパス表示と `input.ignore` の
glob 評価が想定外になるため）。

## theme

| キー | 型 / 既定 | 説明 |
| --- | --- | --- |
| `name` | string / `"default"` | テーマ名 |
| `dark` | bool / `true` | ダークモード切替ボタンを出す |
| `cssVars` | object / `{}` | テーマ CSS 変数の上書き（キーは `--` 省略可） |
| `cssVarsDark` | object / `{}` | ダークモード時のみの上書き |

## nav

| キー | 型 / 既定 | 説明 |
| --- | --- | --- |
| `auto` | bool / `true` | ディレクトリ階層からサイドバーを自動生成 |

## markdown

| キー | 型 / 既定 | 説明 |
| --- | --- | --- |
| `gfm` | bool / `true` | GFM 拡張（表・打ち消し線・autolink・タスクリスト） |
| `highlight.enabled` | bool / `true` | ビルド時シンタックスハイライト（`false` でも `file=` の引用・キャプション・行番号は機能します。止まるのは配色だけです） |
| `highlight.themeLight` | string / `"InspiredGitHub"` | ライトモードの配色 |
| `highlight.themeDark` | string / `"base16-ocean.dark"` | ダークモードの配色 |
| `highlight.lineNumbers` | bool / `false` | コードブロックの行番号表示のサイト既定（ブロック単位の `showLineNumbers` / `noLineNumbers` が優先。[詳細](../guide/code-and-math.md)） |
| `mermaid.enabled` | bool / `true` | ` ```mermaid ` ブロックの描画 |
| `mermaid.backend` | `"client"` \| `"ssr"` / `"client"` | [SSR にすると 9 図種をビルド時 SVG 化](../guide/diagrams.md) |
| `math.enabled` | bool / `true` | 数式（同梱 KaTeX でクライアント描画） |
| `crossref.numbering` | `"page"` \| `"site"` / `"page"` | [図表番号](../guide/writing.md#図表番号と相互参照)の採番単位（ページ内連番 / サイト全体の通し番号） |

## lint

| キー | 型 / 既定 | 説明 |
| --- | --- | --- |
| `maxDirectoryDepth` | number / なし | `content` 配下のディレクトリ深さ制限（直下 = 0。未設定は無制限） |
| `terms` | object / `{}` | 用語統一の辞書（正しい表記 → ゆれ表記の配列） |
| `rules.fullwidthAlphanumeric` | bool / `true` | 全角英数字の検出 |
| `rules.halfwidthKana` | bool / `true` | 半角カナの検出 |
| `rules.katakanaChoon` | bool / `true` | 長音符ゆれ混在の検出 |

## search

| キー | 型 / 既定 | 説明 |
| --- | --- | --- |
| `enabled` | bool / `true` | 全文検索（インデックス生成と検索 UI） |
| `dictionary` | string / なし | vaporetto 分かち書きモデルの差し替え（プロジェクト相対パス） |
| `typoTolerance.enabled` | bool / `true` | タイポトレランス |
| `typoTolerance.maxEdits` | number / `1` | 許容する編集距離 |
| `shard.maxTermsPerShard` | number / `16384` | インデックスのシャード分割単位 |
| `synonyms` | string\[\]\[\] / `[]` | 同義語グループ（クエリ拡張。`lint.terms` と合成） |
| `indexCode` | bool / `false` | フェンスコードブロックを検索対象に含める |

## llms

| キー | 型 / 既定 | 説明 |
| --- | --- | --- |
| `enabled` | bool / `true` | llms.txt の生成 |
| `full` | bool / `true` | llms-full.txt（全文連結）の生成 |

## build / dev

| キー | 型 / 既定 | 説明 |
| --- | --- | --- |
| `build.baseUrl` | string / なし | `site.baseUrl` より優先する基点（CI 注入用。`--base-url` はさらに優先） |
| `build.watchIgnore` | string\[\] / `["**/target", "**/node_modules"]` | `yuzu dev` / `build --watch` の監視から除外する glob（プロジェクトルート相対）。**指定すると既定値を置き換えます**（追記ではありません） |
| `dev.host` | string / `"127.0.0.1"` | dev / preview のバインド先 |
| `dev.port` | number / `5173` | ポート |
| `dev.liveReload` | bool / `true` | WebSocket ライブリロード |
| `dev.open` | bool / `false` | `yuzu dev` 起動時にブラウザを開く |

監視は**プロジェクトルート全体**が対象です（コンテンツインクルードの参照先が
`content/` の外にもあるため）。出力ディレクトリと隠しディレクトリ（`.git` /
`.yuzu`）は `build.watchIgnore` の指定に関係なく常に除外されます。
ビルド生成物を大量に書くツールを同居させている場合は、ここへ足してください。

パターンはパス自身と**祖先ディレクトリ**に対して評価します。つまり
`"**/target"` と書けば `target/` 配下すべてが除外されます（ディレクトリの
作成イベント自体も含む）。`"**/target/**"` と書いても配下のファイルは除外
されますが、`target/` が作られた瞬間の 1 回は再ビルドが走ります。

`yuzu.jsonc` を保存すると設定を読み直してから再ビルドします。ただし監視と
配信の前提になっている `output.dir` / `baseUrl` / `dev.host` / `dev.port` /
`dev.liveReload` / `build.watchIgnore` は起動時の値のままで、変更すると
「再起動しないと反映されません」と警告します。

## git

| キー | 型 / 既定 | 説明 |
| --- | --- | --- |
| `lastUpdated` | bool / `false` | ページフッターに最終コミット日（git 不在時は自動で非表示） |
| `editUrl` | string / なし | 「このページを編集」リンク（`{path}` が content 相対パスに置換） |
