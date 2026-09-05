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

/// 検索 tf キャッシュ × インクルード参照先（Phase 48）。
///
/// `file=` の参照先だけを編集したとき、ページの `.md` は無変更なので
/// source ハッシュは変わらない。参照先の内容ハッシュを別キーで持たないと
/// 検索結果が古いまま残る（実際に踏んだ不具合）
mod 検索キャッシュとインクルード {
    use super::*;
    use yuzu_core::BuildCache;

    /// content と参照先を持つプロジェクトを作る
    fn fixture(code: &str) -> (tempfile::TempDir, tempfile::TempDir) {
        let root = tempfile::tempdir().unwrap();
        write(root.path(), "src/api.rs", code);
        write(
            root.path(),
            "content/inc.md",
            "---\ntitle: 引用\n---\n# 引用\n\n```rust file=\"src/api.rs\"\n```\n",
        );
        write(
            root.path(),
            "content/plain.md",
            "---\ntitle: 素\n---\n# 素\n\n引用のないページ。\n",
        );
        let cache_dir = tempfile::tempdir().unwrap();
        (root, cache_dir)
    }

    fn params(root: &Path) -> IndexParams {
        IndexParams {
            index_code: true,
            project_root: Some(root.to_path_buf()),
            ..IndexParams::default()
        }
    }

    /// 1 回ビルドして (検索できる dist, ヒット数, ミス数) を返す
    fn build(root: &Path, cache_dir: &Path) -> (tempfile::TempDir, usize, usize) {
        let md_opts = MarkdownOptions::default();
        let site = yuzu_core::build_site_model(&root.join("content"), &[], &md_opts).unwrap();
        // 実運用の `yuzu build` 2 回はプロセスをまたぐので、毎回ディスクから読み直す
        let cache = BuildCache::load(cache_dir, "env1");
        cache.begin_build();
        let dist = tempfile::tempdir().unwrap();
        let session = IndexSession::default();
        build_search_index_with(
            &site,
            &md_opts,
            &params(root),
            dist.path(),
            &IndexCtx {
                cache: Some(&cache),
                outputs: None,
                session: Some(&session),
            },
        )
        .unwrap();
        let stats = cache.stats();
        cache.save().unwrap();
        (dist, stats.search_hits, stats.search_misses)
    }

    #[test]
    fn 参照先の変更で検索インデックスが更新される() {
        let (root, cache_dir) = fixture("fn oldSymbol() {}\n");
        let (dist, _, _) = build(root.path(), cache_dir.path());
        assert_eq!(
            search_dist(dist.path(), "oldSymbol", 10).unwrap().len(),
            1,
            "初回は引用の中身が索引される"
        );

        // ページの .md は触らず、参照先だけを書き換える
        write(root.path(), "src/api.rs", "fn newSymbol() {}\n");
        let (dist, _, misses) = build(root.path(), cache_dir.path());
        assert_eq!(misses, 1, "引用ページだけがキャッシュミスになる");
        assert_eq!(
            search_dist(dist.path(), "newSymbol", 10).unwrap().len(),
            1,
            "新しい語が引ける"
        );
        assert!(
            search_dist(dist.path(), "oldSymbol", 10)
                .unwrap()
                .is_empty(),
            "古い語は引けない"
        );
    }

    #[test]
    fn 参照先が変わらなければ全ページがキャッシュにヒットする() {
        let (root, cache_dir) = fixture("fn stable() {}\n");
        build(root.path(), cache_dir.path());
        let (_, hits, misses) = build(root.path(), cache_dir.path());
        // 依存ハッシュの導入で「毎回ミス」に退行していないことのガード
        assert_eq!((hits, misses), (2, 0));
    }

    #[test]
    fn index_code_が無効なら参照先を変えてもヒットする() {
        let (root, cache_dir) = fixture("fn a() {}\n");
        let md_opts = MarkdownOptions::default();
        let run = |expect_misses: usize| {
            let site =
                yuzu_core::build_site_model(&root.path().join("content"), &[], &md_opts).unwrap();
            let cache = BuildCache::load(cache_dir.path(), "env1");
            cache.begin_build();
            let dist = tempfile::tempdir().unwrap();
            // index_code は既定の false（引用は索引されない = 無効化する理由がない）
            build_search_index_with(
                &site,
                &md_opts,
                &IndexParams {
                    project_root: Some(root.path().to_path_buf()),
                    ..IndexParams::default()
                },
                dist.path(),
                &IndexCtx {
                    cache: Some(&cache),
                    outputs: None,
                    session: None,
                },
            )
            .unwrap();
            assert_eq!(cache.stats().search_misses, expect_misses);
            cache.save().unwrap();
        };
        run(2);
        write(root.path(), "src/api.rs", "fn b() {}\n");
        run(0);
    }
}

