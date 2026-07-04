//! render_body_html のスナップショットテスト（GFM 表・コード・mermaid・重複見出し）

use std::fs;

use yuzu_core::{
    CodeBlockRenderer, MarkdownOptions, NoopUrlRewriter, build_site_model, render_body_html,
};

/// mermaid だけ差し替えるテスト用レンダラ（render 側の実装の最小模倣）
struct MermaidOnlyRenderer;

impl CodeBlockRenderer for MermaidOnlyRenderer {
    fn render(&self, lang: Option<&str>, code: &str) -> Option<String> {
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
