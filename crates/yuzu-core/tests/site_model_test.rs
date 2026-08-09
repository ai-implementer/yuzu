//! build_site_model の統合テスト: draft 除外・order ソート・TOC アンカー同期

use std::fs;
use std::path::Path;

use yuzu_core::{
    MarkdownOptions, NoopCodeBlockRenderer, NoopUrlRewriter, build_site_model, build_source_pages,
};

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
fn include_drafts_なら_draft_も含まれナビにも載る() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "index.md", "# top\n");
    write(dir.path(), "wip.md", "---\ndraft: true\n---\n# wip\n");

    let site = yuzu_core::build_site_model_cached(
        dir.path(),
        &[],
        &MarkdownOptions::default(),
        None,
        true,
    )
    .unwrap();
    assert_eq!(site.pages.len(), 2);
    assert!(site.pages.iter().any(|p| p.frontmatter.draft));
    assert!(
        site.nav.iter().any(|n| n.title == "wip"),
        "draft ページもナビに載る（プレビュー用途）"
    );
}

#[test]
fn build_source_pages_は_draft_を含む() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "index.md", "# top\n");
    write(dir.path(), "wip.md", "---\ndraft: true\n---\n# wip\n");
    write(dir.path(), "_drafts/memo.md", "# memo\n");

    // draft は含むが ignore glob は効く
    let pages = build_source_pages(
        dir.path(),
        &["**/_drafts/**".to_string()],
        &MarkdownOptions::default(),
    )
    .unwrap();
    let rels: Vec<String> = pages
        .iter()
        .map(|p| p.rel.to_string_lossy().into_owned())
        .collect();
    assert_eq!(rels, ["index.md", "wip.md"]);
    assert!(pages[1].frontmatter.draft);
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
        None,
    )
    .unwrap()
    .html;
    for id in ids {
        assert!(
            html.contains(&format!("id=\"{id}\"")),
            "本文 HTML に id=\"{id}\" がない:\n{html}"
        );
    }
}

/// comrak の header_ids は見出し内数式の literal を採番に含める。
/// yuzu 側の collect_text が Math を落とすと TOC・linkcheck のアンカーがずれる（回帰固定）
#[test]
fn 見出し内の数式は_toc_と本文のアンカーが一致する() {
    let dir = tempfile::tempdir().unwrap();
    write(
        dir.path(),
        "index.md",
        "# 概要\n\n## エネルギー $E=mc^2$ の式\n\n本文\n",
    );

    let site = build_site_model(dir.path(), &[], &MarkdownOptions::default()).unwrap();
    let page = &site.pages[0];
    let html = yuzu_core::render_body_html(
        page,
        &MarkdownOptions::default(),
        &NoopCodeBlockRenderer,
        &NoopUrlRewriter,
        None,
    )
    .unwrap()
    .html;

    let toc_id = &page.toc[1].id;
    assert!(
        html.contains(&format!("id=\"{toc_id}\"")),
        "TOC の id=\"{toc_id}\" が本文 HTML にない:\n{html}"
    );
}

