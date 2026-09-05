//! フィクスチャプロジェクトをフルビルドし、生成 HTML をスナップショット検証する。
//!
//! 注意: ハイライト済み HTML は syntect のバージョン更新で変わり得る
//! （その場合は `cargo insta review` で差分確認のうえ更新する）。
//! `syntect.css` 自体はスナップショット対象にしない。

use std::fs;
use std::path::Path;

use yuzu_core::MarkdownOptions;
use yuzu_render::{LiveReloadMode, RenderParams, render_site};

/// フィクスチャを tempdir へコピーする（dist をリポジトリ内に作らないため）
fn copy_tree(src: &Path, dest: &Path) {
    for entry in walkdir_files(src) {
        let rel = entry.strip_prefix(src).unwrap();
        let target = dest.join(rel);
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::copy(&entry, target).unwrap();
    }
}

fn walkdir_files(dir: &Path) -> Vec<std::path::PathBuf> {
    let mut files = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in fs::read_dir(d).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                stack.push(p);
            } else {
                files.push(p);
            }
        }
    }
    files.sort();
    files
}

fn build_fixture(live_reload: LiveReloadMode) -> tempfile::TempDir {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/sample-docs");
    let dir = tempfile::tempdir().unwrap();
    copy_tree(&fixture, dir.path());

    let rc = yuzu_config::load(dir.path()).unwrap();
    let site = yuzu_core::build_site_model(
        &rc.content_dir,
        &rc.config.input.ignore,
        &MarkdownOptions {
            gfm: rc.config.markdown.gfm,
            math: rc.config.markdown.math.enabled,
            mermaid: rc.config.markdown.mermaid.enabled,
            // 設定由来の写像は cli と同じ 1 実装を通す（用語集の配線もここで効く）
            glossary: yuzu_render::glossary_options(&rc.config),
            search_page: yuzu_render::search_page_options(&rc.config),
            ..MarkdownOptions::default()
        },
    )
    .unwrap();
    render_site(&RenderParams {
        config: &rc,
        site: &site,
        live_reload,
        ctx: yuzu_render::RenderCtx::default(),
        git_dates: None,
    })
    .unwrap();
    dir
}

#[test]
fn フルビルドのスナップショット() {
    let dir = build_fixture(LiveReloadMode::None);
    let dist = dir.path().join("dist");

    let index = fs::read_to_string(dist.join("index.html")).unwrap();
    let guide = fs::read_to_string(dist.join("guide/getting-started/index.html")).unwrap();

    insta::assert_snapshot!("index_html", index);
    insta::assert_snapshot!("guide_html", guide);
}

#[test]
fn 生成物一式が揃っている() {
    let dir = build_fixture(LiveReloadMode::None);
    let dist = dir.path().join("dist");

    // syntect.css はバージョン更新で差分が出やすいので存在と中身だけ確認
    let syntect_css = fs::read_to_string(dist.join("_assets/css/syntect.css")).unwrap();
    assert!(syntect_css.contains("yz-"));
    assert!(syntect_css.contains("html[data-theme=\"dark\"]"));
    // ダーク配色は画面専用（@media screen）＝印刷は常にライト（Phase 55）
    assert!(syntect_css.contains("@media screen"));

    // テーマアセット・public パススルー・build_id
    assert!(dist.join("_assets/css/theme.css").is_file());
    assert!(dist.join("_assets/js/theme.js").is_file());
    assert!(dist.join("_assets/vendor/mermaid.min.js").is_file());
    assert!(dist.join("_assets/vendor/katex/katex.min.js").is_file());
    assert!(dist.join("_assets/vendor/katex/katex.min.css").is_file());
    assert!(
        dist.join("_assets/vendor/katex/fonts/KaTeX_Main-Regular.woff2")
            .is_file()
    );
    assert!(dist.join("images/logo.svg").is_file());
    assert!(dist.join("__yuzu/build_id").is_file());
    assert!(dist.join("llms.txt").is_file());
    assert!(dist.join("llms-full.txt").is_file());
    assert!(dist.join("404.html").is_file());

    // 通常ビルドにはオートリフレッシュを注入しない
    let index = fs::read_to_string(dist.join("index.html")).unwrap();
    assert!(!index.contains("autorefresh.js"));
}

