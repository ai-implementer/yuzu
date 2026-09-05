---
title: 診断ルール
order: 3
description: yuzu lint / yuzu check が報告する全ルールの一覧
lintDisable:
  - fullwidth-alphanumeric
  - halfwidth-kana
  - katakana-choon
  - term-variant
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

深刻度はすべて warning です。「無効化可」のルールは
[`lint.rules`](#プロジェクト全体の無効化lintrules) の `false` で
プロジェクト全体を無効化できます（`config-*` と抑制機構自身の 2 ルールは対象外）。
`lint.terms` / `lint.max_directory_depth` を使うルールは、設定しない限り発火しません。

| ルール | 検出内容 | `--fix` | 設定 |
| --- | --- | --- | --- |
| `fullwidth-alphanumeric` | 本文中の全角英数字（Ｗｅｂ１２３） | 可 | 無効化可 |
| `halfwidth-kana` | 本文中の半角カナ（ﾃﾞｰﾀ） | 可 | 無効化可 |
| `katakana-choon` | 長音符ゆれの混在（「サーバ」と「サーバー」） | 条件付き | 無効化可 |
| `term-variant` | 辞書に登録したゆれ表記の出現 | 可 | `lint.terms`・無効化可 |
| `duplicate-h1` | 本文の h1 が 2 個以上 | 不可 | 無効化可 |
| `heading-level-skip` | 見出しレベルの飛び（h2 の次に h4） | 不可 | 無効化可 |
| `directory-too-deep` | `content` 配下のディレクトリが深すぎる | 不可 | `lint.max_directory_depth`・無効化可 |
| `code-block-meta` | フェンス情報文字列の書き間違い・範囲外の行ハイライト | 不可 | 無効化可 |
| `duplicate-label` | 図表ラベル（`{#fig:x}`）の同一ページ内での重複 | 不可 | 無効化可 |
| `frontmatter-unknown-key` | frontmatter の未知のトップレベルキー | 不可 | 無効化可 |
| `config-path-outside-root` | `input.dir` がプロジェクトルートの外を指す | 不可 | 常時有効 |
| `invalid-lint-suppression` | frontmatter `lintDisable` の未知・抑制不可のルール名 | 不可 | 常時有効 |
| `unused-lint-suppression` | `lintDisable` に書いたのにこのページで発火しなかった抑制 | 不可 | 常時有効 |

`katakana-choon` の「条件付き」は、長音符ゆれを**多数派の表記へ寄せる**ため、
同数で並んだときは正解を決められず報告だけになる、という意味です。

`config-` で始まるルールは、ページではなく `yuzu.toml` を指します
（パスはプロジェクトルート相対で出ます）。なお設定のキーのタイポ・型違い・
重複は診断ではなく**設定エラー**（終了コード 2）で、どのコマンドでも
読み込み時に止まります（[設定](config.md)参照）。

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
| `alias-invalid` | frontmatter `aliases` の値が URL として解釈できない（`#` `?` を含む・出力パスに使えない文字を含む等） |
| `alias-conflict` | エイリアスが実ページや他のエイリアスと衝突する |
| `route-conflict` | 2 つ以上のページが同じ URL になる（`x.md` と `x/index.md`）。自動生成されるページ（[用語集](../guide/writing.md#用語集と略語)・[検索結果ページ](../guide/search.md#検索結果ページ)）との衝突もここで報告します |
| `unsafe-page-path` | ファイル名（または `markdown.glossary.page` / `search.page`）に出力パスとして使えない文字（`\` と制御文字）。`#` `?` `%` 空白・日本語は URL 化のときにパーセントエンコードされるので対象外です。設定由来の `markdown.glossary.page` / `search.page` と `aliases` は、どの OS でビルドしても同じ出力パスを作るため、Windows で使えない `< > : " \| ? *` も全 OS で拒否します |
| `include-error` | コンテンツインクルード（`file=`）の参照先が読めない・範囲外。Markdown 断片（\`\`\`include）の散文違反（見出し・キャプション行・脚注・frontmatter・`file=` の入れ子）もここで報告 |
| `spec-error` | `openapi` / `jsonschema` の `file:` 参照が読めない・仕様が壊れている・未対応バージョン・`$ref` 先が解決できない |
| `fmt` | 整形差分がある（`yuzu fmt` で修正、`yuzu fmt --diff` で内容を確認できます） |

`spec-error` だけは警告版の `spec-warning` もあります（参照ファイル数の上限超過など、
書き間違いではなく描画が注記へ縮退するだけのもの）。warning なので
`lintDisable` の抑制と `lint.rules` の無効化の対象です。

`route-conflict` と `unsafe-page-path` は `yuzu build` も中断します。どちらも
「書き出してしまうと気づけない」問題です。前者は生成物のどこかに非決定な出力が
残り、後者は書き出しの途中で入出力エラーになります。

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

## ページ単位の抑制（lintDisable）

warning のルールは、frontmatter の `lintDisable` で**そのページに限り**抑制できます
（詳しくは[品質チェック](../guide/quality.md#ページ単位の抑制lintdisable)を参照）:

```yaml
---
lintDisable:
  - term-variant
  - katakana-choon
---
```

error のルールは抑制できません（壊れたリンクや非決定な出力が生成物に残るのを
防ぐためのルールです）。`config-*` はページではなく `yuzu.toml` を指すため
対象外です。未知・抑制不可のルール名は `invalid-lint-suppression`、
書いたのに発火しなかった抑制は `unused-lint-suppression` の warning になります
（直したのに残った指定を放置させないためです）。

なお、このページ自身も冒頭のルール表でゆれ表記の例を生のまま載せるために、
frontmatter の `lintDisable` で該当する 4 ルールを抑制しています
（このページの原文 Markdown が実例です）。

## 行単位の抑制（yuzu-lint-disable-next-line）

1 箇所だけ例外を通したいときは、HTML コメントで**次の内容行に限り**抑制できます
（詳しくは[品質チェック](../guide/quality.md#行単位の抑制yuzu-lint-disable-next-line)を参照）:

```md
<!-- yuzu-lint-disable-next-line term-variant katakana-choon -->
この行のゆれ表記は報告されません。
```

対象は「コメントの後、空行を飛ばした次の内容行」で、ルール名は空白区切りで
複数指定できます。抑制できるルールの範囲・invalid / unused の警告は
ページ単位（`lintDisable`）と同じ規律です。行番号を持たないルール
（`directory-too-deep` など）は行単位では抑制できず、ページ単位を使います。

## プロジェクト全体の無効化（lint.rules）

方針と合わないルールは、`yuzu.toml` の `lint.rules` に「ルール ID → `false`」を
書くと**プロジェクト全体で**無効化できます（対象は `lintDisable` で抑制できる
範囲と同じ warning のルールだけです）:

```toml
[lint.rules]
katakana-choon = false
term-variant = false
```

- 書かない ID は有効のままです（`true` は書いても書かなくても同じ）
- ルール ID のタイポ・error 系の ID・旧形式のキー（`katakanaChoon` 等）は
  無効化できる ID の一覧付きの**設定エラー**になります（黙って効かないまま
  進むことはありません）
- 無効化中のルールをページ（`lintDisable`）・行コメントで抑制していても
  `unused-lint-suppression` にはなりません。ルールを再有効化すると
  抑制はそのまま生き返ります
- 集計行には「（無効化 N 件）」、`--format json` には `summary.disabled` が出ます