#[test]
fn extract_plain_sections_は_h2_h3_で分割しリード文を先頭に置く() {
    let dir = tempfile::tempdir().unwrap();
    write(
        dir.path(),
        "index.md",
        "---\ntitle: セクション\n---\nリード文。\n\n# 大見出し\n\n## 導入\n\n導入の段落。\n\n### 詳細\n\n詳細の段落。\n\n#### 補足\n\n補足の段落。\n\n```rust\nfn secret() {}\n```\n\n## 使い方\n\n使い方その一。\n\n## 使い方\n\n使い方その二。\n",
    );

    let site = build_site_model(dir.path(), &[], &MarkdownOptions::default()).unwrap();
    let page = &site.pages[0];
    let sections =
        yuzu_core::extract_plain_sections(page, &MarkdownOptions::default(), false, None).unwrap();

    // リード文（h1 のテキストは本文として含む）
    assert_eq!(sections[0].anchor, None);
    assert_eq!(sections[0].heading, None);
    assert!(sections[0].body.contains("リード文"), "{:?}", sections[0]);
    assert!(sections[0].body.contains("大見出し"));

    // h2「導入」: 自見出しは body に含まない
    assert_eq!(sections[1].anchor.as_deref(), Some("導入"));
    assert_eq!(sections[1].heading.as_deref(), Some("導入"));
    assert!(sections[1].body.contains("導入の段落"));
    assert!(!sections[1].body.contains("導入\n導入"), "自見出しが混入");

    // h3「詳細」は別セクション。h4「補足」は併合（テキストは残る）
    assert_eq!(sections[2].anchor.as_deref(), Some("詳細"));
    assert!(sections[2].body.contains("詳細の段落"));
    assert!(sections[2].body.contains("補足"), "h4 は併合される");
    assert!(sections[2].body.contains("補足の段落"));
    // コードブロックは除外
    assert!(!sections[2].body.contains("secret"));

    // 重複見出しのアンカーが採番され、本文 HTML の id と一致する（同期の実証）
    assert_eq!(sections[3].anchor.as_deref(), Some("使い方"));
    assert_eq!(sections[4].anchor.as_deref(), Some("使い方-1"));
    let html = yuzu_core::render_body_html(
        page,
        &MarkdownOptions::default(),
        &NoopCodeBlockRenderer,
        &NoopUrlRewriter,
        None,
    )
    .unwrap()
    .html;
    for section in &sections[1..] {
        let id = section.anchor.as_deref().unwrap();
        assert!(
            html.contains(&format!("id=\"{id}\"")),
            "HTML に id=\"{id}\" がない"
        );
    }
}

/// セクション列のどこかに needle を含むか
fn any_body_contains(sections: &[yuzu_core::PlainSection], needle: &str) -> bool {
    sections.iter().any(|s| s.body.contains(needle))
}

#[test]
fn index_code_true_はフェンスコードを含めるが特別言語とインデントは除外する() {
    let dir = tempfile::tempdir().unwrap();
    write(
        dir.path(),
        "index.md",
        concat!(
            "---\ntitle: コード索引\n---\n# 見出し\n\n本文の段落。\n\n",
            "```rust\nfn connectTimeout() {}\n```\n\n",
            "    indented_secret();\n\n",
            "## 図\n\n",
            "```mermaid\nflowchart TD\n  A-->B\n```\n\n",
            "```math\n\\alpha + \\beta\n```\n\n",
            "```openapi\nopenapi: 3.0.3\npaths: {}\n```\n\n",
            "```jsonschema\ndraftSeven: true\n```\n",
        ),
    );

    let site = build_site_model(dir.path(), &[], &MarkdownOptions::default()).unwrap();
    let page = &site.pages[0];

    // index_code=true（既定設定 = 全特別レンダリング有効）: フェンスの通常コードだけ含む
    let on =
        yuzu_core::extract_plain_sections(page, &MarkdownOptions::default(), true, None).unwrap();
    assert!(any_body_contains(&on, "connectTimeout"), "{on:?}");
    // 特別レンダリングされる 4 言語はすべて除外（除外集合の全数検証）
    assert!(!any_body_contains(&on, "flowchart"), "mermaid: {on:?}");
    assert!(!any_body_contains(&on, "alpha"), "math: {on:?}");
    assert!(!any_body_contains(&on, "paths"), "openapi: {on:?}");
    assert!(!any_body_contains(&on, "draftSeven"), "jsonschema: {on:?}");
    // インデントコードブロック（非フェンス）は対象外（ドキュメントの「フェンス」と一致）
    assert!(!any_body_contains(&on, "indented_secret"), "{on:?}");

    // mermaid / math を無効化した構成ではプレーンコード表示になるため索引対象
    let plain_opts = MarkdownOptions {
        math: false,
        mermaid: false,
        ..MarkdownOptions::default()
    };
    let visible = yuzu_core::extract_plain_sections(page, &plain_opts, true, None).unwrap();
    assert!(
        any_body_contains(&visible, "flowchart"),
        "mermaid 無効なら見えるまま索引: {visible:?}"
    );
    assert!(
        any_body_contains(&visible, "alpha"),
        "math 無効なら見えるまま索引: {visible:?}"
    );
    // openapi / jsonschema は無効化フラグが無く常に除外
    assert!(!any_body_contains(&visible, "paths"), "{visible:?}");

    // index_code=false（既定）: コードは一切含まれない
    let off =
        yuzu_core::extract_plain_sections(page, &MarkdownOptions::default(), false, None).unwrap();
    assert!(!any_body_contains(&off, "connectTimeout"), "{off:?}");
    assert!(!any_body_contains(&off, "flowchart"), "{off:?}");
}