#[test]
fn エイリアスはリダイレクト_html_になり_base_url_に追随する() {
    let dir = build_fixture(LiveReloadMode::None);
    let redirect =
        fs::read_to_string(dir.path().join("dist/guide/first-steps/index.html")).unwrap();
    assert!(
        redirect.contains(r#"content="0; url=/docs/guide/getting-started/""#),
        "meta refresh（baseUrl 付き）: {redirect}"
    );
    assert!(redirect.contains(r#"rel="canonical" href="/docs/guide/getting-started/""#));
    assert!(redirect.contains(r#"name="robots" content="noindex""#));
    assert!(redirect.contains(r#"location.replace("/docs/guide/getting-started/")"#));
    assert!(
        redirect.contains("はじめに"),
        "リンクテキストは移動先タイトル"
    );
}

/// `\` を含むファイル名は出力パスにできない（`output::write_under` が拒否する）ので
/// 書き出す前に中断する。Windows では `\` がパス区切りなのでこの形は作れない
#[cfg(unix)]
#[test]
fn 出力パスにできないファイル名はビルドを中断する() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/sample-docs");
    let dir = tempfile::tempdir().unwrap();
    copy_tree(&fixture, dir.path());
    fs::write(
        dir.path().join("content").join("a\\b.md"),
        "---\ntitle: 危険\n---\n\n# 危険\n",
    )
    .unwrap();

    let rc = yuzu_config::load(dir.path()).unwrap();
    let site = yuzu_core::build_site_model(
        &rc.content_dir,
        &rc.config.input.ignore,
        &MarkdownOptions::default(),
    )
    .unwrap();
    let err = render_site(&RenderParams {
        config: &rc,
        site: &site,
        live_reload: LiveReloadMode::None,
        ctx: yuzu_render::RenderCtx::default(),
        git_dates: None,
    })
    .unwrap_err();
    assert!(
        err.to_string().contains("出力パス"),
        "ファイル名起因のエラー: {err}"
    );
    assert!(
        !dir.path().join("dist/index.html").exists(),
        "書き出し前に中断される"
    );
}

/// URL で意味を持つ文字（`#` `%` 空白・非 ASCII）を含むファイル名はビルドを止めず、
/// route → URL の変換点で一律にパーセントエンコードされる。ディスク上のパスは生の
/// ファイル名のままなので、サーバがデコードした要求パスと一致する。
/// 本文リンク・ナビ・ページ単位 .md・llms.txt・sitemap・編集リンクが同じ表記になること
#[test]
fn url_で意味を持つ文字を含むファイル名はエンコードして配信される() {
    let dir = build_fixture_with(|root| {
        let content = root.join("content");
        fs::create_dir_all(content.join("設計")).unwrap();
        fs::write(
            content.join("設計/概 要#1.md"),
            "---\ntitle: 概要\n---\n\n# 概要\n\n![図](<図 1.png>)\n\n![図2](%E5%9B%B3%201.png)\n",
        )
        .unwrap();
        fs::write(content.join("設計/図 1.png"), b"png").unwrap();
        fs::write(
            content.join("a%23b.md"),
            "---\ntitle: パーセント\n---\n\n# パーセント\n",
        )
        .unwrap();
        // `#` を含むファイル名は `%23` と書かないと `#` 以降がフラグメントになる
        // （URL 構文上の制約。`<>` で囲んでも同じ）。空白は `<>` 記法・`%20` 記法・
        // 生のファイル名の 3 通りとも同じページへ解決される
        fs::write(
            content.join("links.md"),
            "---\ntitle: リンク\n---\n\n# リンク\n\n\
             [済](%E8%A8%AD%E8%A8%88/%E6%A6%82%20%E8%A6%81%231.md#概要)\n\n\
             [半](設計/概%20要%231.md)\n\n\
             [角](<設計/概 要%231.md>)\n\n\
             [percent](a%2523b.md)\n",
        )
        .unwrap();
        let toml = root.join("yuzu.toml");
        let src = fs::read_to_string(&toml).unwrap();
        fs::write(
            &toml,
            src.replace("\"/docs/\"", "\"https://example.com/docs/\"")
                + "\n[git]\nedit_url = \"https://github.com/me/docs/edit/main/content/{path}\"\n",
        )
        .unwrap();
    });
    let dist = dir.path().join("dist");
    const ENC: &str = "%E8%A8%AD%E8%A8%88/%E6%A6%82%20%E8%A6%81%231";

    // ディスクは生のファイル名
    assert!(dist.join("設計/概 要#1/index.html").is_file());
    assert!(dist.join("設計/概 要#1.md").is_file());
    assert!(dist.join("設計/図 1.png").is_file());
    assert!(dist.join("a%23b/index.html").is_file());

    // 本文リンク: `%20` 記法・`<>` 記法の 2 本＋ナビ＋前後ページリンクの計 4 箇所が
    // 同じ URL になる（フル エンコード済みの `[済]` はフラグメント付きで別カウント）
    let links = fs::read_to_string(dist.join("links/index.html")).unwrap();
    assert_eq!(
        links
            .matches(&format!("href=\"https://example.com/docs/{ENC}/\""))
            .count(),
        4,
        "{links}"
    );
    // suffix は yuzu ではエンコードしない（comrak の escape_href が非 ASCII を
    // 従来どおり `%XX` にする。ブラウザはデコードして id と照合する）
    assert!(
        links.contains(&format!(
            "href=\"https://example.com/docs/{ENC}/#%E6%A6%82%E8%A6%81\""
        )),
        "{links}"
    );
    assert!(
        links.contains("href=\"https://example.com/docs/a%2523b/\""),
        "`%` は `%25` へ（二重にならない）: {links}"
    );
    // ナビ（サイドバー）も同じ表記
    let index = fs::read_to_string(dist.join("index.html")).unwrap();
    assert!(
        index.contains(&format!("href=\"https://example.com/docs/{ENC}/\"")),
        "{index}"
    );
    assert!(index.contains("href=\"https://example.com/docs/a%2523b/\""));

    // ページ自身: ページ単位 .md の URL・同伴アセット・編集リンク
    let page = fs::read_to_string(dist.join("設計/概 要#1/index.html")).unwrap();
    assert!(
        page.contains(&format!(
            "data-md-url=\"https://example.com/docs/{ENC}.md\""
        )),
        "{page}"
    );
    // 同伴アセット: `<図 1.png>` 記法と `%20` 記法のどちらも同じ URL になる
    assert_eq!(
        page.matches("src=\"https://example.com/docs/%E8%A8%AD%E8%A8%88/%E5%9B%B3%201.png\"")
            .count(),
        2,
        "{page}"
    );
    assert!(
        page.contains(&format!(
            "href=\"https://github.com/me/docs/edit/main/content/{ENC}.md\""
        )),
        "{page}"
    );

    // llms.txt / llms-full.txt / sitemap.xml も同じ変換点を通る
    let llms = fs::read_to_string(dist.join("llms.txt")).unwrap();
    assert!(
        llms.contains(&format!("](https://example.com/docs/{ENC}.md)")),
        "{llms}"
    );
    assert!(
        llms.contains("](https://example.com/docs/a%2523b.md)"),
        "{llms}"
    );
    let full = fs::read_to_string(dist.join("llms-full.txt")).unwrap();
    assert!(
        full.contains(&format!("URL: https://example.com/docs/{ENC}/\n")),
        "{full}"
    );
    let sitemap = fs::read_to_string(dist.join("sitemap.xml")).unwrap();
    assert!(
        sitemap.contains(&format!("<loc>https://example.com/docs/{ENC}/</loc>")),
        "{sitemap}"
    );
    assert!(
        sitemap.contains("<loc>https://example.com/docs/a%2523b/</loc>"),
        "{sitemap}"
    );
}

/// 設定由来の URL（route ではないのでビルドは通る）はテンプレートで
/// エスケープされ、属性や `<script>` の文脈を壊さない
#[test]
fn 設定由来の_url_に危険な文字があってもエスケープされる() {
    let dir = build_fixture_with(|root| {
        let path = root.join("yuzu.toml");
        let src = fs::read_to_string(&path).unwrap();
        fs::write(
            &path,
            src.replace(
                r#"lang = "ja""#,
                concat!("lang = \"ja\"\n", r#"logo = "/images/a\"b.svg""#),
            ),
        )
        .unwrap();
    });

    let index = fs::read_to_string(dir.path().join("dist/index.html")).unwrap();
    assert!(
        index.contains(r#"src="/docs/images/a%22b.svg""#),
        "属性を抜けずエンコードされる: {index}"
    );
}

/// 出力先がリンクだと書き込みが全部リンク先へ素通りする。
/// **clean 無効でも**中断すること = 削除経路に頼った検証では塞げない穴の回帰テスト
#[cfg(unix)]
#[test]
fn 出力先がシンボリックリンクならビルドを中断する() {
    for clean in [true, false] {
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/sample-docs");
        let dir = tempfile::tempdir().unwrap();
        copy_tree(&fixture, dir.path());

        let outside = dir.path().join("outside");
        fs::create_dir_all(&outside).unwrap();
        std::os::unix::fs::symlink(&outside, dir.path().join("dist")).unwrap();

        let path = dir.path().join("yuzu.toml");
        let src = fs::read_to_string(&path).unwrap();
        fs::write(
            &path,
            src.replace(
                "base_url = \"/docs/\"",
                &format!("base_url = \"/docs/\"\n\n[output]\nclean = {clean}"),
            ),
        )
        .unwrap();

        let rc = yuzu_config::load(dir.path()).unwrap();
        let site = yuzu_core::build_site_model(
            &rc.content_dir,
            &rc.config.input.ignore,
            &MarkdownOptions::default(),
        )
        .unwrap();
        let err = render_site(&RenderParams {
            config: &rc,
            site: &site,
            live_reload: LiveReloadMode::None,
            ctx: yuzu_render::RenderCtx::default(),
            git_dates: None,
        })
        .unwrap_err();

        assert!(
            err.to_string().contains("シンボリックリンク"),
            "clean={clean}: リンク起因のエラー: {err}"
        );
        assert!(
            !outside.join("index.html").exists(),
            "clean={clean}: リンク先へ書き込まない"
        );
    }
}

#[test]
fn エイリアス衝突はビルドを中断する() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/sample-docs");
    let dir = tempfile::tempdir().unwrap();
    copy_tree(&fixture, dir.path());
    // 自ページの route と衝突するエイリアスへ書き換える
    let page = dir.path().join("content/guide/getting-started.md");
    let source = fs::read_to_string(&page).unwrap();
    fs::write(
        &page,
        source.replace("guide/first-steps/", "guide/getting-started/"),
    )
    .unwrap();

    let rc = yuzu_config::load(dir.path()).unwrap();
    let site = yuzu_core::build_site_model(
        &rc.content_dir,
        &rc.config.input.ignore,
        &MarkdownOptions::default(),
    )
    .unwrap();
    let err = render_site(&RenderParams {
        config: &rc,
        site: &site,
        live_reload: LiveReloadMode::None,
        ctx: yuzu_render::RenderCtx::default(),
        git_dates: None,
    })
    .unwrap_err();
    assert!(
        err.to_string().contains("aliases"),
        "エイリアス起因のエラー: {err}"
    );
    assert!(
        !dir.path()
            .join("dist/guide/getting-started/index.html")
            .exists(),
        "書き出し前に中断される"
    );
}

/// `highlight.enabled: false` は着色だけを止める設定で、
/// `file=` の引用まで消してはいけない（従来は空の `<pre><code>` になっていた）
#[test]
fn ハイライト無効でも_file_引用が本文に出る() {
    let dir = build_fixture_with(|root| {
        fs::write(root.join("snippet.rs"), "fn 引用対象() {}\n").unwrap();
        let path = root.join("yuzu.toml");
        let src = fs::read_to_string(&path).unwrap();
        fs::write(
            &path,
            src.replace(
                "base_url = \"/docs/\"",
                "base_url = \"/docs/\"\n\n[markdown.highlight]\nenabled = false",
            ),
        )
        .unwrap();

        let index = root.join("content/index.md");
        let source = fs::read_to_string(&index).unwrap();
        fs::write(
            &index,
            format!("{source}\n```rust file=\"snippet.rs\"\n```\n"),
        )
        .unwrap();
    });

    let html = fs::read_to_string(dir.path().join("dist/index.html")).unwrap();
    assert!(
        html.contains("fn 引用対象() {}"),
        "引用が展開される: {html}"
    );
    assert!(
        html.contains("<figcaption>snippet.rs</figcaption>"),
        "{html}"
    );
    assert!(!html.contains("yz-"), "着色クラスは付かない");
    // base.jinja が無条件で <link> するので、空でもファイルは要る
    assert!(dir.path().join("dist/_assets/css/syntect.css").is_file());
}

#[test]
fn route_衝突はビルドを中断する() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/sample-docs");
    let dir = tempfile::tempdir().unwrap();
    copy_tree(&fixture, dir.path());
    // guide/index.md と guide.md はどちらも route `guide/` になる
    for rel in ["content/guide/index.md", "content/guide.md"] {
        fs::write(dir.path().join(rel), "---\ntitle: 重複\n---\n\n# 重複\n").unwrap();
    }

    let rc = yuzu_config::load(dir.path()).unwrap();
    let site = yuzu_core::build_site_model(
        &rc.content_dir,
        &rc.config.input.ignore,
        &MarkdownOptions::default(),
    )
    .unwrap();
    let err = render_site(&RenderParams {
        config: &rc,
        site: &site,
        live_reload: LiveReloadMode::None,
        ctx: yuzu_render::RenderCtx::default(),
        git_dates: None,
    })
    .unwrap_err();
    assert!(
        err.to_string().contains("同じ URL"),
        "route 衝突起因のエラー: {err}"
    );
    assert!(
        !dir.path().join("dist/guide/index.html").exists(),
        "書き出し前に中断される"
    );
}

#[test]
fn base_url_がフル_url_なら_sitemap_xml_を生成する() {
    let dir = build_fixture_with(|root| {
        let path = root.join("yuzu.toml");
        let src = fs::read_to_string(&path).unwrap();
        fs::write(
            &path,
            src.replace("\"/docs/\"", "\"https://example.com/docs/\""),
        )
        .unwrap();
    });
    let sitemap = fs::read_to_string(dir.path().join("dist/sitemap.xml")).unwrap();
    assert!(sitemap.starts_with("<?xml version=\"1.0\""), "{sitemap}");
    assert!(sitemap.contains("<urlset xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\">"));
    assert!(sitemap.contains("<loc>https://example.com/docs/</loc>"));
    assert!(sitemap.contains("<loc>https://example.com/docs/guide/</loc>"));
    assert!(sitemap.contains("<loc>https://example.com/docs/guide/getting-started/</loc>"));
    assert!(
        !sitemap.contains("first-steps"),
        "エイリアスは載らない: {sitemap}"
    );
    assert_eq!(
        sitemap.matches("<url>").count(),
        3,
        "実ページの数だけ: {sitemap}"
    );
}

#[test]
fn base_url_がパスだけなら_sitemap_は生成しない() {
    let dir = build_fixture(LiveReloadMode::None);
    assert!(!dir.path().join("dist/sitemap.xml").exists());
}

#[test]
fn poll_モードはオートリフレッシュが注入される() {
    let dir = build_fixture(LiveReloadMode::Poll);
    let index = fs::read_to_string(dir.path().join("dist/index.html")).unwrap();
    assert!(index.contains("autorefresh.js"));
    assert!(index.contains("data-base=\"/docs/\""));
    assert!(!index.contains("livereload.js"));
}

#[test]
fn ws_モードは_livereload_js_が注入される() {
    let dir = build_fixture(LiveReloadMode::Ws);
    let index = fs::read_to_string(dir.path().join("dist/index.html")).unwrap();
    assert!(index.contains("js/livereload.js"));
    assert!(!index.contains("autorefresh.js"));
}

#[test]
fn llms_txt_のスナップショット() {
    let dir = build_fixture(LiveReloadMode::None);
    let dist = dir.path().join("dist");

    let llms = fs::read_to_string(dist.join("llms.txt")).unwrap();
    let full = fs::read_to_string(dist.join("llms-full.txt")).unwrap();

    insta::assert_snapshot!("llms_txt", llms);
    insta::assert_snapshot!("llms_full_txt", full);
}

/// fixture を上書きしてビルドする共通ヘルパ
fn build_fixture_with(edit: impl FnOnce(&Path)) -> tempfile::TempDir {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/sample-docs");
    let dir = tempfile::tempdir().unwrap();
    copy_tree(&fixture, dir.path());
    edit(dir.path());

    let rc = yuzu_config::load(dir.path()).unwrap();
    let site = yuzu_core::build_site_model(
        &rc.content_dir,
        &rc.config.input.ignore,
        &yuzu_core::MarkdownOptions {
            glossary: yuzu_render::glossary_options(&rc.config),
            search_page: yuzu_render::search_page_options(&rc.config),
            ..yuzu_core::MarkdownOptions::default()
        },
    )
    .unwrap();
    render_site(&RenderParams {
        config: &rc,
        site: &site,
        live_reload: LiveReloadMode::None,
        ctx: yuzu_render::RenderCtx::default(),
        git_dates: None,
    })
    .unwrap();
    dir
}

#[test]
fn llms_false_のページは両ファイルから除外される() {
    let dir = build_fixture_with(|root| {
        // getting-started.md を llms: false に
        let path = root.join("content/guide/getting-started.md");
        let src = fs::read_to_string(&path).unwrap();
        fs::write(
            &path,
            src.replace("title: はじめに", "title: はじめに\nllms: false"),
        )
        .unwrap();
    });
    let dist = dir.path().join("dist");

    let llms = fs::read_to_string(dist.join("llms.txt")).unwrap();
    assert!(!llms.contains("getting-started"), "llms.txt:\n{llms}");
    // ガイドセクションには index（ガイド）が残る = 見出しは消えない
    assert!(llms.contains("## ガイド"), "llms.txt:\n{llms}");
    // 他ページは残る
    assert!(llms.contains("- [ホーム]"));

    let full = fs::read_to_string(dist.join("llms-full.txt")).unwrap();
    assert!(!full.contains("こんにちは yuzu"), "本文が除外される");

    // セクション内の全ページを除外すると、リンク 0 件の見出しごと消える
    let dir = build_fixture_with(|root| {
        for rel in ["content/guide/getting-started.md", "content/guide/index.md"] {
            let path = root.join(rel);
            let src = fs::read_to_string(&path).unwrap();
            fs::write(&path, src.replacen("---\n", "---\nllms: false\n", 1)).unwrap();
        }
    });
    let llms = fs::read_to_string(dir.path().join("dist/llms.txt")).unwrap();
    assert!(!llms.contains("## ガイド"), "llms.txt:\n{llms}");
}

#[test]
fn site_logo_の有無でヘッダーの_img_が切り替わる() {
    // 未設定（既存 fixture）: img も has-logo も出ない（🍊 は CSS 側なので HTML に痕跡なし）
    let dir = build_fixture(LiveReloadMode::None);
    let index = fs::read_to_string(dir.path().join("dist/index.html")).unwrap();
    assert!(!index.contains("site-logo"));
    assert!(!index.contains("has-logo"));

    // 設定あり: baseUrl（/docs/）が前置された src と has-logo クラス、装飾扱いの alt=""
    let dir = build_fixture_with(|root| {
        fs::write(
            root.join("yuzu.toml"),
            "[site]\ntitle = \"Fixture Docs\"\nlogo = \"/images/logo.svg\"\n\n[build]\nbase_url = \"/docs/\"\n",
        )
        .unwrap();
    });
    let index = fs::read_to_string(dir.path().join("dist/index.html")).unwrap();
    assert!(
        index.contains(r#"<a class="site-title has-logo" href="/docs/">"#),
        "index.html:\n{index}"
    );
    assert!(
        index.contains(r#"<img class="site-logo" src="/docs/images/logo.svg" alt="">"#),
        "index.html:\n{index}"
    );
}

#[test]
fn llms_無効化と_full_無効化() {
    // enabled: false → 両ファイルとも出ない
    let dir = build_fixture_with(|root| {
        fs::write(
            root.join("yuzu.toml"),
            "[site]\ntitle = \"Fixture Docs\"\n\n[llms]\nenabled = false\n",
        )
        .unwrap();
    });
    assert!(!dir.path().join("dist/llms.txt").exists());
    assert!(!dir.path().join("dist/llms-full.txt").exists());

    // full: false → llms.txt のみ
    let dir = build_fixture_with(|root| {
        fs::write(
            root.join("yuzu.toml"),
            "[site]\ntitle = \"Fixture Docs\"\n\n[llms]\nfull = false\n",
        )
        .unwrap();
    });
    assert!(dir.path().join("dist/llms.txt").exists());
    assert!(!dir.path().join("dist/llms-full.txt").exists());
}

#[test]
fn mermaid_ssr_はページ単位で_mermaid_js_の要否が決まる() {
    let dir = build_fixture_with(|root| {
        // backend を ssr に。sequence のみのページと flowchart ページを追加
        fs::write(
            root.join("yuzu.toml"),
            "[site]\ntitle = \"Fixture Docs\"\n\n[markdown.mermaid]\nbackend = \"ssr\"\n",
        )
        .unwrap();
        fs::write(
            root.join("content/seq-only.md"),
            "---\ntitle: シーケンスのみ\n---\n# 図\n\n```mermaid\nsequenceDiagram\n    A->>B: こんにちは\n```\n",
        )
        .unwrap();
        fs::write(
            root.join("content/fallback.md"),
            "---\ntitle: ジャーニー\n---\n# 図\n\n```mermaid\njourney\n    title 一日\n    section 朝\n      起床: 5: 私\n```\n",
        )
        .unwrap();
    });
    let dist = dir.path().join("dist");

    // sequence のみのページ: SSR された SVG があり、mermaid.js は読み込まない
    let seq = fs::read_to_string(dist.join("seq-only/index.html")).unwrap();
    assert!(seq.contains("figure class=\"mermaid-ssr\""), "SSR figure");
    assert!(seq.contains("<svg class=\"tankan tankan-sequence\""));
    assert!(seq.contains("var(--fg, #1f2328)"), "テーマ変数の注入");
    assert!(!seq.contains("pre class=\"mermaid\""), "フォールバックなし");
    assert!(!seq.contains("mermaid.min.js"), "mermaid.js 不要");

    // 未対応図種（journey）のページ: フォールバックして mermaid.js を読み込む
    // （mindmap / timeline は Phase 27 で SSR 対応済みのため例に使えない）
    let fallback = fs::read_to_string(dist.join("fallback/index.html")).unwrap();
    assert!(fallback.contains("pre class=\"mermaid\""), "フォールバック");
    assert!(fallback.contains("mermaid.min.js"), "mermaid.js 必要");
    assert!(!fallback.contains("mermaid-ssr"));

    // 既存 fixture の index.md（```mermaid の graph TD）は M2 から SSR 側
    let index = fs::read_to_string(dist.join("index.html")).unwrap();
    assert!(index.contains("tankan-flowchart"), "flowchart も SSR");
    assert!(!index.contains("mermaid.min.js"));
}

#[test]
fn math_はページ単位で_katex_の要否が決まる() {
    let dir = build_fixture_with(|root| {
        fs::write(
            root.join("content/formula.md"),
            "---\ntitle: 数式\n---\n# 数式\n\nインライン $x^2$ と:\n\n$$\nE = mc^2\n$$\n",
        )
        .unwrap();
        fs::write(
            root.join("content/code-math.md"),
            "---\ntitle: 数式フェンス\n---\n# 数式フェンス\n\n```math\na^2 + b^2 = c^2\n```\n",
        )
        .unwrap();
    });
    let dist = dir.path().join("dist");

    // 数式ページ: comrak の math 出力があり KaTeX 一式を読み込む
    let formula = fs::read_to_string(dist.join("formula/index.html")).unwrap();
    assert!(formula.contains("data-math-style=\"display\""), "math 出力");
    assert!(formula.contains("vendor/katex/katex.min.css"), "KaTeX CSS");
    assert!(formula.contains("vendor/katex/katex.min.js"), "KaTeX JS");
    assert!(formula.contains("js/katex-init.js"), "初期化 JS");

    // ```math フェンスのみのページも KaTeX を読み込む（highlight.rs のガードの結合確認）
    let code_math = fs::read_to_string(dist.join("code-math/index.html")).unwrap();
    assert!(
        code_math.contains("<code class=\"language-math\" data-math-style=\"display\""),
        "comrak の特殊化が生きている:\n{code_math}"
    );
    assert!(code_math.contains("vendor/katex/katex.min.js"));

    // 数式のないページには KaTeX が出ない
    let index = fs::read_to_string(dist.join("index.html")).unwrap();
    assert!(!index.contains("katex"), "数式なしページに KaTeX 不要");

    // math.enabled=false なら $ はテキストのまま・KaTeX も読み込まない
    let dir = build_fixture_with(|root| {
        fs::write(
            root.join("yuzu.toml"),
            "[site]\ntitle = \"Fixture Docs\"\n\n[markdown.math]\nenabled = false\n",
        )
        .unwrap();
        fs::write(
            root.join("content/formula.md"),
            "---\ntitle: 数式\n---\n# 数式\n\nインライン $x^2$ の話。\n",
        )
        .unwrap();
    });
    let formula = fs::read_to_string(dir.path().join("dist/formula/index.html")).unwrap();
    assert!(formula.contains("$x^2$"), "素のテキストのまま");
    assert!(!formula.contains("data-math-style=\"inline\""));
    assert!(!formula.contains("katex"));
}

#[test]
fn 前後ページリンクは_nav_順で全ページを連結する() {
    // フラット順: ホーム → はじめに → 応用（サイドバー表示順）
    let dir = build_fixture_with(|root| {
        fs::write(
            root.join("content/guide/advanced.md"),
            "---\ntitle: 応用\norder: 2\n---\n# 応用\n\n本文\n",
        )
        .unwrap();
    });
    let dist = dir.path().join("dist");

    // 先頭（ホーム）: prev なし・next = ガイド（ディレクトリ index）
    let index = fs::read_to_string(dist.join("index.html")).unwrap();
    assert!(!index.contains("rel=\"prev\""));
    assert!(index.contains(r#"<a class="pager-next" rel="next" href="/docs/guide/">"#));

    // 中間（はじめに）: 両方あり
    let mid = fs::read_to_string(dist.join("guide/getting-started/index.html")).unwrap();
    assert!(mid.contains(r#"rel="prev" href="/docs/guide/">"#));
    assert!(mid.contains(r#"rel="next" href="/docs/guide/advanced/">"#));

    // 末尾（応用）: next なし・prev = はじめに
    let last = fs::read_to_string(dist.join("guide/advanced/index.html")).unwrap();
    assert!(!last.contains("rel=\"next\""));
    assert!(last.contains(r#"rel="prev" href="/docs/guide/getting-started/">"#));

    // llms.txt のリンク出現順と一致する（この標準構成において。
    // トップレベル葉ページがディレクトリより後ろに並ぶ構成では llms 側が
    // 先頭セクションへ前寄せするため一致しない = 仕様差として許容）
    let llms = fs::read_to_string(dist.join("llms.txt")).unwrap();
    let pos = |needle: &str| {
        llms.find(needle)
            .unwrap_or_else(|| panic!("{needle} が llms.txt にない"))
    };
    assert!(pos("(/docs/index.md)") < pos("(/docs/guide.md)"));
    assert!(pos("(/docs/guide.md)") < pos("(/docs/guide/getting-started.md)"));
    assert!(pos("(/docs/guide/getting-started.md)") < pos("(/docs/guide/advanced.md)"));
}

#[test]
fn パンくずはラベルとリンクを出し分ける() {
    // fixture の guide/ は index.md 持ち → パンくず中間がリンクになる
    let dir = build_fixture(LiveReloadMode::None);
    let dist = dir.path().join("dist");

    // 深いページ: ホーム(リンク) / ガイド(リンク) / はじめに(現在・リンクなし)
    let page = fs::read_to_string(dist.join("guide/getting-started/index.html")).unwrap();
    assert!(
        page.contains(r#"<li><a href="/docs/">ホーム</a></li>"#),
        "page:\n{page}"
    );
    assert!(page.contains(r#"<li><a href="/docs/guide/">ガイド</a></li>"#));
    assert!(
        page.contains(r#"<span class="breadcrumb-current" aria-current="page">はじめに</span>"#)
    );

    // ディレクトリ index 自身: [ホーム, ガイド(現在)]
    let guide = fs::read_to_string(dist.join("guide/index.html")).unwrap();
    assert!(guide.contains(r#"<li><a href="/docs/">ホーム</a></li>"#));
    assert!(
        guide.contains(r#"<span class="breadcrumb-current" aria-current="page">ガイド</span>"#)
    );

    // トップページには出ない
    let index = fs::read_to_string(dist.join("index.html")).unwrap();
    assert!(!index.contains("class=\"breadcrumb\""));
}

#[test]
fn サイドバーは現在セクションだけ開いた_details_になる() {
    let dir = build_fixture(LiveReloadMode::None);
    let dist = dir.path().join("dist");

    // guide 配下のページ: guide セクションが open、summary 内はリンク
    // （テキストクリック = 遷移・マーカークリック = 開閉）
    let page = fs::read_to_string(dist.join("guide/getting-started/index.html")).unwrap();
    assert!(
        page.contains(r#"<details class="nav-section" open>"#),
        "{page}"
    );
    assert!(
        page.contains(r#"<summary><a href="/docs/guide/">ガイド</a></summary>"#),
        "{page}"
    );

    // トップページ: guide セクションは閉じる
    let index = fs::read_to_string(dist.join("index.html")).unwrap();
    assert!(
        index.contains(r#"<details class="nav-section">"#),
        "{index}"
    );
    assert!(!index.contains(r#"<details class="nav-section" open>"#));

    // ディレクトリ index 自身: open ＋ active は summary 配下のリンク側
    let guide = fs::read_to_string(dist.join("guide/index.html")).unwrap();
    assert!(guide.contains(r#"<details class="nav-section" open>"#));
    assert!(
        guide.contains(
            r#"<summary><a href="/docs/guide/" aria-current="page">ガイド</a></summary>"#
        ),
        "{guide}"
    );

    // 404: trail が空なので全セクション閉じ
    let nf = fs::read_to_string(dist.join("404.html")).unwrap();
    assert!(
        !nf.contains(r#"<details class="nav-section" open>"#),
        "{nf}"
    );
}

#[test]
fn nav_collapse_false_なら従来の全展開になる() {
    let dir = build_fixture_with(|root| {
        let path = root.join("yuzu.toml");
        let src = fs::read_to_string(&path).unwrap();
        fs::write(
            &path,
            src.replace("[site]", "[nav]\ncollapse = false\n\n[site]"),
        )
        .unwrap();
    });
    let index = fs::read_to_string(dir.path().join("dist/index.html")).unwrap();
    assert!(!index.contains("nav-section"), "{index}");
    assert!(
        index.contains(r#"<a href="/docs/guide/">ガイド</a>"#),
        "{index}"
    );
}

#[test]
fn toc_は入れ子になり_theme_toc_levels_で範囲を変えられる() {
    // 既定（2-3）: <nav> 内包・h3 は h2 の子・h4 は出ない
    let dir = build_fixture(LiveReloadMode::None);
    let page =
        fs::read_to_string(dir.path().join("dist/guide/getting-started/index.html")).unwrap();
    assert!(
        page.contains(r#"<nav aria-label="このページの目次">"#),
        "{page}"
    );
    assert!(
        page.contains("<a href=\"#使い方\">使い方</a>\n\n<ul>"),
        "h3 が h2 の入れ子にならない: {page}"
    );
    assert!(!page.contains("toc-level-4"), "{page}");

    // levels "2-4": h4 も入れ子で出る
    let dir = build_fixture_with(|root| {
        let path = root.join("yuzu.toml");
        let src = fs::read_to_string(&path).unwrap();
        fs::write(
            &path,
            src.replace("[site]", "[theme.toc]\nlevels = \"2-4\"\n\n[site]"),
        )
        .unwrap();
    });
    let page =
        fs::read_to_string(dir.path().join("dist/guide/getting-started/index.html")).unwrap();
    assert!(page.contains("toc-level-4"), "{page}");

    // 不正な levels は警告して既定へ縮退（ビルドは成功する）
    let dir = build_fixture_with(|root| {
        let path = root.join("yuzu.toml");
        let src = fs::read_to_string(&path).unwrap();
        fs::write(
            &path,
            src.replace("[site]", "[theme.toc]\nlevels = \"abc\"\n\n[site]"),
        )
        .unwrap();
    });
    let page =
        fs::read_to_string(dir.path().join("dist/guide/getting-started/index.html")).unwrap();
    assert!(!page.contains("toc-level-4"), "{page}");
    assert!(page.contains("toc-level-3"), "{page}");
}

#[test]
fn search_有効なら検索_ui_が入り_無効なら出ない() {
    // 既定（enabled: true）
    let dir = build_fixture(LiveReloadMode::None);
    let index = fs::read_to_string(dir.path().join("dist/index.html")).unwrap();
    assert!(index.contains("yuzu-search-input"));
    assert!(index.contains("js/search-ui.js"));
    assert!(index.contains("data-search-base=\"/docs/_search/\""));

    // 無効化した fixture
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/sample-docs");
    let dir = tempfile::tempdir().unwrap();
    copy_tree(&fixture, dir.path());
    fs::write(
        dir.path().join("yuzu.toml"),
        "[site]\ntitle = \"Fixture Docs\"\n\n[search]\nenabled = false\n",
    )
    .unwrap();
    let rc = yuzu_config::load(dir.path()).unwrap();
    let site = yuzu_core::build_site_model(
        &rc.content_dir,
        &rc.config.input.ignore,
        &yuzu_core::MarkdownOptions::default(),
    )
    .unwrap();
    render_site(&RenderParams {
        config: &rc,
        site: &site,
        live_reload: LiveReloadMode::None,
        ctx: yuzu_render::RenderCtx::default(),
        git_dates: None,
    })
    .unwrap();
    let index = fs::read_to_string(dir.path().join("dist/index.html")).unwrap();
    assert!(!index.contains("yuzu-search-input"));
    assert!(!index.contains("search-ui.js"));
}

#[test]
fn base_url_がリンクとアセットに反映される() {
    let dir = build_fixture(LiveReloadMode::None);
    let index = fs::read_to_string(dir.path().join("dist/index.html")).unwrap();

    // 本文リンク（.md → pretty URL）・画像・アセット・ナビすべて /docs/ 配下
    assert!(index.contains("href=\"/docs/guide/getting-started/\""));
    assert!(index.contains("src=\"/docs/images/logo.svg\""));
    assert!(index.contains("href=\"/docs/_assets/css/theme.css\""));
}

#[test]
fn theme_css_vars_が_head_に注入される() {
    let dir = build_fixture_with(|root| {
        let path = root.join("yuzu.toml");
        let src = fs::read_to_string(&path).unwrap();
        fs::write(
            &path,
            format!(
                "[theme.css_vars]\naccent = \"#0a6cff\"\n\n[theme.css_vars_dark]\naccent = \"#7fb2ff\"\n\n{src}"
            ),
        )
        .unwrap();
    });
    let index = fs::read_to_string(dir.path().join("dist/index.html")).unwrap();
    assert!(index.contains("--accent: #0a6cff;"), "light の上書きが入る");
    assert!(index.contains("html[data-theme=\"dark\"] {"));
    assert!(index.contains("--accent: #7fb2ff;"), "dark の上書きが入る");
    // dark 側の上書きは画面専用（印刷では :root のライト値が生きる。Phase 55）
    assert!(index.contains("@media screen {"), "{index}");
}

#[test]
fn theme_css_vars_未設定なら_style_を注入しない() {
    let dir = build_fixture(LiveReloadMode::None);
    let index = fs::read_to_string(dir.path().join("dist/index.html")).unwrap();
    assert!(!index.contains("theme.cssVars 由来"));
}

#[test]
fn include_drafts_で_draft_ページがバナー付きで出力される() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/sample-docs");
    let dir = tempfile::tempdir().unwrap();
    copy_tree(&fixture, dir.path());
    fs::write(
        dir.path().join("content/wip.md"),
        "---\ntitle: 下書きページ\ndraft: true\n---\n# 下書きページ\n\n執筆中。\n",
    )
    .unwrap();

    let rc = yuzu_config::load(dir.path()).unwrap();
    // --drafts 相当: include_drafts = true でモデル構築
    let site = yuzu_core::build_site_model_cached(
        &rc.content_dir,
        &rc.config.input.ignore,
        &MarkdownOptions::default(),
        None,
        true,
    )
    .unwrap();
    render_site(&RenderParams {
        config: &rc,
        site: &site,
        live_reload: LiveReloadMode::None,
        ctx: yuzu_render::RenderCtx::default(),
        git_dates: None,
    })
    .unwrap();

    let wip = fs::read_to_string(dir.path().join("dist/wip/index.html")).unwrap();
    assert!(wip.contains("draft-banner"), "draft バナーが出る");
    let index = fs::read_to_string(dir.path().join("dist/index.html")).unwrap();
    assert!(!index.contains("draft-banner"), "非 draft ページには出ない");
    assert!(index.contains("下書きページ"), "draft がナビに載る");

    // 通常ビルド（include_drafts = false）では draft ページ自体が出力されない
    let site = yuzu_core::build_site_model(
        &rc.content_dir,
        &rc.config.input.ignore,
        &MarkdownOptions::default(),
    )
    .unwrap();
    render_site(&RenderParams {
        config: &rc,
        site: &site,
        live_reload: LiveReloadMode::None,
        ctx: yuzu_render::RenderCtx::default(),
        git_dates: None,
    })
    .unwrap();
    assert!(
        !dir.path().join("dist/wip").exists() || {
            // output.clean 既定 true なら dist が作り直されて消えている
            !dir.path().join("dist/wip/index.html").exists()
        }
    );
}

#[test]
fn ページ単位_md_が原文そのままで配信される() {
    let dir = build_fixture(LiveReloadMode::None);
    let dist = dir.path().join("dist");

    // ルートは index.md、下層は route 末尾スラッシュを外した .md
    let root_md = fs::read_to_string(dist.join("index.md")).unwrap();
    let source = fs::read_to_string(dir.path().join("content/index.md")).unwrap();
    assert_eq!(root_md, source, "原文バイトそのまま（frontmatter 込み）");
    assert!(dist.join("guide/getting-started.md").is_file());

    // HTML にはコピーボタン用の data-md-url と page-copy.js が入る
    let index = fs::read_to_string(dist.join("index.html")).unwrap();
    assert!(index.contains(r#"data-md-url="/docs/index.md""#));
    assert!(index.contains("js/page-copy.js"));
    let guide = fs::read_to_string(dist.join("guide/getting-started/index.html")).unwrap();
    assert!(guide.contains(r#"data-md-url="/docs/guide/getting-started.md""#));

    // llms.txt のリンクは .md を指す
    let llms = fs::read_to_string(dist.join("llms.txt")).unwrap();
    assert!(
        llms.contains("(/docs/guide/getting-started.md)"),
        "llms.txt:\n{llms}"
    );
    assert!(!llms.contains("(/docs/guide/getting-started/)"));
}

#[test]
fn git_メタは日付マップと_edit_url_設定から出る() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/sample-docs");
    let dir = tempfile::tempdir().unwrap();
    copy_tree(&fixture, dir.path());
    fs::write(
        dir.path().join("yuzu.toml"),
        "[site]\ntitle = \"Fixture Docs\"\n\n[build]\nbase_url = \"/docs/\"\n\n[git]\nlast_updated = true\nedit_url = \"https://example.com/edit/main/content/{path}\"\n",
    )
    .unwrap();

    let rc = yuzu_config::load(dir.path()).unwrap();
    let site = yuzu_core::build_site_model(
        &rc.content_dir,
        &rc.config.input.ignore,
        &MarkdownOptions::default(),
    )
    .unwrap();
    // git の実行は cli 層の責務なので、テストでは日付マップを直接注入する
    let mut dates = std::collections::HashMap::new();
    dates.insert("index.md".to_string(), "2026-07-14".to_string());
    render_site(&RenderParams {
        config: &rc,
        site: &site,
        live_reload: LiveReloadMode::None,
        ctx: yuzu_render::RenderCtx::default(),
        git_dates: Some(&dates),
    })
    .unwrap();

    let index = fs::read_to_string(dir.path().join("dist/index.html")).unwrap();
    assert!(index.contains("最終更新: 2026-07-14"), "日付が出る");
    assert!(
        index.contains(r#"href="https://example.com/edit/main/content/index.md""#),
        "editUrl の {{path}} が置換される"
    );

    // 日付マップに無いページは編集リンクだけ（最終更新は出ない）
    let guide =
        fs::read_to_string(dir.path().join("dist/guide/getting-started/index.html")).unwrap();
    assert!(!guide.contains("最終更新"), "未追跡ページは日付なし");
    assert!(
        guide.contains("content/guide/getting-started.md\""),
        "編集リンクは出る"
    );
}

#[test]
fn git_メタ未設定なら_page_meta_を出さない() {
    let dir = build_fixture(LiveReloadMode::None);
    let index = fs::read_to_string(dir.path().join("dist/index.html")).unwrap();
    assert!(!index.contains("page-meta"));
}

#[test]
fn openapi_ブロックは_api_spec_として_ssr_される() {
    let dir = build_fixture_with(|root| {
        fs::write(
            root.join("content/api.md"),
            concat!(
                "---\ntitle: API 仕様\n---\n# API\n\n",
                "```openapi\n",
                "openapi: 3.0.3\n",
                "info:\n  title: ペット API\n  version: 1.2.3\n",
                "paths:\n",
                "  /pets:\n",
                "    get:\n",
                "      summary: ペット一覧\n",
                "      responses:\n",
                "        \"200\":\n",
                "          description: 成功\n",
                "```\n",
            ),
        )
        .unwrap();
    });
    let html = fs::read_to_string(dir.path().join("dist/api/index.html")).unwrap();
    assert!(html.contains("api-spec"), "SSR の器が出る:\n{html}");
    assert!(html.contains("api-method-get"), "メソッドバッジ");
    assert!(html.contains("ペット API"), "info.title");
    assert!(html.contains("/pets"), "パス");
    assert!(
        !html.contains("markdown-alert-caution"),
        "正常系はエラーボックスにならない"
    );
}

#[test]
fn swagger_2_0_ブロックも_ssr_される() {
    let dir = build_fixture_with(|root| {
        fs::write(
            root.join("content/legacy-api.md"),
            concat!(
                "---\ntitle: 旧 API 仕様\n---\n# 旧 API\n\n",
                "```openapi\n",
                "swagger: \"2.0\"\n",
                "info:\n  title: レガシー API\n  version: 0.9.0\n",
                "paths:\n",
                "  /orders:\n",
                "    get:\n",
                "      summary: 注文一覧\n",
                "      responses:\n",
                "        \"200\":\n",
                "          description: 成功\n",
                "          schema:\n",
                "            $ref: \"#/definitions/Order\"\n",
                "definitions:\n",
                "  Order:\n",
                "    type: object\n",
                "    properties:\n",
                "      id:\n",
                "        type: integer\n",
                "```\n",
            ),
        )
        .unwrap();
    });
    let html = fs::read_to_string(dir.path().join("dist/legacy-api/index.html")).unwrap();
    assert!(html.contains("api-spec"), "SSR の器が出る:\n{html}");
    assert!(html.contains("api-method-get"), "メソッドバッジ");
    assert!(html.contains("レガシー API"), "info.title");
    assert!(html.contains("api-schemas"), "definitions 一覧");
    assert!(html.contains("<code>Order</code>"), "スキーマ名");
    assert!(
        !html.contains("markdown-alert-caution"),
        "2.0 はエラーボックスにならない:\n{html}"
    );
}

#[test]
fn content_同伴の画像はコピーされ_src_が絶対_url_になる() {
    let dir = build_fixture_with(|root| {
        fs::write(root.join("content/guide/shot.png"), b"PNG-DUMMY").unwrap();
        fs::write(root.join("content/top.png"), b"PNG-TOP").unwrap();
        fs::write(
            root.join("content/guide/pics.md"),
            "---\ntitle: 画像\n---\n# 画像\n\n![ページ横](shot.png)\n\n![上の階層](../top.png)\n",
        )
        .unwrap();
    });
    let dist = dir.path().join("dist");

    // 同伴アセットが content と同じ相対パスへコピーされる
    assert_eq!(fs::read(dist.join("guide/shot.png")).unwrap(), b"PNG-DUMMY");
    assert!(dist.join("top.png").is_file());

    // 相対 src は base 付き絶対 URL へ解決される（pretty URL の階層ずれ対策）
    let html = fs::read_to_string(dist.join("guide/pics/index.html")).unwrap();
    assert!(html.contains(r#"src="/docs/guide/shot.png""#), "{html}");
    assert!(html.contains(r#"src="/docs/top.png""#));
}

#[test]
fn 存在しないパス用の_404_ページが生成される() {
    let dir = build_fixture(LiveReloadMode::None);
    let html = fs::read_to_string(dir.path().join("dist/404.html")).unwrap();

    assert!(html.contains("ページが見つかりません"), "{html}");
    // 404 は任意のパスで表示されるため、リンク・アセットは base 付き絶対 URL
    assert!(html.contains(r#"href="/docs/">トップページへ戻る"#));
    assert!(html.contains(r#"href="/docs/_assets/css/theme.css""#));
    // 検索ボックス付きヘッダーとサイドバー（ハイライトなし）で探し直せる
    assert!(html.contains("yuzu-search-input"));
    assert!(html.contains("sidebar-nav"));
    assert!(!html.contains(r#"aria-current="page""#));

    insta::assert_snapshot!("not_found_html", html);
}

#[test]
fn public_の_404_html_はテーマ生成より優先される() {
    let dir = build_fixture_with(|root| {
        fs::write(root.join("public/404.html"), "<h1>独自の 404</h1>").unwrap();
    });
    let html = fs::read_to_string(dir.path().join("dist/404.html")).unwrap();
    assert_eq!(html, "<h1>独自の 404</h1>");
}

#[test]
fn 壊れた_openapi_はエラーボックスでビルドは継続する() {
    let dir = build_fixture_with(|root| {
        fs::write(
            root.join("content/broken-api.md"),
            "---\ntitle: 壊れた API\n---\n# 壊れた API\n\n```openapi\nfoo: [unclosed\n```\n\n後続の本文。\n",
        )
        .unwrap();
    });
    let html = fs::read_to_string(dir.path().join("dist/broken-api/index.html")).unwrap();
    assert!(
        html.contains("markdown-alert-caution"),
        "エラーボックスが出る:\n{html}"
    );
    assert!(html.contains("後続の本文。"), "ページ自体は生成される");
}

/// pipeline が render_body_html へプロジェクトルートを渡していることの固定
/// （Markdown 断片は core 側で展開されるため、配線が切れると黙って
/// エラーボックスになる）
#[test]
fn markdown_断片が_dist_の_html_へ展開される() {
    let dir = build_fixture_with(|root| {
        fs::create_dir_all(root.join("snippets")).unwrap();
        fs::write(
            root.join("snippets/note.md"),
            "共通の**注意書き**が展開されます。\n",
        )
        .unwrap();
        let index = root.join("content/index.md");
        let source = fs::read_to_string(&index).unwrap();
        fs::write(
            &index,
            format!("{source}\n```include file=\"snippets/note.md\"\n```\n"),
        )
        .unwrap();
    });

    let html = fs::read_to_string(dir.path().join("dist/index.html")).unwrap();
    assert!(
        html.contains("共通の<strong>注意書き</strong>が展開されます。"),
        "断片が散文として展開される: {html}"
    );
    assert!(
        !html.contains("language-include"),
        "include フェンスがコードブロックとして残らない"
    );
}

#[test]
fn 用語集ページが生成されサイドバーと本文に反映される() {
    // 共有 fixture（build_fixture）には glossary を入れない = 既存
    // スナップショット 5 件が動かないことを保ったまま、設定を足した版で検証する
    let dir = build_fixture_with(|root| {
        let path = root.join("yuzu.toml");
        let src = fs::read_to_string(&path).unwrap();
        fs::write(
            &path,
            src.replace(
                "[site]",
                "[markdown.glossary.terms]\nSSG = \"Static Site Generator\"\n\n[markdown.crossref]\n\n[site]",
            ),
        )
        .unwrap();
        // 本文へ略語を仕込む（同じ用語を 2 回置いて初出だけ包まれることを見る）
        let index = root.join("content/index.md");
        let src = fs::read_to_string(&index).unwrap();
        fs::write(
            &index,
            format!("{src}\n\nyuzu は SSG です。SSG は 2 回目。\n"),
        )
        .unwrap();
    });
    let dist = dir.path().join("dist");

    // 用語集ページとページ単位 Markdown が出る
    let glossary = fs::read_to_string(dist.join("glossary/index.html")).unwrap();
    assert!(
        glossary.contains(r#"id="ssg""#),
        "用語ごとのアンカー:\n{glossary}"
    );
    assert!(dist.join("glossary.md").is_file());
    // 用語集ページ自身は abbr 化しない
    assert!(!glossary.contains("<abbr"), "{glossary}");
    // 実ファイルが無いので「このページを編集」リンクは出さない
    assert!(!glossary.contains("edit/main"), "{glossary}");

    // 本文はページ内初出だけが包まれる
    let index = fs::read_to_string(dist.join("index.html")).unwrap();
    assert_eq!(index.matches("<abbr").count(), 1, "{index}");
    assert!(
        index.contains(r#"<abbr title="Static Site Generator">SSG</abbr>"#),
        "{index}"
    );
    // サイドバー（nav）にも載る
    assert!(index.contains("glossary/\">用語集"), "{index}");
    // llms.txt にも通常ページとして載る
    let llms = fs::read_to_string(dist.join("llms.txt")).unwrap();
    assert!(llms.contains("用語集"), "{llms}");
}

#[test]
fn 検索結果ページが生成され集約からは除外される() {
    // 共有 fixture には search.page を入れない = 既存スナップショットが動かない
    // ことを保ったまま、設定を足した版で検証する（用語集と同じ流儀）。
    // baseUrl をフル URL にして sitemap の除外も同時に見る
    let dir = build_fixture_with(|root| {
        let path = root.join("yuzu.toml");
        let src = fs::read_to_string(&path).unwrap();
        fs::write(
            &path,
            src.replace("[site]", "[search]\npage = \"search\"\n\n[site]")
                .replace("\"/docs/\"", "\"https://example.com/docs/\""),
        )
        .unwrap();
    });
    let dist = dir.path().join("dist");

    // 専用テンプレートで生成される（結果コンテナ・noscript・件数の data 属性）
    let html = fs::read_to_string(dist.join("search/index.html")).unwrap();
    assert!(html.contains(r#"id="yuzu-search-page""#), "{html}");
    assert!(html.contains("<noscript>"), "{html}");
    assert!(html.contains(r#"data-page-size="10""#), "{html}");
    assert!(html.contains("js/search-page.js"), "{html}");
    insta::assert_snapshot!("search_html", html);

    // ページ単位 Markdown は出さない（llms からも除外済みで導線が無い）
    assert!(!dist.join("search.md").exists());
    // sitemap に載らない（実ページ 2 件のまま）
    let sitemap = fs::read_to_string(dist.join("sitemap.xml")).unwrap();
    assert!(!sitemap.contains("/search/"), "{sitemap}");
    assert_eq!(sitemap.matches("<url>").count(), 3, "{sitemap}");
    // llms.txt に載らない
    let llms = fs::read_to_string(dist.join("llms.txt")).unwrap();
    assert!(!llms.contains("search"), "{llms}");
    // サイドバーには出ず、全ページの search-ui.js に遷移先だけ配られる
    let index = fs::read_to_string(dist.join("index.html")).unwrap();
    assert!(
        !index.contains(r#"href="https://example.com/docs/search/""#),
        "{index}"
    );
    assert!(
        index.contains(r#"data-search-page="https://example.com/docs/search/""#),
        "{index}"
    );
}

#[test]
fn 検索無効なら結果ページは生成されない() {
    let dir = build_fixture_with(|root| {
        let path = root.join("yuzu.toml");
        let src = fs::read_to_string(&path).unwrap();
        fs::write(
            &path,
            src.replace(
                "[site]",
                "[search]\nenabled = false\npage = \"search\"\n\n[site]",
            ),
        )
        .unwrap();
    });
    assert!(!dir.path().join("dist/search").exists());
}
