//! インデックス生成 → ネイティブ検索の統合テスト
//! （ブラウザ検索と同一のエンジン・モデルを通る）

use std::fs;
use std::path::Path;

use yuzu_core::MarkdownOptions;
use yuzu_index::{
    IndexCtx, IndexParams, IndexSession, build_search_index, build_search_index_with, search_dist,
};

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
    assert_eq!(manifest["version"], 3);
    assert_eq!(manifest["docCount"], 4);
    assert_eq!(manifest["tokenizer"]["kind"], "vaporetto");
    // モデルは同梱モデルと同一バイト（sha256 が入っている）
    assert_eq!(
        manifest["tokenizer"]["modelSha256"].as_str().unwrap().len(),
        64
    );
    // content_hash（OPFS キャッシュの版管理用）は sha256 hex（64桁）
    assert_eq!(manifest["contentHash"].as_str().unwrap().len(), 64);
}

/// dist/_search/manifest.json から contentHash を読む
fn content_hash_of(dist: &Path) -> String {
    let manifest: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(dist.join("_search/manifest.json")).unwrap())
            .unwrap();
    manifest["contentHash"].as_str().unwrap().to_string()
}

#[test]
fn content_hash_は同一入力なら決定的() {
    let (_content1, dist1) = build_fixture();
    let (_content2, dist2) = build_fixture();
    assert_eq!(
        content_hash_of(dist1.path()),
        content_hash_of(dist2.path()),
        "同一フィクスチャの2回ビルドは同じ content_hash になる"
    );
}