#[test]
fn extract_plain_sections_はコードブロックと_html_を除外する() {
    let dir = tempfile::tempdir().unwrap();
    write(
        dir.path(),
        "index.md",
        "---\ntitle: 抽出テスト\n---\n# 見出し\n\n本文の一行目\n続きの行\n\nインライン `code_api` は含む。\n\n```rust\nfn secret_code() {}\n```\n\n```mermaid\ngraph TD;\n```\n\n<div>raw html</div>\n\n- 項目いち\n- 項目に\n",
    );

    let site = build_site_model(dir.path(), &[], &MarkdownOptions::default()).unwrap();
    let sections =
        yuzu_core::extract_plain_sections(&site.pages[0], &MarkdownOptions::default(), false, None)
            .unwrap();
    let lead = &sections[0].body;

    // 含む: 見出し（h1 は本文に併合）・本文（SoftBreak は空白に）・インラインコード・リスト項目
    assert!(lead.contains("見出し"));
    assert!(lead.contains("本文の一行目 続きの行"));
    assert!(lead.contains("code_api"));
    assert!(lead.contains("項目いち"));
    // 含まない: フェンスコード・mermaid ソース・生 HTML・frontmatter
    assert!(!lead.contains("secret_code"));
    assert!(!lead.contains("graph TD"));
    assert!(!lead.contains("raw html"));
    assert!(!lead.contains("抽出テスト")); // frontmatter の title は本文ではない
    // ブロック区切りで改行が入る（トークナイズの文脈を切る）
    assert!(lead.lines().count() >= 4, "lead:\n{lead}");
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

/// `content/` を作る前の `yuzu build` を落とさない（0 ページで成功）
#[test]
fn content_が存在しなければ空のサイトになる() {
    let dir = tempfile::tempdir().unwrap();
    let missing = dir.path().join("content");

    let site = build_site_model(&missing, &[], &MarkdownOptions::default()).unwrap();
    assert!(site.pages.is_empty());
}

/// 走査エラーを握りつぶすと「ページが消えたのにビルド成功」になる
#[cfg(unix)]
#[test]
fn 読めないディレクトリがあるとサイトモデル構築はエラーになる() {
    use std::os::unix::fs::PermissionsExt;

    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "index.md", "# top\n");
    write(dir.path(), "secret/page.md", "# 隠しページ\n");

    let secret = dir.path().join("secret");
    fs::set_permissions(&secret, fs::Permissions::from_mode(0o000)).unwrap();
    // root 実行では権限が効かないので、実際に読めないことを確かめてから判定する
    let unreadable = fs::read_dir(&secret).is_err();
    let result = build_site_model(dir.path(), &[], &MarkdownOptions::default());
    // tempdir の後片付けができるように戻す
    fs::set_permissions(&secret, fs::Permissions::from_mode(0o755)).unwrap();

    if unreadable {
        assert!(result.is_err(), "無言でスキップせずエラーにする");
    }
}

