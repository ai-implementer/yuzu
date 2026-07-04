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
        },
    )
    .unwrap();
    render_site(&RenderParams {
        config: &rc,
        site: &site,
        live_reload,
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

    // テーマアセット・public パススルー・build_id
    assert!(dist.join("_assets/css/theme.css").is_file());
    assert!(dist.join("_assets/js/theme.js").is_file());
    assert!(dist.join("_assets/vendor/mermaid.min.js").is_file());
    assert!(dist.join("images/logo.svg").is_file());
    assert!(dist.join("__yuzu/build_id").is_file());
    assert!(dist.join("llms.txt").is_file());
    assert!(dist.join("llms-full.txt").is_file());

    // 通常ビルドにはオートリフレッシュを注入しない
    let index = fs::read_to_string(dist.join("index.html")).unwrap();
    assert!(!index.contains("autorefresh.js"));
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

/// fixture を上書きしてビルドする共通ヘルパ（llms 系テスト用）
fn build_fixture_with(edit: impl FnOnce(&Path)) -> tempfile::TempDir {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/sample-docs");
    let dir = tempfile::tempdir().unwrap();
    copy_tree(&fixture, dir.path());
    edit(dir.path());

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
    // リンク 0 件になった guide セクションは見出しごと消える
    assert!(!llms.contains("## guide"), "llms.txt:\n{llms}");
    // 他ページは残る
    assert!(llms.contains("- [ホーム]"));

    let full = fs::read_to_string(dist.join("llms-full.txt")).unwrap();
    assert!(!full.contains("こんにちは yuzu"), "本文が除外される");
}

#[test]
fn llms_無効化と_full_無効化() {
    // enabled: false → 両ファイルとも出ない
    let dir = build_fixture_with(|root| {
        fs::write(
            root.join("yuzu.jsonc"),
            r#"{ "site": { "title": "Fixture Docs" }, "llms": { "enabled": false } }"#,
        )
        .unwrap();
    });
    assert!(!dir.path().join("dist/llms.txt").exists());
    assert!(!dir.path().join("dist/llms-full.txt").exists());

    // full: false → llms.txt のみ
    let dir = build_fixture_with(|root| {
        fs::write(
            root.join("yuzu.jsonc"),
            r#"{ "site": { "title": "Fixture Docs" }, "llms": { "full": false } }"#,
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
            root.join("yuzu.jsonc"),
            r#"{ "site": { "title": "Fixture Docs" },
                 "markdown": { "mermaid": { "backend": "ssr" } } }"#,
        )
        .unwrap();
        fs::write(
            root.join("content/seq-only.md"),
            "---\ntitle: シーケンスのみ\n---\n# 図\n\n```mermaid\nsequenceDiagram\n    A->>B: こんにちは\n```\n",
        )
        .unwrap();
        fs::write(
            root.join("content/class.md"),
            "---\ntitle: クラス図\n---\n# 図\n\n```mermaid\nclassDiagram\n    Animal <|-- Duck\n```\n",
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

    // 未対応図種（classDiagram）のページ: フォールバックして mermaid.js を読み込む
    let class = fs::read_to_string(dist.join("class/index.html")).unwrap();
    assert!(class.contains("pre class=\"mermaid\""), "フォールバック");
    assert!(class.contains("mermaid.min.js"), "mermaid.js 必要");
    assert!(!class.contains("mermaid-ssr"));

    // 既存 fixture の index.md（```mermaid の graph TD）は M2 から SSR 側
    let index = fs::read_to_string(dist.join("index.html")).unwrap();
    assert!(index.contains("tankan-flowchart"), "flowchart も SSR");
    assert!(!index.contains("mermaid.min.js"));
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
        dir.path().join("yuzu.jsonc"),
        r#"{ "site": { "title": "Fixture Docs" }, "search": { "enabled": false } }"#,
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
