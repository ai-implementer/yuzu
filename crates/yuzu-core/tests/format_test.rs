//! format_document（`yuzu fmt` の整形コア）のテスト。
//! 本文は normalize と同じ正規形、frontmatter はバイト温存

use std::fs;

use yuzu_core::{MarkdownOptions, Page, build_source_pages, format_document};

fn page_from(source: &str) -> Page {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("index.md"), source).unwrap();
    let pages = build_source_pages(dir.path(), &[], &MarkdownOptions::default()).unwrap();
    pages.into_iter().next().unwrap()
}

fn format_str(source: &str) -> String {
    format_document(&page_from(source), &MarkdownOptions::default()).unwrap()
}

const SAMPLE: &str = r#"---
# コメント行も温存される
title: "引用符 付きタイトル"
order: 2
description: 説明
---


見出し
===

* アスタリスク箇条書き
* 二つ目

裸 URL: https://example.com/path
"#;

#[test]
fn frontmatter_を温存して整形する() {
    let out = format_str(SAMPLE);
    // frontmatter はコメント・クォート・キー順ごとバイト温存
    assert!(
        out.starts_with(
            "---\n# コメント行も温存される\ntitle: \"引用符 付きタイトル\"\norder: 2\ndescription: 説明\n---\n\n"
        ),
        "out:\n{out}"
    );
    // 本文は正規形（setext → ATX、`*` → `-`、裸 URL → autolink）
    assert!(out.contains("# 見出し"), "out:\n{out}");
    assert!(out.contains("- アスタリスク箇条書き"), "out:\n{out}");
    assert!(out.contains("<https://example.com/path>"), "out:\n{out}");
    assert!(
        out.ends_with('\n') && !out.ends_with("\n\n"),
        "末尾改行は 1 個"
    );
    insta::assert_snapshot!("formatted_md", out);
}

#[test]
fn 整形は冪等() {
    let once = format_str(SAMPLE);
    let twice = format_str(&once);
    assert_eq!(once, twice, "format(format(x)) == format(x)");
}

#[test]
fn frontmatter_なしでも整形できる() {
    let out = format_str("見出し\n===\n\n本文\n");
    assert_eq!(out, "# 見出し\n\n本文\n");
}

#[test]
fn 本文が空でも壊れない() {
    let out = format_str("---\ntitle: 空\n---\n");
    assert_eq!(out, "---\ntitle: 空\n---\n");
    // 完全な空ファイルは空のまま
    assert_eq!(format_str(""), "");
}

#[test]
fn crlf_の本文は_lf_に正規化される() {
    let out = format_str("# 見出し\r\n\r\n一行目\r\n二行目\r\n");
    assert!(!out.contains('\r'), "out:\n{out:?}");
    assert_eq!(out, "# 見出し\n\n一行目\n二行目\n");
}

/// Phase 7 記法（Admonition・脚注）のサンプル。
/// 脚注定義を本文の途中に置き、未参照の定義も混ぜてある
const PHASE7_SAMPLE: &str = r#"---
title: 執筆表現
---

# 執筆表現

> [!note]
> 小文字で書いた種別

> [!WARNING] 独自タイトル
> 本文

先頭の参照[^used]。

[^used]: 使われる脚注

途中の段落。

[^unused]: 参照されない脚注
"#;

#[test]
fn admonition_は大文字正規化されタイトルを温存する() {
    let out = format_str(PHASE7_SAMPLE);
    assert!(out.contains("> [!NOTE]\n"), "out:\n{out}");
    assert!(out.contains("> [!WARNING] 独自タイトル"), "out:\n{out}");
}

#[test]
fn 脚注定義は位置と未参照を温存する() {
    let out = format_str(PHASE7_SAMPLE);
    // 定義が文書末尾へ移動させられない（「途中の段落」より前に留まる）
    let def = out.find("[^used]:").expect("定義が残る");
    let para = out.find("途中の段落").expect("段落が残る");
    assert!(def < para, "定義が末尾へ移動している:\n{out}");
    // 未参照の定義も削除されない
    assert!(out.contains("[^unused]:"), "out:\n{out}");
}

#[test]
fn phase7_記法でも整形は冪等() {
    let once = format_str(PHASE7_SAMPLE);
    let twice = format_str(&once);
    assert_eq!(once, twice, "format(format(x)) == format(x)");
}

/// Phase 8 記法（数式）。$ 区切りの温存・通貨表記の不干渉を固定する
const MATH_SAMPLE: &str = r#"---
title: 数式
---

# 数式

インライン $x^2 + y^2$ とコード数式 $`a+b`$ を含む段落。

$$
\int_0^1 f(x) \, dx
$$

コーヒーは $5、ランチは $12 かかる。
"#;

#[test]
fn 数式の_dollar_区切りは温存される() {
    let out = format_str(MATH_SAMPLE);
    assert!(out.contains("$x^2 + y^2$"), "out:\n{out}");
    assert!(out.contains("$`a+b`$"), "out:\n{out}");
    assert!(out.contains("$$\n\\int_0^1 f(x) \\, dx\n$$"), "out:\n{out}");
    // 通貨表記は数式化も $ エスケープもされない
    assert!(out.contains("コーヒーは $5、ランチは $12"), "out:\n{out}");
}

#[test]
fn 数式でも整形は冪等() {
    let once = format_str(MATH_SAMPLE);
    let twice = format_str(&once);
    assert_eq!(once, twice, "format(format(x)) == format(x)");
}

#[test]
fn math_無効なら_dollar_はテキストのまま() {
    let opts = MarkdownOptions {
        math: false,
        ..MarkdownOptions::default()
    };
    let out = format_document(&page_from("# t\n\n式 $x^2$ と $5 の話。\n"), &opts).unwrap();
    assert!(out.contains("式 $x^2$ と $5 の話。"), "out:\n{out}");
}