#[test]
fn content_hash_は内容が変わると変化する() {
    let (_content, dist_before) = build_fixture();

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
    // 追加ページ 1 つぶん語彙が変わる
    write(
        content.path(),
        "guide/extra.md",
        "---\ntitle: 追加\n---\n# 追加\n\n新しいページを1つ追加しました。\n",
    );
    let md_opts = MarkdownOptions::default();
    let site = yuzu_core::build_site_model(content.path(), &[], &md_opts).unwrap();
    let dist_after = tempfile::tempdir().unwrap();
    build_search_index(&site, &md_opts, &IndexParams::default(), dist_after.path()).unwrap();

    assert_ne!(
        content_hash_of(dist_before.path()),
        content_hash_of(dist_after.path()),
        "内容が変わると content_hash も変わる"
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

#[test]
fn index_code_でコード内の関数名がヒットし抜粋に出る() {
    // 特別言語（mermaid 等）の除外は yuzu-core 側の単体テストで全数検証済み。
    // ここではこの層固有の配線（IndexParams → 抽出、ヒット・抜粋 merge）だけを見る
    let content = tempfile::tempdir().unwrap();
    write(
        content.path(),
        "index.md",
        "---\ntitle: API\n---\n# API リファレンス\n\n接続の設定を説明します。\n\n```rust\nfn plutoResolve(host: &str) {}\n```\n",
    );
    let md_opts = MarkdownOptions::default();
    let site = yuzu_core::build_site_model(content.path(), &[], &md_opts).unwrap();
    // 2 回のインデックス構築でトークナイザ（zstd モデル展開）を共有して時間を抑える
    let session = IndexSession::default();
    let ctx = IndexCtx {
        cache: None,
        outputs: None,
        session: Some(&session),
    };

    // 既定（index_code=false）: コード内の関数名ではヒットしない
    // （builder が params.index_code を無視・固定していないことの e2e ガード）
    let dist_off = tempfile::tempdir().unwrap();
    build_search_index_with(
        &site,
        &md_opts,
        &IndexParams::default(),
        dist_off.path(),
        &ctx,
    )
    .unwrap();
    assert!(
        search_dist(dist_off.path(), "plutoResolve", 10)
            .unwrap()
            .is_empty(),
        "既定ではコードは索引されない"
    );

    // index_code=true: 関数名でヒットし、抜粋にコード行が出る
    let dist_on = tempfile::tempdir().unwrap();
    let params = IndexParams {
        index_code: true,
        ..IndexParams::default()
    };
    build_search_index_with(&site, &md_opts, &params, dist_on.path(), &ctx).unwrap();
    let results = search_dist(dist_on.path(), "plutoResolve", 10).unwrap();
    assert!(!results.is_empty(), "index_code でコードがヒットする");
    assert!(
        results[0].excerpt.contains("plutoResolve"),
        "抜粋にコード行が出る: {}",
        results[0].excerpt
    );
}

/// dist/_search から token の位置込み postings を引く（terms.fst → 該当シャード解決）
fn postings_of(dist: &Path, token: &str) -> Vec<mikan::Posting> {
    let search = dist.join("_search");
    let map = fst::Map::new(fs::read(search.join("terms.fst")).unwrap()).unwrap();
    let term_id = map
        .get(token)
        .unwrap_or_else(|| panic!("{token} が terms.fst にない")) as u32;
    let manifest: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(search.join("manifest.json")).unwrap()).unwrap();
    let shard_meta = manifest["shards"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| {
            s["termStart"].as_u64().unwrap() as u32 <= term_id
                && term_id < s["termEnd"].as_u64().unwrap() as u32
        })
        .expect("term_id を含むシャードがある");
    let bytes = fs::read(search.join(shard_meta["file"].as_str().unwrap())).unwrap();
    let shard = mikan::Shard::parse(&bytes).unwrap();
    shard
        .postings_with_positions(term_id - shard_meta["termStart"].as_u64().unwrap() as u32)
        .unwrap()
}

/// 位置検証用の 1 ページ fixture（body/heading/title の各フィールドを持つ）。
/// ASCII 語はトークナイザで 1 語のまま小文字化されるので位置が予測できる
fn build_position_fixture() -> (tempfile::TempDir, tempfile::TempDir) {
    let content = tempfile::tempdir().unwrap();
    write(
        content.path(),
        "index.md",
        "---\ntitle: fruit\n---\n# fruit\n\nalpha beta gamma\n\n## delta\n\napple banana apple\n",
    );
    let md_opts = MarkdownOptions::default();
    let site = yuzu_core::build_site_model(content.path(), &[], &md_opts).unwrap();
    let dist = tempfile::tempdir().unwrap();
    build_search_index(&site, &md_opts, &IndexParams::default(), dist.path()).unwrap();
    (content, dist)
}

#[test]
fn postings_に出現位置が昇順で入る() {
    let (_content, dist) = build_position_fixture();
    // h2 セクション doc（doc_id 1）の body: "apple banana apple"
    let postings = postings_of(dist.path(), "apple");
    assert_eq!(postings.len(), 1, "{postings:?}");
    assert_eq!(postings[0].doc_id, 1);
    assert_eq!(postings[0].tf, 2, "本文は重み 1 × 2 回");
    assert_eq!(
        postings[0].positions,
        vec![0, 2],
        "body 先頭からのトークン添字"
    );
}

#[test]
fn 見出し語は_tf_と_pos_count_がずれる() {
    let (_content, dist) = build_position_fixture();
    let postings = postings_of(dist.path(), "delta");
    assert_eq!(postings.len(), 1, "{postings:?}");
    assert_eq!(postings[0].tf, 2, "見出しは重み 2 × 1 回");
    assert_eq!(postings[0].positions.len(), 1, "位置は実出現の 1 個だけ");
}

#[test]
fn フィールド境界に位置ギャップが入る() {
    let (_content, dist) = build_position_fixture();
    // h2 doc の body は 3 トークン（apple banana apple）→ 見出しフィールドは
    // body 末尾 + ギャップから始まる
    let postings = postings_of(dist.path(), "delta");
    assert_eq!(
        postings[0].positions,
        vec![3 + yuzu_index::FIELD_POS_GAP],
        "heading 先頭語の位置 = body トークン数 + FIELD_POS_GAP"
    );
}

#[test]
fn タイトル語の位置はリード_doc_だけに付く() {
    let (_content, dist) = build_position_fixture();
    // title "fruit" はリード doc（doc_id 0）にだけ加算される（h1 は body に出ない）
    let postings = postings_of(dist.path(), "fruit");
    assert_eq!(postings.len(), 1, "{postings:?}");
    assert_eq!(postings[0].doc_id, 0);
    assert!(!postings[0].positions.is_empty());
}

#[test]
fn フレーズ検索はフィールド境界をまたいで偽ヒットしない() {
    let content = tempfile::tempdir().unwrap();
    // 正例: 本文中に「ライブリロード」が連続で出る
    write(
        content.path(),
        "hit.md",
        "---\ntitle: 正例\n---\n# 正例\n\nライブリロードで自動更新される。\n",
    );
    // 偽例: セクション本文の末尾が「ライブ」・自セクション見出しの先頭が「リロード」。
    // 位置ストリームは body → heading の順なので、ギャップが無ければ隣接になってしまう
    write(
        content.path(),
        "boundary.md",
        "---\ntitle: 境界\n---\n# 境界\n\nリード文。\n\n## リロードの手順\n\n説明の最後がライブ\n",
    );
    let md_opts = MarkdownOptions::default();
    let site = yuzu_core::build_site_model(content.path(), &[], &md_opts).unwrap();
    let dist = tempfile::tempdir().unwrap();
    build_search_index(&site, &md_opts, &IndexParams::default(), dist.path()).unwrap();

    let results = search_dist(dist.path(), "\"ライブリロード\"", 10).unwrap();
    assert_eq!(results.len(), 1, "{results:?}");
    assert_eq!(results[0].url, "hit/");
    assert!(
        results[0].excerpt.contains("ライブリロード"),
        "抜粋にフレーズが出る: {}",
        results[0].excerpt
    );

    // 引用符なしなら両ページとも（token 単位で）ヒットする
    let results = search_dist(dist.path(), "ライブリロード", 10).unwrap();
    assert_eq!(results.len(), 2, "{results:?}");
}
