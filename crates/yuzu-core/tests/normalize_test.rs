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

/// Phase 7 記法（Admonition・脚注）。llms-full.txt は原文に忠実な正規形を出す
const PHASE7_SAMPLE: &str = r#"---
title: 執筆表現
---

# 執筆表現

> [!TIP]
> ヒント

先頭の参照[^used]。

[^used]: 使われる脚注

途中の段落。

[^unused]: 参照されない脚注
"#;

#[test]
fn 脚注定義は位置と未参照を温存する() {
    let out = normalize_str(PHASE7_SAMPLE);
    let def = out.find("[^used]:").expect("定義が残る");
    let para = out.find("途中の段落").expect("段落が残る");
    assert!(def < para, "定義が末尾へ移動している:\n{out}");
    assert!(out.contains("[^unused]:"), "out:\n{out}");
    assert!(out.contains("> [!TIP]"), "out:\n{out}");
}

#[test]
fn phase7_記法でも正規化は冪等() {
    let once = normalize_str(PHASE7_SAMPLE);
    let twice = normalize_markdown(&page_from(&once), &MarkdownOptions::default()).unwrap();
    assert_eq!(once, twice, "normalize(normalize(x)) == normalize(x)");
}

#[test]
fn 数式は_llms_向け正規形でも温存される() {
    let out = normalize_str(
        "---\ntitle: 数式\n---\n\n# 数式\n\n式 $x^2$ と:\n\n$$\nE = mc^2\n$$\n\n```math\na^2 + b^2\n```\n",
    );
    assert!(out.contains("$x^2$"), "out:\n{out}");
    assert!(out.contains("$$\nE = mc^2\n$$"), "out:\n{out}");
    assert!(
        out.contains("``` math\na^2 + b^2\n```") || out.contains("```math\na^2 + b^2\n```"),
        "out:\n{out}"
    );
}
