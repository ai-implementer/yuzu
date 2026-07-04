//! normalize_markdown（正規化 Markdown 出力）のテスト。
//! llms-full.txt と将来の `yuzu fmt` の土台

use std::fs;

use yuzu_core::{MarkdownOptions, Page, build_site_model, normalize_markdown};

fn page_from(source: &str) -> Page {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("index.md"), source).unwrap();
    let site = build_site_model(dir.path(), &[], &MarkdownOptions::default()).unwrap();
    site.pages.into_iter().next().unwrap()
}

fn normalize_str(source: &str) -> String {
    normalize_markdown(&page_from(source), &MarkdownOptions::default()).unwrap()
}

const SAMPLE: &str = r#"---
title: 正規化サンプル
description: テスト
---

見出し
===

* アスタリスク箇条書き
* 二つ目
  * ネスト

| 機能 | 状態 |
|:---|---:|
| build | ✅ |

- [ ] 未完了タスク
- [x] 完了タスク

コードは `inline` と:

```rust
fn main() {}
```

裸 URL: https://example.com/path

改行を
含む段落。
"#;

#[test]
fn 正規化のスナップショット() {
    insta::assert_snapshot!("normalized_md", normalize_str(SAMPLE));
}

#[test]
fn frontmatter_は出力に含まれない() {
    let out = normalize_str(SAMPLE);
    assert!(!out.contains("title:"), "out:\n{out}");
    assert!(!out.starts_with("---"), "out:\n{out}");
    assert!(out.contains("見出し"), "本文は残る");
}

#[test]
fn 正規化は冪等() {
    let once = normalize_str(SAMPLE);
    let twice = normalize_markdown(&page_from(&once), &MarkdownOptions::default()).unwrap();
    assert_eq!(once, twice, "normalize(normalize(x)) == normalize(x)");
}

#[test]
fn 本文が空なら空文字列() {
    let out = normalize_str("---\ntitle: 空\n---\n");
    assert_eq!(out.trim(), "");
}