/// 公開 API を既定設定（`project_root: None`）で直接呼ぶ経路でも、
/// 出力先がシンボリックリンクならリンク先へ書き込まない
#[cfg(unix)]
#[test]
fn 出力先がシンボリックリンクなら書き出さない() {
    let content = tempfile::tempdir().unwrap();
    write(content.path(), "index.md", "---\ntitle: t\n---\n# t\n");
    let md_opts = MarkdownOptions::default();
    let site = yuzu_core::build_site_model(content.path(), &[], &md_opts).unwrap();

    let dir = tempfile::tempdir().unwrap();
    let outside = dir.path().join("outside");
    fs::create_dir_all(&outside).unwrap();
    let dist = dir.path().join("dist");
    std::os::unix::fs::symlink(&outside, &dist).unwrap();

    assert!(build_search_index(&site, &md_opts, &IndexParams::default(), &dist).is_err());
    assert!(
        !outside.join("_search").exists(),
        "リンク先へ書き出していない"
    );
}

mod 検索キャッシュと断片 {
    use super::*;
    use yuzu_core::BuildCache;

    /// 断片を参照するページを持つプロジェクト（**index_code は既定の false のまま**）
    fn fixture(fragment: &str) -> (tempfile::TempDir, tempfile::TempDir) {
        let root = tempfile::tempdir().unwrap();
        write(root.path(), "snippets/note.md", fragment);
        write(
            root.path(),
            "content/frag.md",
            "---\ntitle: 断片\n---\n# 断片\n\n```include file=\"snippets/note.md\"\n```\n",
        );
        write(
            root.path(),
            "content/plain.md",
            "---\ntitle: 素\n---\n# 素\n\n断片のないページ。\n",
        );
        let cache_dir = tempfile::tempdir().unwrap();
        (root, cache_dir)
    }

    fn params(root: &Path) -> IndexParams {
        IndexParams {
            // 断片は indexCode と無関係に索引される（per-spec ゲートの検証の要）
            index_code: false,
            project_root: Some(root.to_path_buf()),
            ..IndexParams::default()
        }
    }

    fn build(root: &Path, cache_dir: &Path) -> (tempfile::TempDir, usize, usize) {
        let md_opts = MarkdownOptions::default();
        let site = yuzu_core::build_site_model(&root.join("content"), &[], &md_opts).unwrap();
        let cache = BuildCache::load(cache_dir, "env1");
        cache.begin_build();
        let dist = tempfile::tempdir().unwrap();
        let session = IndexSession::default();
        build_search_index_with(
            &site,
            &md_opts,
            &params(root),
            dist.path(),
            &IndexCtx {
                cache: Some(&cache),
                outputs: None,
                session: Some(&session),
            },
        )
        .unwrap();
        let stats = cache.stats();
        cache.save().unwrap();
        (dist, stats.search_hits, stats.search_misses)
    }

    #[test]
    fn 断片の編集で検索インデックスが更新される() {
        let (root, cache_dir) = fixture("アボカドの注意書きです。\n");
        let (dist, _, _) = build(root.path(), cache_dir.path());
        assert_eq!(
            search_dist(dist.path(), "アボカド", 10).unwrap().len(),
            1,
            "断片の中身が索引される（indexCode 無効のまま）"
        );

        // ページの .md は触らず、断片だけを書き換える
        write(root.path(), "snippets/note.md", "バナナの注意書きです。\n");
        let (dist, _, misses) = build(root.path(), cache_dir.path());
        assert_eq!(misses, 1, "断片参照ページだけがキャッシュミスになる");
        assert_eq!(
            search_dist(dist.path(), "バナナ", 10).unwrap().len(),
            1,
            "断片の編集が検索へ反映される"
        );
        assert!(
            search_dist(dist.path(), "アボカド", 10).unwrap().is_empty(),
            "古い内容は消える"
        );
    }

    #[test]
    fn 断片が変わらなければキャッシュにヒットする() {
        let (root, cache_dir) = fixture("変わらない注意書き。\n");
        build(root.path(), cache_dir.path());
        let (_, hits, misses) = build(root.path(), cache_dir.path());
        assert_eq!(misses, 0, "全ページヒット");
        assert_eq!(hits, 2);
    }
}

