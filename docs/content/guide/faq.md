---
title: よくある質問とトラブル
order: 12
description: 検索が動かない・リンクが切れる・図が出ないときの対処と、問い合わせ先
---

# よくある質問とトラブル

## 検索が動かない・リンクやスタイルが崩れる

`dist/index.html` をダブルクリックして、ファイルとして（アドレスが `file://` の状態で）
開いていないか確認してください。yuzu が出力するページのリンクと CSS・JavaScript の参照は
サイトのルート（`/`）から始まるので、ファイルとして開くと崩れ、検索も動きません。
手元では `yuzu preview` で開き、公開するときは Web サーバに置いてください
（[dist はサーバで配信する](install.md#dist-はサーバで配信する)）。

## サブパスに置いたらリンクや CSS が切れる

`https://example.com/docs/` のようにサブパスに置くときは、`base_url` を指定して
ビルドします。

```toml
[site]
base_url = "/docs/"
```

コマンドラインの `yuzu build --base-url /docs/` でも指定できます（設定より優先）。
`yuzu new` が同梱する GitHub Pages のワークフローは、公開先の URL を自動で渡します。
`./` のような相対パスは使えません（設定エラーになります）。詳しくは
[配信とデプロイ](deploy.md#サブパス配信base_url)を参照してください。

## 「ポート 5173 は既に使用中です」と出る

別の `yuzu dev` / `yuzu preview` や、ほかのツールが同じポートを使っています。
そのプロセスを止めるか、`--port` で別のポートを指定してください。

```bash
yuzu dev --port 5174
```

いつも別のポートを使うなら、`yuzu.toml` の `[dev]` に `port = 5174` と書きます。

## 図が表示されない

図をブラウザで描く既定の設定（`[markdown.mermaid]` の `backend = "client"`）では、
ブラウザの JavaScript が無効なときや mermaid.min.js を読み込めないとき
（ファイルとして開いたときなど）に、図の代わりに Mermaid の原文が表示されます。
JavaScript を使えない閲覧環境なら、ビルド時に SVG にする `backend = "ssr"` を
使ってください（[client と ssr の選び方](diagrams.md#client-と-ssr-の選び方)）。

`backend = "ssr"` で図の書き方に誤りがあると、ビルドのログに次の警告が出て、
その図だけブラウザで描く形に切り替わります。行番号を見て図を直してください。

    WARN mermaid の構文エラー（クライアント描画へフォールバック): 2 行目: ノード id がありません

## 改行が CRLF のファイルに整形差分が出る

`yuzu check` は、改行が CRLF のファイルを整形差分（`error[fmt]`）として報告します。
`yuzu fmt` を実行すると LF に揃います。Windows で編集する場合は、git が改行を
変えないよう、リポジトリに `.gitattributes` を置いてください:

```text
*.md text eol=lf
```

## 変更が反映されない

`yuzu build` は前回の結果を `.yuzu/cache/` に残して、変更したページだけを
作り直します。結果がおかしいと思ったら、`yuzu build --force` で最初から作り直して
ください（`.yuzu/` を削除しても同じです。いつ消しても安全です）。

## 質問・不具合の報告

質問・不具合の報告・機能の要望は
[GitHub の Issues](https://github.com/ai-implementer/yuzu/issues) で受け付けています。
不具合の報告には、`yuzu --version` の出力・OS・再現する手順（できれば最小の原稿と
`yuzu.toml`）を添えてください。「内部エラーが起きました（yuzu の不具合です…）」と
出たときも、その表示をそのまま貼ってください。

セキュリティ上の問題（脆弱性）は、公開の Issues には書かず、
[SECURITY.md](https://github.com/ai-implementer/yuzu/blob/main/SECURITY.md) の手順で
非公開で報告してください。
