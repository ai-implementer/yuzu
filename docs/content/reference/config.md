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

同じキーを 2 回書くと後勝ちで黙って無視されるため、重複キーは
`site.title` 形式のパス付きで警告されます。

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
    "highlight": { "enabled": true, "themeLight": "InspiredGitHub", "themeDark": "base16-ocean.dark" },
    "mermaid": { "enabled": true, "backend": "client" },
    "math": { "enabled": true }
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
  "build": { "baseUrl": "/docs/" },
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
| `input.dir` | string / `"content"` | 原稿ディレクトリ |
| `input.ignore` | string\[\] / `[]` | 除外する glob パターン |
| `output.dir` | string / `"dist"` | 出力ディレクトリ |
| `output.clean` | bool / `true` | ビルド前に出力をクリーンする |

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
| `highlight.enabled` | bool / `true` | ビルド時シンタックスハイライト |
| `highlight.themeLight` | string / `"InspiredGitHub"` | ライトモードの配色 |
| `highlight.themeDark` | string / `"base16-ocean.dark"` | ダークモードの配色 |
| `mermaid.enabled` | bool / `true` | ` ```mermaid ` ブロックの描画 |
| `mermaid.backend` | `"client"` \| `"ssr"` / `"client"` | [SSR にすると 9 図種をビルド時 SVG 化](../guide/diagrams.md) |
| `math.enabled` | bool / `true` | 数式（同梱 KaTeX でクライアント描画） |

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
| `dev.host` | string / `"127.0.0.1"` | dev / preview のバインド先 |
| `dev.port` | number / `5173` | ポート |
| `dev.liveReload` | bool / `true` | WebSocket ライブリロード |
| `dev.open` | bool / `false` | `yuzu dev` 起動時にブラウザを開く |

## git

| キー | 型 / 既定 | 説明 |
| --- | --- | --- |
| `lastUpdated` | bool / `false` | ページフッターに最終コミット日（git 不在時は自動で非表示） |
| `editUrl` | string / なし | 「このページを編集」リンク（`{path}` が content 相対パスに置換） |