#[test]
fn グループがナビ由来で_manifest_へ焼き込まれる() {
    let (_content, dist) = build_fixture();
    let manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(dist.path().join("_search/manifest.json")).unwrap())
            .unwrap();

    // fixture は index.md（ルート）と guide/ 配下 2 ページ。
    // ルート直下の単独ページはグループにしないので 1 つだけ。
    // `guide/index.md` が無いので表示名はディレクトリ名へフォールバックする
    // （scaffold もこの形なので、生名フォールバックは実運用で起きる）
    assert_eq!(manifest["groups"], serde_json::json!(["guide"]));

    // **doc_groups は docCount と同じ長さ**（エンジン側は寛容に縮退するので、
    // 長さの整合はここで縛る）
    let doc_groups = manifest["docGroups"].as_array().unwrap();
    assert_eq!(
        doc_groups.len(),
        manifest["docCount"].as_u64().unwrap() as usize
    );
    // 1 ページ = 複数 doc なので、ページ単位のグループがセクション数ぶん展開される
    assert!(doc_groups.iter().any(|v| v.as_u64() == Some(0)));
    // ルートページは未分類（u16::MAX）
    assert!(
        doc_groups
            .iter()
            .any(|v| v.as_u64() == Some(u16::MAX as u64))
    );
}

#[test]
fn セクション絞り込みは件数まで正確() {
    let (_content, dist) = build_fixture();

    let all = yuzu_index::search_dist_with_options(dist.path(), "検索", 10, &[]).unwrap();
    let guide =
        yuzu_index::search_dist_with_options(dist.path(), "検索", 10, &["guide".to_string()])
            .unwrap();

    assert!(guide.total > 0);
    assert!(guide.total <= all.total);
    assert_eq!(guide.total_unfiltered, all.total);
    // 絞り込むと guide 配下だけになる
    assert!(guide.results.iter().all(|r| r.url.starts_with("guide/")));
    assert!(
        guide
            .results
            .iter()
            .all(|r| r.section.as_deref() == Some("guide"))
    );
    // ファセット件数は絞り込みの有無で変わらない
    assert_eq!(guide.group_counts, all.group_counts);
}

#[test]
fn 未知のセクション名はエラーになる() {
    let (_content, dist) = build_fixture();
    let err =
        yuzu_index::search_dist_with_options(dist.path(), "検索", 10, &["存在しない".to_string()])
            .unwrap_err();
    let message = err.to_string();
    // 指定できる名前を添えて案内する（黙って 0 件にしない）
    assert!(message.contains("存在しない"), "{message}");
    assert!(message.contains("guide"), "{message}");
}

