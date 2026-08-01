---
title: 診断ルール
order: 3
description: yuzu lint / yuzu check が報告する全ルールの一覧
---

# 診断ルール

`yuzu lint` と `yuzu check` の出力に出る `warning[duplicate-h1]` の `\[\]` の中身が、
このページのルール ID です。

```text
content/guide/x.md:12:1: warning[duplicate-h1] 本文に h1 が 2 個以上あります
```

`yuzu lint` のルールは `yuzu check` にすべて含まれます（check はこれにリンク検査・
エイリアス検証・整形差分を足したものです）。機械可読な出力については
[CLI コマンド](cli.md)の「診断の出力形式」を参照してください。

## yuzu lint のルール

深刻度はすべて warning です。設定列は 3 種類あります — 値で有効・無効を切り替えるもの、
設定しないと発火しないもの、常時有効なものです。

| ルール | 検出内容 | `--fix` | 設定 |
| --- | --- | --- | --- |
| `fullwidth-alphanumeric` | 本文中の全角英数字（`Ｗｅｂ１２３`） | 可 | `lint.rules.fullwidthAlphanumeric`（既定 `true`） |
| `halfwidth-kana` | 本文中の半角カナ（`ﾃﾞｰﾀ`） | 可 | `lint.rules.halfwidthKana`（既定 `true`） |
| `katakana-choon` | 長音符ゆれの混在（`サーバ` と `サーバー`） | 条件付き | `lint.rules.katakanaChoon`（既定 `true`） |
| `term-variant` | 辞書に登録したゆれ表記の出現 | 可 | `lint.terms`（未設定なら発火しない） |
| `duplicate-h1` | 本文の h1 が 2 個以上 | 不可 | 常時有効 |
| `heading-level-skip` | 見出しレベルの飛び（h2 の次に h4） | 不可 | 常時有効 |
| `directory-too-deep` | `content` 配下のディレクトリが深すぎる | 不可 | `lint.maxDirectoryDepth`（未設定なら発火しない） |
| `code-block-meta` | フェンス情報文字列の書き間違い・範囲外の行ハイライト | 不可 | 常時有効 |
| `duplicate-label` | 図表ラベル（`{#fig:x}`）の同一ページ内での重複 | 不可 | 常時有効 |
| `frontmatter-unknown-key` | frontmatter の未知のトップレベルキー | 不可 | 常時有効 |
| `config-unknown-key` | `yuzu.jsonc` の未知のキー（タイポ） | 不可 | 常時有効 |
| `config-duplicate-key` | `yuzu.jsonc` のキーの重複（JSONC は後勝ち） | 不可 | 常時有効 |
| `config-path-outside-root` | `input.dir` がプロジェクトルートの外を指す | 不可 | 常時有効 |

`katakana-choon` の「条件付き」は、長音符ゆれを**多数派の表記へ寄せる**ため、
同数で並んだときは正解を決められず報告だけになる、という意味です。

`config-` で始まる 2 つだけは、ページではなく `yuzu.jsonc` を指します
（パスはプロジェクトルート相対で出ます）。設定のタイポは無言で無視されて
「設定したのに効かない」事故になりやすいため、`yuzu lint` / `check` で
気づけるようにしてあります。

`code-block-meta` はフェンス情報文字列の問題をまとめて報告します
（`showLineNumbers` の書き間違い、`{2,4-6}` の解釈できない部分、`file=` のない `lines=`、
隣接するフェンスが無い単独の `tab=`、` ```include ` の `file=` 漏れと無視される表示メタ、
コード行数を超える行ハイライト、特別描画される言語への表示メタ指定）。
記法は[コードと数式](../guide/code-and-math.md)を参照してください。

## yuzu check が追加するルール

深刻度はすべて error で、常時有効・`--fix` では直せません。

| ルール | 検出内容 |
| --- | --- |
| `broken-link` | 内部リンクの切れ（外部 URL は検査しません） |
| `broken-anchor` | 見出し・図表ラベルへのアンカーの切れ |
| `alias-invalid` | frontmatter `aliases` の値が URL として解釈できない |
| `alias-conflict` | エイリアスが実ページや他のエイリアスと衝突する |
| `route-conflict` | 2 つ以上のページが同じ URL になる（`x.md` と `x/index.md`） |
| `unsafe-page-path` | ファイル名に URL で意味を持つ文字（`#` `?` `%` `"` `'` `<` `>` `` ` `` `\`） |
| `include-error` | コンテンツインクルード（`file=`）の参照先が読めない・範囲外。Markdown 断片（```include）の散文違反（見出し・キャプション行・脚注・frontmatter・`file=` の入れ子）もここで報告 |
| `spec-error` | `openapi` / `jsonschema` の `file:` 参照が読めない・仕様が壊れている・未対応バージョン・`$ref` 先が解決できない |
| `fmt` | 整形差分がある（`yuzu fmt` で修正、`yuzu fmt --diff` で内容を確認できます） |

`spec-error` だけは警告版の `spec-warning` もあります（参照ファイル数の上限超過など、
書き間違いではなく描画が注記へ縮退するだけのもの）。

`route-conflict` と `unsafe-page-path` は `yuzu build` も中断します。どちらも
「生成物のどこかに壊れたリンクや非決定な出力が残る」問題で、書き出してしまうと
気づけないためです。yuzu はファイル名を slug 化せずそのまま URL にするので、
`a#b.md` の URL `/a#b/` はブラウザから `/a` ＋ フラグメント `b/` に見えます。

> [!NOTE]
> API 仕様の描画は失敗してもビルドを止めません（執筆中に止まらないよう、
> エラーボックスや注記にして継続します）。そのため**公開前に気づける場所は
> `yuzu check` だけ**です。とくに `$ref` 先の失敗は画面上では小さな注記に
> なるため、見落としやすくなっています。

## 位置情報が付かないルール

`fmt` はファイル単位の診断、`directory-too-deep` / `route-conflict` / `unsafe-page-path` は
ファイル配置そのものの問題なので、どれも行番号を持ちません。出力では次のように見えます。

| 形式 | 見え方 |
| --- | --- |
| human | `content/x.md: error\[fmt\] …`（`:行:列` が付かない） |
| json | `line` と `column` が `null` |
| github | 位置指定のない注釈（ファイルの先頭に付きます） |

> [!NOTE]
> 無効化できるのは `lint.rules` の 3 ルールだけです。ほかは常時有効で、
> 行単位で抑制するコメントにも対応していません。
