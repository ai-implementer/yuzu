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

/// 図表番号と相互参照（Phase 43）: 採番・アンカー・空リンクの自動補完
#[test]
fn 図表キャプションの採番と参照補完() {
    const SRC: &str = concat!(
        "# 見出し\n\n",
        "```mermaid\ngraph TD; A-->B\n```\n\n",
        "Figure: 依存関係 {#fig:deps}\n\n",
        "| a | b |\n| --- | --- |\n| 1 | 2 |\n\n",
        "表: 対応表 {#tbl:matrix}\n\n",
        "```rust\nfn main() {}\n```\n\n",
        "リスト: サンプル {#lst:main}\n\n",
        "図をもう 1 つ。\n\n",
        "Figure: 2 枚目 {#fig:second}\n\n",
        "参照: [](#fig:deps) / [](#tbl:matrix) / [](#lst:main) / [](#fig:second)。\n",
        "テキスト付き: [この図](#fig:deps)。未定義: [](#fig:missing)。\n",
    );
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("index.md"), SRC).unwrap();

    let site = build_site_model(dir.path(), &[], &MarkdownOptions::default()).unwrap();
    let page = &site.pages[0];

    // メタ抽出でラベルが採番されている（種別ごとに独立・文書順）
    let labels: Vec<(&str, usize)> = page
        .labels
        .iter()
        .map(|l| (l.id.as_str(), l.number))
        .collect();
    assert_eq!(
        labels,
        vec![
            ("fig:deps", 1),
            ("tbl:matrix", 1),
            ("lst:main", 1),
            ("fig:second", 2)
        ]
    );

    let html = render_body_html(
        page,
        &MarkdownOptions::default(),
        &MermaidOnlyRenderer,
        &NoopUrlRewriter,
    )
    .unwrap();

    // キャプションはアンカー付きで採番表示される
    assert!(html.contains(r#"<p class="caption caption-fig" id="fig:deps"><span class="caption-label">図 1</span>: 依存関係</p>"#), "{html}");
    assert!(
        html.contains(r#"id="tbl:matrix"><span class="caption-label">表 1</span>"#),
        "{html}"
    );
    assert!(
        html.contains(r#"id="lst:main"><span class="caption-label">リスト 1</span>"#),
        "{html}"
    );
    assert!(
        html.contains(r#"id="fig:second"><span class="caption-label">図 2</span>"#),
        "{html}"
    );

    // 空テキストのリンクは採番テキストで補完され、テキスト付きはそのまま
    assert!(html.contains(r##"<a href="#fig:deps">図 1</a>"##), "{html}");
    assert!(
        html.contains(r##"<a href="#tbl:matrix">表 1</a>"##),
        "{html}"
    );
    assert!(
        html.contains(r##"<a href="#lst:main">リスト 1</a>"##),
        "{html}"
    );
    assert!(
        html.contains(r##"<a href="#fig:second">図 2</a>"##),
        "{html}"
    );
    assert!(
        html.contains(r##"<a href="#fig:deps">この図</a>"##),
        "{html}"
    );
    // 未定義ラベルは補完しない（check が broken-anchor で報告する）
    assert!(html.contains(r##"<a href="#fig:missing"></a>"##), "{html}");

    // 素の段落（キャプションでない）は普通の <p> のまま
    assert!(html.contains("<p>図をもう 1 つ。</p>"), "{html}");
}

/// 折りたたみ Admonition（Phase 44）: `-` / `+` マーカーで details 化
#[test]
fn 折りたたみ_admonition_は_details_になる() {
    const SRC: &str = concat!(
        "# 見出し\n\n",
        "> [!NOTE]- 閉じた状態\n> 中身の段落。\n>\n> - リスト\n> - も入る\n\n",
        "> [!TIP]+ 開いた状態\n> 最初から開いている。\n\n",
        "> [!CAUTION]-\n> タイトル省略。\n\n",
        "> [!WARNING] 通常のまま\n> 折りたたまない。\n",
    );
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("index.md"), SRC).unwrap();

    let site = build_site_model(dir.path(), &[], &MarkdownOptions::default()).unwrap();
    let html = render_body_html(
        &site.pages[0],
        &MarkdownOptions::default(),
        &MermaidOnlyRenderer,
        &NoopUrlRewriter,
    )
    .unwrap();

    // `-` は閉じた details、`+` は open 付き
    assert!(
        html.contains(r#"<details class="markdown-alert markdown-alert-note">"#),
        "{html}"
    );
    assert!(
        html.contains(r#"<summary class="markdown-alert-title">閉じた状態</summary>"#),
        "{html}"
    );
    assert!(
        html.contains(r#"<details class="markdown-alert markdown-alert-tip" open>"#),
        "{html}"
    );
    // タイトル省略時は comrak と同じ既定タイトル
    assert!(
        html.contains(r#"<summary class="markdown-alert-title">Caution</summary>"#),
        "{html}"
    );
    // マーカーなしは従来どおり div のまま
    assert!(
        html.contains(r#"<div class="markdown-alert markdown-alert-warning">"#),
        "{html}"
    );
    assert!(
        html.contains(r#"<p class="markdown-alert-title">通常のまま</p>"#),
        "{html}"
    );

    // 中身（段落・リスト）は details の内側に順序どおり入る
    let note = html
        .split(r#"<details class="markdown-alert markdown-alert-note">"#)
        .nth(1)
        .and_then(|s| s.split("</details>").next())
        .unwrap();
    assert!(note.contains("<p>中身の段落。</p>"), "{note}");
    assert!(note.contains("<li>リスト</li>"), "{note}");
    assert_eq!(html.matches("<details").count(), 3, "{html}");
    assert_eq!(html.matches("</details>").count(), 3, "{html}");
}

/// 図表番号のサイト全体通し番号（Phase 45）: サイドバー表示順で連番になる
#[test]
fn サイト通し番号は表示順で連続する() {
    let dir = tempfile::tempdir().unwrap();
    // order で表示順を固定（index → a → b）
    fs::write(
        dir.path().join("index.md"),
        "---\ntitle: ホーム\norder: 1\n---\n\n# ホーム\n\nFigure: 1 枚目 {#fig:a}\n",
    )
    .unwrap();
    fs::write(
        dir.path().join("second.md"),
        "---\ntitle: 2 番目\norder: 2\n---\n\n# 2 番目\n\nFigure: 2 枚目 {#fig:b}\n\nTable: 表 {#tbl:b}\n",
    )
    .unwrap();
    fs::write(
        dir.path().join("third.md"),
        "---\ntitle: 3 番目\norder: 3\n---\n\n# 3 番目\n\nFigure: 3 枚目 {#fig:c}\n\n参照: [](#fig:c)。\n",
    )
    .unwrap();

    let site_opts = MarkdownOptions {
        crossref_site_numbering: true,
        ..MarkdownOptions::default()
    };
    let site = yuzu_core::build_site_model(dir.path(), &[], &site_opts).unwrap();
    let number_of = |id: &str| {
        site.pages
            .iter()
            .flat_map(|p| &p.labels)
            .find(|l| l.id == id)
            .unwrap()
            .number
    };
    // 図は表示順に 1, 2, 3。表は別カウンタなので 1
    assert_eq!(number_of("fig:a"), 1);
    assert_eq!(number_of("fig:b"), 2);
    assert_eq!(number_of("fig:c"), 3);
    assert_eq!(number_of("tbl:b"), 1);

    // 本文 HTML のキャプション・参照も同じ通し番号になる
    let third = site.pages.iter().find(|p| p.route == "third/").unwrap();
    let html = render_body_html(third, &site_opts, &MermaidOnlyRenderer, &NoopUrlRewriter).unwrap();
    assert!(
        html.contains(r#"<span class="caption-label">図 3</span>"#),
        "{html}"
    );
    assert!(html.contains(r##"<a href="#fig:c">図 3</a>"##), "{html}");

    // 既定（ページ内連番）では各ページ 1 から
    let site = yuzu_core::build_site_model(dir.path(), &[], &MarkdownOptions::default()).unwrap();
    let number_of = |id: &str| {
        site.pages
            .iter()
            .flat_map(|p| &p.labels)
            .find(|l| l.id == id)
            .unwrap()
            .number
    };
    assert_eq!(number_of("fig:a"), 1);
    assert_eq!(number_of("fig:b"), 1);
    assert_eq!(number_of("fig:c"), 1);
}

#[test]
fn 隣接する_tab_付きフェンスは_1_グループになる() {
    const SRC: &str = concat!(
        "# 見出し\n\n",
        "```rust tab=\"Rust\"\nfn main() {}\n```\n\n",
        "```ts tab=\"TypeScript\"\nconsole.log(1)\n```\n\n",
        "```sh tab=\"Shell\"\necho hi\n```\n",
    );
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("index.md"), SRC).unwrap();

    let site = build_site_model(dir.path(), &[], &MarkdownOptions::default()).unwrap();
    let html = render_body_html(
        &site.pages[0],
        &MarkdownOptions::default(),
        &MermaidOnlyRenderer,
        &NoopUrlRewriter,
    )
    .unwrap();

    assert_eq!(html.matches(r#"<div class="tabs""#).count(), 1, "{html}");
    assert_eq!(html.matches(r#"class="tab-label""#).count(), 3, "{html}");
    // 先頭だけ checked（= 初期表示）
    assert_eq!(html.matches(" checked>").count(), 1, "{html}");
    assert!(html.contains(">Rust</label>"), "{html}");
    assert!(html.contains(">TypeScript</label>"), "{html}");
    // JS ゼロ: script を差し込まない
    assert!(!html.contains("<script"), "{html}");
}

#[test]
fn 段落を挟むとタブグループが切れる() {
    const SRC: &str = concat!(
        "```rust tab=\"A\"\na\n```\n\n",
        "```rust tab=\"B\"\nb\n```\n\n",
        "区切りの段落。\n\n",
        "```rust tab=\"C\"\nc\n```\n\n",
        "```rust tab=\"D\"\nd\n```\n",
    );
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("index.md"), SRC).unwrap();

    let site = build_site_model(dir.path(), &[], &MarkdownOptions::default()).unwrap();
    let html = render_body_html(
        &site.pages[0],
        &MarkdownOptions::default(),
        &MermaidOnlyRenderer,
        &NoopUrlRewriter,
    )
    .unwrap();

    assert_eq!(html.matches(r#"<div class="tabs""#).count(), 2, "{html}");
    // ラジオ名がグループごとに異なる（別グループを巻き添えで切り替えない）
    assert!(html.contains(r#"name="yz-tabs-0""#), "{html}");
    assert!(html.contains(r#"name="yz-tabs-1""#), "{html}");
}

#[test]
fn タブが一枚だけならグループにしない() {
    const SRC: &str = "```rust tab=\"Rust\"\nfn main() {}\n```\n\n段落。\n";
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("index.md"), SRC).unwrap();

    let site = build_site_model(dir.path(), &[], &MarkdownOptions::default()).unwrap();
    let html = render_body_html(
        &site.pages[0],
        &MarkdownOptions::default(),
        &MermaidOnlyRenderer,
        &NoopUrlRewriter,
    )
    .unwrap();

    // 切り替え先が無いのでラベルだけが浮く。通常のコードブロックとして出す
    assert!(!html.contains(r#"class="tabs""#), "{html}");
    assert!(!html.contains("tab-label"), "{html}");
}
