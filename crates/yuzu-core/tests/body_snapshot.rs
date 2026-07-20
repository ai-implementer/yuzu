//! render_body_html のスナップショットテスト（GFM 表・コード・mermaid・重複見出し）

use std::fs;

use yuzu_core::{
    CodeBlockMeta, CodeBlockRenderer, MarkdownOptions, NoopUrlRewriter, build_site_model,
    render_body_html,
};

/// mermaid だけ差し替えるテスト用レンダラ（render 側の実装の最小模倣）
struct MermaidOnlyRenderer;

impl CodeBlockRenderer for MermaidOnlyRenderer {
    fn render(&self, lang: Option<&str>, _meta: &CodeBlockMeta, code: &str) -> Option<String> {
        if lang == Some("mermaid") {
            Some(format!(
                "<pre class=\"mermaid\">{}</pre>\n",
                code.replace('&', "&amp;").replace('<', "&lt;")
            ))
        } else {
            None
        }
    }
}

const SAMPLE: &str = r#"---
title: サンプル
description: スナップショット用
---

# サンプル

GFM の**表**:

| 機能 | 状態 |
| --- | --- |
| build | ✅ |
| ~~検索~~ | Phase 3 |

## コード

```rust
fn main() {
    println!("こんにちは yuzu");
}
```

## 図

```mermaid
sequenceDiagram
    participant A
    A->>B: hello
```

## 使い方

## 使い方

- [x] タスクリスト
- [ ] 未完了

autolink: https://example.com
"#;

#[test]
fn 本文_html_のスナップショット() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("index.md"), SAMPLE).unwrap();

    let site = build_site_model(dir.path(), &[], &MarkdownOptions::default()).unwrap();
    let html = render_body_html(
        &site.pages[0],
        &MarkdownOptions::default(),
        &MermaidOnlyRenderer,
        &NoopUrlRewriter,
    )
    .unwrap();

    insta::assert_snapshot!("body_html", html);
}

/// Phase 7 記法。HTML レンダ側の固定点:
/// - div.markdown-alert-{note,caution} と p.markdown-alert-title（既定題・独自題）
/// - 脚注は同一定義への複数参照を含め、section.footnotes が**末尾に 1 回だけ**出る
///   （fmt 側の「定義位置温存」オプションを誤って流用すると壊れる回帰の検知）
const ALERTS_FOOTNOTES: &str = r#"---
title: alerts と脚注
---

# alerts と脚注

> [!NOTE]
> 補足です。

> [!CAUTION] 独自タイトル
> 取り返しがつきません。

本文の参照[^a]と再利用[^a]。

[^a]: 脚注本文には**強調**も書ける。
"#;

/// Phase 8 記法（数式）。comrak の出力形を固定する:
/// - $..$ → span[data-math-style="inline"] / $$..$$ → span[data-math-style="display"]
/// - $`..`$ → code[data-math-style="inline"] / ```math → pre>code.language-math
/// - 通貨表記（直後が数字の $）は数式化されない・literal は HTML エスケープされる
const MATH: &str = r#"---
title: 数式
---

# 数式

インライン $x^2$ とコード数式 $`a+b`$ の段落。

$$
\int_0^1 f(x) \, dx
$$

```math
a^2 + b^2 = c^2
```

比較 $a < b$ と通貨 $100 と $200 の話。
"#;

#[test]
fn 数式の_html_スナップショット() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("index.md"), MATH).unwrap();

    let site = build_site_model(dir.path(), &[], &MarkdownOptions::default()).unwrap();
    let html = render_body_html(
        &site.pages[0],
        &MarkdownOptions::default(),
        &MermaidOnlyRenderer,
        &NoopUrlRewriter,
    )
    .unwrap();

    // 通貨表記は数式化されない
    assert!(html.contains("$100 と $200"), "html:\n{html}");
    insta::assert_snapshot!("math_html", html);
}

#[test]
fn alerts_と脚注の_html_スナップショット() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("index.md"), ALERTS_FOOTNOTES).unwrap();

    let site = build_site_model(dir.path(), &[], &MarkdownOptions::default()).unwrap();
    let html = render_body_html(
        &site.pages[0],
        &MarkdownOptions::default(),
        &MermaidOnlyRenderer,
        &NoopUrlRewriter,
    )
    .unwrap();

    assert_eq!(html.matches("<section class=\"footnotes\"").count(), 1);
    insta::assert_snapshot!("alerts_footnotes_html", html);
}
