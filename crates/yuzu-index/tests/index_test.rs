//! インデックス生成 → ネイティブ検索の統合テスト
//! （ブラウザ検索と同一のエンジン・モデルを通る）

use std::fs;
use std::path::Path;

use yuzu_core::MarkdownOptions;
use yuzu_index::{IndexParams, build_search_index, search_dist};

fn write(dir: &Path, rel: &str, content: &str) {
    let path = dir.join(rel);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}

fn build_fixture() -> (tempfile::TempDir, tempfile::TempDir) {
    let content = tempfile::tempdir().unwrap();
    write(
        content.path(),
        "index.md",
        "---\ntitle: ホーム\norder: 1\n---\n# ようこそ\n\nyuzu は Markdown から静的サイトを生成するツールです。\n",
    );
    write(
        content.path(),
        "guide/getting-started.md",
        "---\ntitle: はじめに\n---\n# はじめに\n\nビルドは yuzu build を実行します。全文検索はブラウザで動きます。\n\n## 検索の使い方\n\n検索ボックスに日本語を入力します。検索は誤字にも寛容です。\n",
    );
    write(
        content.path(),
        "guide/theme.md",
        "---\ntitle: テーマ\n---\n# テーマ\n\nテーマは theme ディレクトリで上書きできます。\n",
    );

    let md_opts = MarkdownOptions::default();
    let site = yuzu_core::build_site_model(content.path(), &[], &md_opts).unwrap();

    let dist = tempfile::tempdir().unwrap();
    build_search_index(&site, &md_opts, &IndexParams::default(), dist.path()).unwrap();
    (content, dist)
}

#[test]
fn 生成物一式が_search_に揃う() {
    let (_content, dist) = build_fixture();
    let search = dist.path().join("_search");

    assert!(search.join("manifest.json").is_file());
    assert!(search.join("terms.fst").is_file());
    assert!(search.join("model.zst").is_file());
    assert!(search.join("index/0000.bin").is_file());
    // doc = セクション: index/theme はリードのみ、getting-started はリード + h2 で 2
    for doc_id in 0..4 {
        assert!(search.join(format!("fragment/{doc_id}.json")).is_file());
    }

    let manifest: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(search.join("manifest.json")).unwrap()).unwrap();
    assert_eq!(manifest["version"], 2);
    assert_eq!(manifest["docCount"], 4);
    assert_eq!(manifest["tokenizer"]["kind"], "vaporetto");
    // モデルは同梱モデルと同一バイト（sha256 が入っている）
    assert_eq!(
        manifest["tokenizer"]["modelSha256"].as_str().unwrap().len(),
        64
    );
}

#[test]
fn 日本語クエリでランク付き結果が返る() {
    let (_content, dist) = build_fixture();

    let results = search_dist(dist.path(), "検索", 10).unwrap();
    assert!(!results.is_empty());
    // 「検索」を最も濃く含む「検索の使い方」セクションが先頭（アンカー付き）
    assert_eq!(results[0].url, "guide/getting-started/");
    assert_eq!(results[0].anchor.as_deref(), Some("検索の使い方"));
    assert_eq!(results[0].heading.as_deref(), Some("検索の使い方"));
    assert!(results[0].score > 0.0);
    // 動的抜粋はクエリ語を含む
    assert!(
        results[0].excerpt.contains("検索"),
        "excerpt={}",
        results[0].excerpt
    );

    // テーマページには「検索」が出ないのでヒットしない
    assert!(results.iter().all(|r| r.url != "guide/theme/"));
}

#[test]
fn タイトル一致は重み付けで上位に来る() {
    let (_content, dist) = build_fixture();
    let results = search_dist(dist.path(), "テーマ", 10).unwrap();
    assert!(!results.is_empty());
    // タイトル語はリード doc（アンカーなし）に載る
    assert_eq!(results[0].url, "guide/theme/", "results={results:?}");
    assert_eq!(results[0].anchor, None);
    assert_eq!(results[0].heading, None);
}

#[test]
fn 見出し一致はセクション_doc_が先頭() {
    let (_content, dist) = build_fixture();
    let results = search_dist(dist.path(), "使い方", 10).unwrap();
    assert!(!results.is_empty());
    assert_eq!(
        results[0].heading.as_deref(),
        Some("検索の使い方"),
        "results={results:?}"
    );
    assert_eq!(results[0].anchor.as_deref(), Some("検索の使い方"));
}

#[test]
fn リード文ヒットはアンカーなし() {
    let (_content, dist) = build_fixture();
    let results = search_dist(dist.path(), "ビルド", 10).unwrap();
    assert!(!results.is_empty());
    let hit = &results[0];
    assert_eq!(hit.url, "guide/getting-started/", "results={results:?}");
    assert_eq!(hit.anchor, None, "リード文の内容はアンカーなしの doc");
}

#[test]
fn 一編集の誤字クエリでもヒットする() {
    let (_content, dist) = build_fixture();
    // "markdown" の 1 置換誤字
    let results = search_dist(dist.path(), "markdowm", 10).unwrap();
    assert!(!results.is_empty(), "誤字でもヒットする");
    assert_eq!(results[0].url, "");
}

#[test]
fn インデックスが無ければ_missing_エラー() {
    let dist = tempfile::tempdir().unwrap();
    let err = search_dist(dist.path(), "x", 10).unwrap_err();
    assert!(err.to_string().contains("yuzu build"));
}

#[test]
fn 同義語グループでゆれ表記の検索が正表記の文書にヒットする() {
    let content = tempfile::tempdir().unwrap();
    write(
        content.path(),
        "index.md",
        "---\ntitle: ホーム\n---\n# ホーム\n\nブラウザで検索できます。\n",
    );
    let md_opts = MarkdownOptions::default();
    let site = yuzu_core::build_site_model(content.path(), &[], &md_opts).unwrap();

    let dist = tempfile::tempdir().unwrap();
    let params = IndexParams {
        synonyms: vec![vec!["ブラウザ".to_string(), "閲覧ソフト".to_string()]],
        ..IndexParams::default()
    };
    build_search_index(&site, &md_opts, &params, dist.path()).unwrap();

    // manifest に正規化済みグループが焼き込まれる
    let manifest: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(dist.path().join("_search/manifest.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(manifest["synonyms"][0][0], "ブラウザ");

    // ゆれ表記（編集距離では届かない語）で正表記の文書がヒットし、
    // 抜粋には正表記側がハイライトされる
    let results = search_dist(dist.path(), "閲覧ソフト", 10).unwrap();
    assert_eq!(results.len(), 1, "{results:?}");
    assert_eq!(results[0].title, "ホーム");
    assert!(
        results[0].excerpt.contains("ブラウザ"),
        "{}",
        results[0].excerpt
    );
}
