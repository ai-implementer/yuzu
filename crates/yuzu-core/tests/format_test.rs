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

#[test]
fn fmt_は_include_フェンスを温存し冪等() {
    // 断片インクルード（Phase 51）は fmt では展開しない。
    // 情報文字列（file= / lines=）はバイト等価で往復する（Phase 39 の契約）
    let src = "# t\n\n```include file=\"snippets/note.md\" lines=2-5\n```\n\n本文。\n";
    let once = format_str(src);
    assert_eq!(once, src, "1 バイトも変わらない");
    assert_eq!(format_str(&once), once, "冪等");
}

#[test]
fn 定義リストと約物強調は_fmt_を往復しても壊れない() {
    // Phase 53 で有効化した 2 フラグ（description_lists / cjk_friendly_emphasis）は
    // comrak_options 経由で fmt / normalize / linkcheck にも効く。
    // 「整形すると記法が別物になる」ことが無いのを冪等性で縛る
    let source = concat!(
        "# 見出し\n",
        "\n",
        "これは**「重要」**です。\n",
        "\n",
        "SSG\n",
        "\n",
        ": 静的サイトジェネレータ。\n",
    );
    let once = format_str(source);
    assert!(once.contains("**「重要」**"), "強調が壊れる:\n{once}");
    assert!(
        once.contains(": 静的サイトジェネレータ。"),
        "定義が壊れる:\n{once}"
    );
    assert_eq!(format_str(&once), once, "冪等でない:\n{once}");
}

#[test]
fn 抑制コメント直後の空行を落として密着形へ整形する() {
    // comrak は HtmlBlock の後に必ず空行を挿入するが、抑制コメントは
    // 対象行との密着形が正規形（restore_yuzu_syntax が空行を落とす）
    let source = concat!(
        "# 見出し\n",
        "\n",
        "<!-- yuzu-lint-disable-next-line term-variant -->\n",
        "サーバの説明。\n",
    );
    let out = format_str(source);
    assert!(
        out.contains("<!-- yuzu-lint-disable-next-line term-variant -->\nサーバの説明。"),
        "密着形にならない:\n{out}"
    );
    // 空行入りで書いても密着形へ正規化される
    let spaced = concat!(
        "# 見出し\n",
        "\n",
        "<!-- yuzu-lint-disable-next-line term-variant -->\n",
        "\n",
        "サーバの説明。\n",
    );
    assert_eq!(format_str(spaced), out, "空行入りは密着形へ寄る");
}

#[test]
fn 抑制コメントの整形は冪等() {
    let source = concat!(
        "# 見出し\n",
        "\n",
        "<!-- yuzu-lint-disable-next-line term-variant katakana-choon -->\n",
        "サーバの説明。\n",
        "\n",
        "<!-- yuzu-lint-disable-next-line halfwidth-kana -->\n",
        "<!-- yuzu-lint-disable-next-line fullwidth-alphanumeric -->\n",
        "続き。\n",
    );
    let once = format_str(source);
    assert_eq!(format_str(&once), once, "冪等でない:\n{once}");
    assert!(
        once.contains(
            "<!-- yuzu-lint-disable-next-line halfwidth-kana -->\n<!-- yuzu-lint-disable-next-line fullwidth-alphanumeric -->\n続き。"
        ),
        "積んだコメントも密着形:\n{once}"
    );
}

#[test]
fn 引用ブロック内の抑制コメントも密着形を保つ() {
    let source = concat!(
        "# 見出し\n",
        "\n",
        "> <!-- yuzu-lint-disable-next-line term-variant -->\n",
        "> サーバの説明。\n",
    );
    let once = format_str(source);
    assert!(
        once.contains("> <!-- yuzu-lint-disable-next-line term-variant -->\n> サーバの説明。"),
        "引用内で密着形にならない:\n{once}"
    );
    assert_eq!(format_str(&once), once, "冪等でない:\n{once}");
}

#[test]
fn tight_リスト項目内の抑制コメントを壊さない() {
    // tight リスト内では comrak は空行を挿入しない（1 改行に縮む）ため、
    // 復元条件が成立せず何もしないのが正
    let source = concat!(
        "# 見出し\n",
        "\n",
        "- 項目\n",
        "  <!-- yuzu-lint-disable-next-line term-variant -->\n",
        "  サーバの説明。\n",
    );
    let once = format_str(source);
    assert!(
        once.contains(
            "- 項目\n  <!-- yuzu-lint-disable-next-line term-variant -->\n  サーバの説明。"
        ),
        "リスト内の密着形が崩れた:\n{once}"kokokokoko
    );
    assert_eq!(format_str(&once), once, "冪等でない:\n{once}");
}

#[test]
fn 引用内の順序付きリストの桁上がりでもパニックせず原文のまま返す() {
    // comrak 0.53/0.54 の既知バグ: 入れ子内の順序付きリストが 9 → 10 項目で
    // 桁が増えると prefix 計算がずれて format_commonmark がパニックする。
    // yuzu 側で捕捉し、該当ページは整形スキップ（原文のまま = fmt 差分なし）
    let src = concat!(
        "---\ntitle: t\n---\n\n# t\n\n",
        "> 1. a\n> 2. b\n> 3. c\n> 4. d\n> 5. e\n",
        "> 6. f\n> 7. g\n> 8. h\n> 9. i\n> 10. j\n",
    );
    let out = format_str(src);
    assert_eq!(out, src, "原文のまま（整形スキップ）");
}

#[test]
fn yuzu_lint_以外の_html_コメントは空行付きのまま温存する() {
    let source = concat!(
        "# 見出し\n",
        "\n",
        "<!-- ただのメモ -->\n",
        "\n",
        "本文。\n",
    );
    let out = format_str(source);
    assert!(
        out.contains("<!-- ただのメモ -->\n\n本文。"),
        "普通のコメントの空行が消えた:\n{out}"
    );
    assert_eq!(format_str(&out), out, "冪等でない:\n{out}");
}