#[test]
fn 断片は_index_code_無効でも索引される() {
    let root = tempfile::tempdir().unwrap();
    fs::create_dir_all(root.path().join("content")).unwrap();
    fs::create_dir_all(root.path().join("snippets")).unwrap();
    fs::write(
        root.path().join("content/index.md"),
        "# ページ\n\n本文。\n\n```include file=\"snippets/note.md\"\n```\n",
    )
    .unwrap();
    fs::write(
        root.path().join("snippets/note.md"),
        "断片の**注意書き**と[リンク](/guide/)です。\n",
    )
    .unwrap();

    let site = build_site_model(
        &root.path().join("content"),
        &[],
        &MarkdownOptions::default(),
    )
    .unwrap();
    // index_code = false（既定）のまま断片が索引される
    let sections = yuzu_core::extract_plain_sections(
        &site.pages[0],
        &MarkdownOptions::default(),
        false,
        Some(root.path()),
    )
    .unwrap();
    let lead = &sections[0].body;
    assert!(
        lead.contains("断片の注意書きとリンクです。"),
        "記法が落ちる: {lead}"
    );
    assert!(!lead.contains("**"), "{lead}");

    // 読めない断片は黙ってスキップ（本文の索引は続く）
    fs::remove_file(root.path().join("snippets/note.md")).unwrap();
    let sections = yuzu_core::extract_plain_sections(
        &site.pages[0],
        &MarkdownOptions::default(),
        false,
        Some(root.path()),
    )
    .unwrap();
    assert!(sections[0].body.contains("本文。"));
    assert!(!sections[0].body.contains("注意書き"));
}

/// 用語辞書つきの `MarkdownOptions`
fn glossary_opts(pairs: &[(&str, &str)], page: &str) -> MarkdownOptions {
    MarkdownOptions {
        glossary: yuzu_core::GlossaryOptions {
            terms: pairs
                .iter()
                .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
                .collect(),
            page: page.to_string(),
            page_title: "用語集".to_string(),
            ..yuzu_core::GlossaryOptions::default()
        },
        ..MarkdownOptions::default()
    }
}

#[test]
fn 用語辞書が空なら用語集ページは作られない() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "index.md", "# top\n");

    // 既定（辞書が空）では従来どおり = 既存プロジェクトの出力が 1 バイトも変わらない
    let site = build_site_model(dir.path(), &[], &MarkdownOptions::default()).unwrap();
    assert_eq!(site.pages.len(), 1);
    assert!(site.pages.iter().all(|p| !p.is_generated()));

    // 辞書があっても page が空なら作らない
    let opts = glossary_opts(&[("SSG", "Static Site Generator")], "");
    let site = build_site_model(dir.path(), &[], &opts).unwrap();
    assert_eq!(site.pages.len(), 1);
}

#[test]
fn 用語集ページが合成されナビにも載る() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "index.md", "# top\n");
    let opts = glossary_opts(
        &[("SSG", "Static Site Generator"), ("TOC", "目次")],
        "glossary",
    );

    let site = build_site_model(dir.path(), &[], &opts).unwrap();
    let page = site.pages.iter().find(|p| p.is_generated()).unwrap();
    assert_eq!(page.route, "glossary/");
    assert_eq!(page.rel, Path::new("glossary.md"));
    assert_eq!(page.title, "用語集");
    // 用語ごとに h2 = アンカー（`glossary.md#ssg` へのリンクが解決できる）
    let ids: Vec<&str> = page.toc.iter().map(|t| t.id.as_str()).collect();
    assert_eq!(ids, ["用語集", "ssg", "toc"]);
    // サイドバーに出る
    assert!(
        site.nav
            .iter()
            .any(|n| n.route.as_deref() == Some("glossary/")),
        "{:?}",
        site.nav
    );
    // build_source_pages（fmt / lint / check の入力）にも載る = リンク検査の
    // 有効ターゲットになる
    let pages = build_source_pages(dir.path(), &[], &opts).unwrap();
    assert!(
        pages
            .iter()
            .any(|p| p.is_generated() && p.route == "glossary/")
    );
}

