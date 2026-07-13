//! インクリメンタルビルドの統合テスト。
//! CLI（build_once）と同じオーケストレーションを再現して 2 回ビルドし、
//! キャッシュヒット・出力不変・孤児掃除を検証する

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;
use std::time::SystemTime;

use yuzu_core::{BuildCache, CacheStats, MarkdownOptions, OutputTracker, output};
use yuzu_render::{LiveReloadMode, RenderCtx, RenderParams, render_site};

fn write(dir: &Path, rel: &str, content: &str) {
    let path = dir.join(rel);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}

fn setup_project(root: &Path) {
    write(
        root,
        "yuzu.jsonc",
        r#"{ "site": { "title": "Incr Docs" }, "build": { "baseUrl": "/docs/" }, "search": { "enabled": false } }"#,
    );
    write(
        root,
        "content/index.md",
        "---\ntitle: ホーム\norder: 1\n---\n# ようこそ\n\n本文。\n",
    );
    write(
        root,
        "content/guide/a.md",
        "---\ntitle: ページA\n---\n# A\n\nAの本文。\n",
    );
    write(
        root,
        "content/guide/b.md",
        "---\ntitle: ページB\n---\n# B\n\nBの本文。\n",
    );
}

/// build_once 相当（render まで。検索は index テスト側で担保）
fn build_incremental(root: &Path, cache: &BuildCache) -> (BTreeSet<String>, CacheStats) {
    cache.begin_build();
    let rc = yuzu_config::load(root).unwrap();
    let md_opts = MarkdownOptions::default();
    let site = yuzu_core::build_site_model_cached(
        &rc.content_dir,
        &rc.config.input.ignore,
        &md_opts,
        Some(cache),
        false,
    )
    .unwrap();

    let routes: Vec<String> = site
        .pages
        .iter()
        .map(|p| format!("{}\t{}", p.rel.display(), p.route))
        .collect();
    cache.set_routes_key(BuildCache::sha256_hex_parts(&[routes
        .join("\n")
        .as_bytes()]));

    let tracker = OutputTracker::new(&rc.output_dir);
    render_site(&RenderParams {
        config: &rc,
        site: &site,
        live_reload: LiveReloadMode::None,
        ctx: RenderCtx {
            cache: Some(cache),
            outputs: Some(&tracker),
            shared: None,
        },
        git_dates: None,
    })
    .unwrap();

    let written = tracker.into_written();
    cache.save().unwrap();
    (written, cache.stats())
}

fn mtime(path: &Path) -> SystemTime {
    fs::metadata(path).unwrap().modified().unwrap()
}

#[test]
fn 二回目のビルドは全ヒットで出力とmtimeが不変() {
    let dir = tempfile::tempdir().unwrap();
    setup_project(dir.path());
    let cache_dir = dir.path().join(".yuzu/cache");

    let cache = BuildCache::load(&cache_dir, "env1");
    let (written1, stats1) = build_incremental(dir.path(), &cache);
    assert_eq!(stats1.body_misses, 3, "初回は全ミス");

    let index_html = dir.path().join("dist/index.html");
    let html1 = fs::read_to_string(&index_html).unwrap();
    let mtime1 = mtime(&index_html);

    std::thread::sleep(std::time::Duration::from_millis(20));

    // ディスクから読み直した新しい BuildCache でも全ヒットする（永続化の検証）
    let cache = BuildCache::load(&cache_dir, "env1");
    let (written2, stats2) = build_incremental(dir.path(), &cache);
    assert_eq!(stats2.body_misses, 0, "2 回目は全ヒット: {stats2:?}");
    assert_eq!(stats2.body_hits, 3);
    assert_eq!(written1, written2);
    assert_eq!(fs::read_to_string(&index_html).unwrap(), html1, "出力不変");
    assert_eq!(mtime(&index_html), mtime1, "mtime 温存");
}

