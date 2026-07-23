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