#[test]
fn 用語集ページは_fmt_の正規形と一致する() {
    // `yuzu check` の整形差分でも llms-full.txt でも食い違わないことの保証
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "index.md", "# top\n");
    let opts = glossary_opts(
        &[
            ("SSG", "Static Site Generator。静的サイト生成"),
            ("TOC", "目次"),
        ],
        "glossary",
    );

    let site = build_site_model(dir.path(), &[], &opts).unwrap();
    let page = site.pages.iter().find(|p| p.is_generated()).unwrap();
    assert_eq!(
        yuzu_core::format_document(page, &opts).unwrap(),
        page.source,
        "生成 Markdown が fmt の正規形でない"
    );
    // 決定的（BTreeMap のキー順）
    assert_eq!(
        page.source,
        "# 用語集\n\n## SSG\n\nStatic Site Generator。静的サイト生成\n\n## TOC\n\n目次\n"
    );
}

#[test]
fn 用語集ページの_route_はディレクトリ配下にも置ける() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "index.md", "# top\n");
    write(dir.path(), "reference/index.md", "# リファレンス\n");
    let opts = glossary_opts(&[("SSG", "Static Site Generator")], "reference/glossary");

    let site = build_site_model(dir.path(), &[], &opts).unwrap();
    let page = site.pages.iter().find(|p| p.is_generated()).unwrap();
    assert_eq!(page.route, "reference/glossary/");
    // nav も rel のパス要素からツリーを組むので reference の配下に入る
    let reference = site
        .nav
        .iter()
        .find(|n| n.route.as_deref() == Some("reference/"))
        .unwrap();
    assert!(
        reference
            .children
            .iter()
            .any(|c| c.route.as_deref() == Some("reference/glossary/")),
        "{reference:?}"
    );
}

#[test]
fn 不正な_page_値では用語集ページを作らない() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "index.md", "# top\n");
    // content_dir の外へ出る値・空セグメントは黙って無効にする
    // （設定の書き間違いでビルドを止めず、かつルート外へは絶対に書かない）
    for page in [
        "../outside",
        "a/../../x",
        "a//b",
        ".",
        "..",
        "\\abs",
        "C:/x",
    ] {
        let opts = glossary_opts(&[("SSG", "Static Site Generator")], page);
        let site = build_site_model(dir.path(), &[], &opts).unwrap();
        assert!(
            site.pages.iter().all(|p| !p.is_generated()),
            "page={page} で合成されてしまう"
        );
    }
}

#[test]
fn page_値の前後のスラッシュは吸収する() {
    // frontmatter の aliases と同じ正規化（`/x/` も `x` も同じ route になる）
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "index.md", "# top\n");
    for page in ["glossary", "/glossary", "glossary/", "/glossary/"] {
        let opts = glossary_opts(&[("SSG", "Static Site Generator")], page);
        let site = build_site_model(dir.path(), &[], &opts).unwrap();
        let generated = site.pages.iter().find(|p| p.is_generated()).unwrap();
        assert_eq!(generated.route, "glossary/", "page={page}");
    }
}

// ---- 検索結果ページ（search.page）の合成 ----

fn search_page_opts(page: &str, page_title: &str) -> MarkdownOptions {
    MarkdownOptions {
        search_page: yuzu_core::SearchPageOptions {
            page: page.to_string(),
            page_title: page_title.to_string(),
        },
        ..MarkdownOptions::default()
    }
}

#[test]
fn 既定では検索結果ページは作られない() {
    // 既存プロジェクトの `content/search.md` と衝突してビルド不能にならないよう
    // 明示オプトイン（route 空 = 無効）が既定
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "index.md", "# top\n");
    let site = build_site_model(dir.path(), &[], &search_page_opts("", "検索")).unwrap();
    assert_eq!(site.pages.len(), 1);
    assert!(site.pages.iter().all(|p| !p.is_generated()));
}

