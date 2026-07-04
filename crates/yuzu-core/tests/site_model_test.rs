//! build_site_model の統合テスト: draft 除外・order ソート・TOC アンカー同期

use std::fs;
use std::path::Path;

use yuzu_core::{MarkdownOptions, NoopCodeBlockRenderer, NoopUrlRewriter, build_site_model};

fn write(dir: &Path, rel: &str, content: &str) {
    let path = dir.join(rel);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}

#[test]
fn draft_は除外される() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "index.md", "# top\n");
    write(dir.path(), "wip.md", "---\ndraft: true\n---\n# wip\n");

    let site = build_site_model(dir.path(), &[], &MarkdownOptions::default()).unwrap();
    assert_eq!(site.pages.len(), 1);
    assert_eq!(site.pages[0].route, "");
}

#[test]
fn ignore_glob_で除外できる() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "index.md", "# top\n");
    write(dir.path(), "_drafts/memo.md", "# memo\n");

    let site = build_site_model(
        dir.path(),
        &["**/_drafts/**".to_string()],
        &MarkdownOptions::default(),
    )
    .unwrap();
    assert_eq!(site.pages.len(), 1);
}

#[test]
fn nav_は_order_昇順で未指定はファイル名順の最後尾() {
    let dir = tempfile::tempdir().unwrap();
    write(
        dir.path(),
        "index.md",
        "---\ntitle: ホーム\norder: 1\n---\n",
    );
    write(dir.path(), "zebra.md", "---\ntitle: Zebra\norder: 2\n---\n");
    write(dir.path(), "alpha.md", "---\ntitle: Alpha\n---\n"); // order 未指定
    write(dir.path(), "beta.md", "---\ntitle: Beta\n---\n"); // order 未指定

    let site = build_site_model(dir.path(), &[], &MarkdownOptions::default()).unwrap();
    let titles: Vec<&str> = site.nav.iter().map(|n| n.title.as_str()).collect();
    // order 付き（ホーム=1, Zebra=2）→ 未指定はファイル名順（alpha, beta）
    assert_eq!(titles, ["ホーム", "Zebra", "Alpha", "Beta"]);
}

#[test]
fn ディレクトリは_index_md_の表示名とリンクを持つ() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "index.md", "---\ntitle: ホーム\n---\n");
    write(
        dir.path(),
        "guide/index.md",
        "---\ntitle: ガイド\norder: 1\n---\n",
    );
    write(
        dir.path(),
        "guide/getting-started.md",
        "---\ntitle: はじめに\n---\n",
    );

    let site = build_site_model(dir.path(), &[], &MarkdownOptions::default()).unwrap();
    let guide = site
        .nav
        .iter()
        .find(|n| n.title == "ガイド")
        .expect("guide ディレクトリのノードがある");
    assert_eq!(guide.route.as_deref(), Some("guide/"));
    // index.md 自身は子に重複して現れない
    assert_eq!(guide.children.len(), 1);
    assert_eq!(guide.children[0].title, "はじめに");
    assert_eq!(
        guide.children[0].route.as_deref(),
        Some("guide/getting-started/")
    );
}

#[test]
fn タイトルは_frontmatter_h1_ファイル名の順で決まる() {
    let dir = tempfile::tempdir().unwrap();
    write(
        dir.path(),
        "a.md",
        "---\ntitle: FM タイトル\n---\n# H1 タイトル\n",
    );
    write(dir.path(), "b.md", "# H1 タイトル\n");
    write(dir.path(), "c.md", "本文のみ\n");

    let site = build_site_model(dir.path(), &[], &MarkdownOptions::default()).unwrap();
    let titles: Vec<&str> = site.pages.iter().map(|p| p.title.as_str()).collect();
    assert_eq!(titles, ["FM タイトル", "H1 タイトル", "c"]);
}

/// 同名見出しの連発でも TOC の ID と本文 HTML の id 属性が一致すること
/// （comrak header_ids 拡張との採番同期の回帰テスト）
#[test]
fn 重複見出しの_toc_id_が本文アンカーと一致する() {
    let dir = tempfile::tempdir().unwrap();
    write(
        dir.path(),
        "index.md",
        "# 概要\n\n## 使い方\n\n本文\n\n## 使い方\n\n本文\n\n## 使い方\n\n本文\n",
    );

    let site = build_site_model(dir.path(), &[], &MarkdownOptions::default()).unwrap();
    let page = &site.pages[0];

    let ids: Vec<&str> = page.toc.iter().map(|t| t.id.as_str()).collect();
    assert_eq!(ids, ["概要", "使い方", "使い方-1", "使い方-2"]);

    let html = yuzu_core::render_body_html(
        page,
        &MarkdownOptions::default(),
        &NoopCodeBlockRenderer,
        &NoopUrlRewriter,
    )
    .unwrap();
    for id in ids {
        assert!(
            html.contains(&format!("id=\"{id}\"")),
            "本文 HTML に id=\"{id}\" がない:\n{html}"
        );
    }
}

#[test]
fn extract_plain_text_はコードブロックと_html_を除外する() {
    let dir = tempfile::tempdir().unwrap();
    write(
        dir.path(),
        "index.md",
        "---\ntitle: 抽出テスト\n---\n# 見出し\n\n本文の一行目\n続きの行\n\nインライン `code_api` は含む。\n\n```rust\nfn secret_code() {}\n```\n\n```mermaid\ngraph TD;\n```\n\n<div>raw html</div>\n\n- 項目いち\n- 項目に\n",
    );

    let site = build_site_model(dir.path(), &[], &MarkdownOptions::default()).unwrap();
    let text = yuzu_core::extract_plain_text(&site.pages[0], &MarkdownOptions::default()).unwrap();

    // 含む: 見出し・本文（SoftBreak は空白に）・インラインコード・リスト項目
    assert!(text.contains("見出し"));
    assert!(text.contains("本文の一行目 続きの行"));
    assert!(text.contains("code_api"));
    assert!(text.contains("項目いち"));
    // 含まない: フェンスコード・mermaid ソース・生 HTML・frontmatter
    assert!(!text.contains("secret_code"));
    assert!(!text.contains("graph TD"));
    assert!(!text.contains("raw html"));
    assert!(!text.contains("抽出テスト")); // frontmatter の title は本文ではない
    // ブロック区切りで改行が入る
    assert!(text.lines().count() >= 4, "text:\n{text}");
}

#[test]
fn toc_は_sourcepos_を持つ() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "index.md", "# 一行目\n\n本文\n\n## 五行目\n");

    let site = build_site_model(dir.path(), &[], &MarkdownOptions::default()).unwrap();
    let toc = &site.pages[0].toc;
    assert_eq!(toc[0].span.start_line, 1);
    assert_eq!(toc[1].span.start_line, 5);
}