#[test]
fn 一ページ編集はそのページだけ再計算される() {
    let dir = tempfile::tempdir().unwrap();
    setup_project(dir.path());
    let cache_dir = dir.path().join(".yuzu/cache");

    let cache = BuildCache::load(&cache_dir, "env1");
    build_incremental(dir.path(), &cache);
    let a_html = dir.path().join("dist/guide/a/index.html");
    let b_html = dir.path().join("dist/guide/b/index.html");
    let mtime_b = mtime(&b_html);

    std::thread::sleep(std::time::Duration::from_millis(20));
    write(
        dir.path(),
        "content/guide/a.md",
        "---\ntitle: ページA\n---\n# A\n\n編集後の本文。\n",
    );

    let cache = BuildCache::load(&cache_dir, "env1");
    let (_, stats) = build_incremental(dir.path(), &cache);
    assert_eq!(stats.body_misses, 1, "編集した A だけ再計算: {stats:?}");
    assert_eq!(stats.body_hits, 2);
    assert!(
        fs::read_to_string(&a_html)
            .unwrap()
            .contains("編集後の本文"),
        "A は更新される"
    );
    assert_eq!(
        mtime(&b_html),
        mtime_b,
        "B は書き込みスキップ（mtime 不変）"
    );
}

#[test]
fn ページ削除で孤児の出力が掃除される() {
    let dir = tempfile::tempdir().unwrap();
    setup_project(dir.path());
    let cache_dir = dir.path().join(".yuzu/cache");
    let manifest = cache_dir.join("output-manifest.json");

    let cache = BuildCache::load(&cache_dir, "env1");
    let (written1, _) = build_incremental(dir.path(), &cache);
    output::save_manifest(&manifest, &written1).unwrap();
    assert!(dir.path().join("dist/guide/b/index.html").exists());

    fs::remove_file(dir.path().join("content/guide/b.md")).unwrap();

    let cache = BuildCache::load(&cache_dir, "env1");
    let (written2, _) = build_incremental(dir.path(), &cache);
    let previous = output::load_manifest(&manifest).unwrap();
    let removed = output::remove_orphans(&dir.path().join("dist"), &previous, &written2).unwrap();
    output::save_manifest(&manifest, &written2).unwrap();

    // ページ 1 枚につき index.html とページ単位 .md の 2 ファイルが孤児になる
    assert_eq!(removed, 2);
    assert!(
        !dir.path().join("dist/guide/b").exists(),
        "孤児 HTML と空ディレクトリが消える"
    );
    assert!(
        !dir.path().join("dist/guide/b.md").exists(),
        "ページ単位 .md も消える"
    );
    assert!(dir.path().join("dist/guide/a/index.html").exists());
}

#[test]
fn env_key_が変わると全ページ再計算になる() {
    let dir = tempfile::tempdir().unwrap();
    setup_project(dir.path());
    let cache_dir = dir.path().join(".yuzu/cache");

    let cache = BuildCache::load(&cache_dir, "env1");
    build_incremental(dir.path(), &cache);

    // baseUrl 変更等 = envKey が変わる想定
    let cache = BuildCache::load(&cache_dir, "env2");
    let (_, stats) = build_incremental(dir.path(), &cache);
    assert_eq!(stats.body_misses, 3, "全ミス: {stats:?}");
}

#[test]
fn ページ追加は_routes_key_の変化で全_body_を再計算する() {
    let dir = tempfile::tempdir().unwrap();
    setup_project(dir.path());
    let cache_dir = dir.path().join(".yuzu/cache");

    let cache = BuildCache::load(&cache_dir, "env1");
    build_incremental(dir.path(), &cache);

    // ページ追加 → .md リンク解決の入力（routes）が変わる → body 全無効化
    write(
        dir.path(),
        "content/guide/c.md",
        "---\ntitle: ページC\n---\n# C\n\nCの本文。\n",
    );
    let cache = BuildCache::load(&cache_dir, "env1");
    let (_, stats) = build_incremental(dir.path(), &cache);
    assert_eq!(
        stats.body_misses, 4,
        "既存 3 + 新規 1 全て再計算: {stats:?}"
    );
}