#[test]
fn 検索結果ページが合成されナビには載らない() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "index.md", "# top\n");
    let opts = search_page_opts("search", "検索");

    let site = build_site_model(dir.path(), &[], &opts).unwrap();
    let page = site.pages.iter().find(|p| p.is_generated()).unwrap();
    assert_eq!(page.generated, Some(yuzu_core::GeneratedKind::Search));
    assert_eq!(page.route, "search/");
    assert_eq!(page.rel, Path::new("search.md"));
    assert_eq!(page.title, "検索");
    // JS 前提の機能ページなのでサイドバーには出さない
    assert!(
        !site
            .nav
            .iter()
            .any(|n| n.route.as_deref() == Some("search/")),
        "{:?}",
        site.nav
    );
    // llms.txt / llms-full.txt には収録しない（既存の frontmatter フィルタに乗せる）
    assert!(!page.frontmatter.llms);
    // 集約の除外ヘルパ（検索索引・sitemap・ページ単位 .md）
    assert!(!page.in_nav());
    assert!(!page.in_search_index());
    assert!(!page.in_sitemap());
    assert!(!page.emits_page_md());
    // fmt の正規形（`yuzu check` の整形差分と食い違わない）
    assert_eq!(
        yuzu_core::format_document(page, &opts).unwrap(),
        page.source
    );
    // build_source_pages（fmt / lint / check の入力）にも載る = リンク検査の
    // 有効ターゲット・route 衝突検査の対象になる
    let pages = build_source_pages(dir.path(), &[], &opts).unwrap();
    assert!(
        pages
            .iter()
            .any(|p| p.is_generated() && p.route == "search/")
    );
}

#[test]
fn 検索結果ページのタイトルが空ならファイル名から補う() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "index.md", "# top\n");
    let site = build_site_model(dir.path(), &[], &search_page_opts("search", "")).unwrap();
    let page = site.pages.iter().find(|p| p.is_generated()).unwrap();
    assert_eq!(page.title, "search");
}

#[test]
fn 不正な_search_page_値では検索結果ページを作らない() {
    // 字句検証は用語集と同じ synth_page_rel を共有する（詳細ケースはそちらのテスト）
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "index.md", "# top\n");
    for page in ["../outside", "a//b", "..", "\\abs", "C:/x"] {
        let site = build_site_model(dir.path(), &[], &search_page_opts(page, "検索")).unwrap();
        assert!(
            site.pages.iter().all(|p| !p.is_generated()),
            "page={page} で合成されてしまう"
        );
    }
}

#[test]
fn 用語集と検索結果ページは共存できる() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "index.md", "# top\n");
    let opts = MarkdownOptions {
        search_page: yuzu_core::SearchPageOptions {
            page: "search".to_string(),
            page_title: "検索".to_string(),
        },
        ..glossary_opts(&[("SSG", "Static Site Generator")], "glossary")
    };
    let site = build_site_model(dir.path(), &[], &opts).unwrap();
    let kinds: Vec<_> = site.pages.iter().filter_map(|p| p.generated).collect();
    // 順序固定（routesKey のバイト安定のため用語集 → 検索）
    assert_eq!(
        kinds,
        [
            yuzu_core::GeneratedKind::Glossary,
            yuzu_core::GeneratedKind::Search
        ]
    );
    // 用語集は nav に載り、検索結果ページは載らない
    assert!(
        site.nav
            .iter()
            .any(|n| n.route.as_deref() == Some("glossary/"))
    );
    assert!(
        !site
            .nav
            .iter()
            .any(|n| n.route.as_deref() == Some("search/"))
    );
}

#[test]
fn 用語集ページは_lint_の対象外() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "index.md", "# top\n");
    // 説明文に全角英数字（lint 対象の文字）を入れても診断が出ないこと
    let opts = glossary_opts(
        &[("SSG", "Ｓｔａｔｉｃ Ｓｉｔｅ Ｇｅｎｅｒａｔｏｒ")],
        "glossary",
    );
    let pages = build_source_pages(dir.path(), &[], &opts).unwrap();
    let lint_opts = yuzu_core::LintOptions::default();

    let generated = pages.iter().find(|p| p.is_generated()).unwrap();
    assert!(
        yuzu_core::lint_page(generated, &opts, &lint_opts)
            .unwrap()
            .is_empty()
    );
    assert!(yuzu_core::lint_project(&pages, &opts).unwrap().is_empty());
}