#[test]
fn グループが変わっても_content_hash_は変わらない() {
    // content_hash の対象は terms.fst ＋ シャード ＋ モデルだけ。ここへ manifest 由来の
    // 値（docGroups / groups）を混ぜると、区分名を変えただけでブラウザの OPFS
    // キャッシュが全破棄される。**本文・タイトルを完全に同じにして
    // ディレクトリ名だけ変える**ことで、グループの違いだけを取り出して比較する
    let hash_of = |dir_name: &str| -> String {
        let content = tempfile::tempdir().unwrap();
        write(content.path(), "index.md", "# top\n\n共通の本文。\n");
        write(
            content.path(),
            &format!("{dir_name}/a.md"),
            "---\ntitle: A\n---\n# A\n\n検索の本文。\n",
        );
        let md_opts = MarkdownOptions::default();
        let site = yuzu_core::build_site_model(content.path(), &[], &md_opts).unwrap();
        let dist = tempfile::tempdir().unwrap();
        build_search_index(&site, &md_opts, &IndexParams::default(), dist.path()).unwrap();
        let manifest: serde_json::Value =
            serde_json::from_slice(&fs::read(dist.path().join("_search/manifest.json")).unwrap())
                .unwrap();
        // 前提: グループ名は実際に違っている（比較が無意味になっていないこと）
        assert_eq!(manifest["groups"], serde_json::json!([dir_name]));
        manifest["contentHash"].as_str().unwrap().to_string()
    };
    assert_eq!(hash_of("alpha"), hash_of("beta"));
}

#[test]
fn 検索結果ページ自身は索引に載らない() {
    // `search.page` の合成ページは JS 前提の空ページなので、ヒットしても読むものが無い
    let content = tempfile::tempdir().unwrap();
    write(
        content.path(),
        "index.md",
        "---\ntitle: ホーム\n---\n# ようこそ\n\n検索できる本文です。\n",
    );
    let md_opts = MarkdownOptions {
        search_page: yuzu_core::SearchPageOptions {
            page: "search".to_string(),
            page_title: "検索".to_string(),
        },
        ..MarkdownOptions::default()
    };
    let site = yuzu_core::build_site_model(content.path(), &[], &md_opts).unwrap();
    assert_eq!(site.pages.len(), 2, "検索結果ページは pages には居る");

    let dist = tempfile::tempdir().unwrap();
    let stats = build_search_index(&site, &md_opts, &IndexParams::default(), dist.path()).unwrap();
    assert_eq!(stats.pages, 1, "索引対象は実ページのみ");

    let manifest: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(dist.path().join("_search/manifest.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(manifest["docCount"], 1);
    let fragment: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(dist.path().join("_search/fragment/0.json")).unwrap(),
    )
    .unwrap();
    assert_ne!(fragment["url"], "search/");
}

#[test]
fn 索引の_url_は_route_をパーセントエンコードした配信_url() {
    // search-hits.js は `base + url` をそのまま href にするので、
    // 本文・ナビと同じ変換点（encode_path）を通した値を焼く
    let content = tempfile::tempdir().unwrap();
    write(
        content.path(),
        "設計/概 要#1.md",
        "---\ntitle: 概要\n---\n# 概要\n\nこのページは設計の概要を説明します。\n",
    );
    let md_opts = MarkdownOptions::default();
    let site = yuzu_core::build_site_model(content.path(), &[], &md_opts).unwrap();
    let dist = tempfile::tempdir().unwrap();
    build_search_index(&site, &md_opts, &IndexParams::default(), dist.path()).unwrap();

    let results = search_dist(dist.path(), "設計", 10).unwrap();
    assert!(!results.is_empty());
    assert_eq!(
        results[0].url, "%E8%A8%AD%E8%A8%88/%E6%A6%82%20%E8%A6%81%231/",
        "results={results:?}"
    );
}
