---
title: コードと数式
order: 2
description: syntect によるビルド時ハイライト・コピーボタン・KaTeX 数式
---

# コードと数式

## シンタックスハイライト

コードブロックは **syntect がビルド時にハイライト**し、CSS クラスとして
出力します。クライアント JS はゼロで、ライト / ダークの両テーマに追従します。

```rust
/// ページ派生物のキャッシュキー（3 層無効化の最下層）
fn source_hash(bytes: &[u8]) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    bytes.hash(&mut hasher);
    hasher.finish()
}
```

two-face の拡張構文セットにより、TypeScript / TSX / TOML / Dockerfile なども
ハイライトされます:

```toml
[site]
title = "yuzu"
lang = "ja"
```

```typescript
type Page = { route: string; title: string; order?: number };
const byOrder = (a: Page, b: Page) => (a.order ?? Infinity) - (b.order ?? Infinity);
```

## タイトル・行ハイライト・行番号

フェンスの情報文字列に言語へ続けてメタを書くと、キャプションと行の強調が
使えます（すべてビルド時に HTML 化。クライアント JS はゼロのままです）:

````markdown
```rust title="src/hello.rs" {2,4-6} showLineNumbers
````

- `title="..."` — ファイル名などのキャプションをブロック上部に表示します
- `{2,4-6}` — 指定した行を強調します（1 始まり。番号とレンジのカンマ区切り）
- `showLineNumbers` / `noLineNumbers` — 行番号表示をブロック単位で切り替えます
  （サイト既定は設定の `markdown.highlight.lineNumbers`。既定 false）

下は 3 つすべてを使った実例です:

```rust title="src/hello.rs" {2,4-6} showLineNumbers
fn main() {
    let name = "yuzu";
    let mut lines = Vec::new();
    for i in 1..=3 {
        lines.push(format!("{i}: こんにちは {name}"));
    }
    println!("{}", lines.join("\n"));
}
```

行番号は CSS カウンタによる表示なので、コピーボタンや範囲選択のコピーには
混入しません。メタは検索インデックスにも入りません（コード本文だけが索引
対象）。`yuzu fmt` は情報文字列を逐語で温存します。書き間違い
（`showLineNumbers` のタイポ・コードの行数を超えた行ハイライトなど）は
描画では無視されますが、`yuzu lint` が行番号付きで警告します。

なお mermaid / openapi / jsonschema / math のような特別レンダリングされる
ブロックでは、これらのメタは無視されます。

## ソースファイルの埋め込み

情報文字列に `file=` を書くと、**実ファイルの中身をビルド時に読み込んで**
コードブロックにします。設計書にコードを転記して古くなる問題を防げます:

````markdown
```jsonc file="yuzu.jsonc" lines=15-31
````

- パスは**プロジェクトルート相対**です（ルート外への参照は拒否されます）
- `lines=15-31`（範囲）/ `lines=7`（単一行）で切り出せます。省略時はファイル全体
- `title` を省略すると `パス:行範囲` が自動でキャプションになります
- 言語を省略した場合は拡張子から推定します
- 行ハイライト `{2}` は**切り出した後の相対行**を指します

下は、このサイト自身の `yuzu.jsonc` から Markdown 設定の部分を引用した例です
（設定を変更すれば、このページの表示も次のビルドで自動的に変わります）:

```jsonc file="yuzu.jsonc" lines=15-31
```

参照先を編集すると、`yuzu dev` はプロジェクトルートを監視しているので
自動で再ビルドされます（`dist/` や隠しディレクトリは監視対象外）。
参照切れ・行範囲外は `yuzu check` がエラーとして報告し、ビルド時は
エラーボックスを表示して他のページの生成は続けます。

> [!NOTE]
> 埋め込んだ内容は `search.indexCode` が有効なら検索インデックスに載ります
> （表示されているものは検索できる、を保つため）。一方 llms.txt は原文の
> ままです（`yuzu fmt` の正規形と一致する不変条件を保つため）。

## タブ / コードグループ

連続するフェンスに `tab="..."` を書くと、**1 つのタブグループ**に束ねられます。
言語別のサンプルや OS 別の手順を切り替えて見せるときに使います。

````markdown
```sh tab="macOS / Linux"
curl -fsSL https://example.com/install.sh | sh
```

```powershell tab="Windows"
winget install example
```
````

```sh tab="macOS / Linux"
curl -fsSL https://example.com/install.sh | sh
```

```powershell tab="Windows"
winget install example
```

- グループになるのは**隣接したフェンス**だけです。間に段落や見出しを挟むと
  そこでグループが切れるので、意図しないブロックを巻き込みません
- 切り替えは radio と CSS の `order` だけで、**クライアント JS は使いません**。
  タブの枚数に上限はありません
- `title=` や行ハイライトなど、ほかの表示メタと併用できます
- 素の Markdown ビューアで開くと、**コードが縦に並ぶだけ**で壊れません
- 選択されていないタブの中身も HTML には含まれるため、検索・llms.txt の扱いは
  通常のコードブロックと同じです（折りたたみと同じ考え方）

> [!NOTE]
> `tab=` を 1 つのフェンスだけに書くと切り替え先が無いため、通常のコード
> ブロックとして描画されます。指定が効かないまま気づけないので、
> `yuzu lint` が `code-block-meta` で警告します。

Markdown の断片（注意書き・免責文など）を取り込みたい場合は、コード引用ではなく
[Markdown 断片のインクルード](writing.md#markdown-断片のインクルード)
（` ```include `）を使います。

## コピーボタン

コードブロックの右上から、中身をワンクリックでコピーできます
（Clipboard API のプログレッシブエンハンスメント。JS 無効・非 https の
環境ではボタン自体が現れません）。行番号・キャプションはコピーに含まれず、
コードだけがコピーされます。

## 数式（KaTeX）

GitHub 互換の記法で数式が書けます。描画は**同梱の KaTeX** が
クライアントで行い、**数式のあるページだけ** CSS / JS（約 600KB）を
読み込みます。

インライン数式は `$...$` で書きます: $E = mc^2$

ブロック数式は `$$...$$` です:

$$
\text{BM25}(D, Q) = \sum_{i=1}^{n} \text{IDF}(q_i) \cdot \frac{f(q_i, D) \cdot (k_1 + 1)}{f(q_i, D) + k_1 \cdot \left(1 - b + b \cdot \frac{|D|}{\text{avgdl}}\right)}
$$

` ```math ` ブロックも使えます:

```math
a^2 + b^2 = c^2
```

> [!NOTE]
> `$100` のような通貨表記は数式になりません（直後に数字が来る `$` は無効）。
> 数式が不要なら `markdown.math.enabled: false` で機能ごと無効化できます。
