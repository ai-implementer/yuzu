//! yuzu-config の統合テスト: JSONC 読み込み・上方探索・解決

use std::fs;

use yuzu_config::{find_project_root, load, write_resolved};

/// コメント・トレーリングカンマ・URL 内の `//` を含む JSONC が読めること
#[test]
fn コメント付き_jsonc_を読み込める() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join("yuzu.jsonc"),
        r#"{
  // サイト設定
  "site": {
    "title": "テストサイト",
    "baseUrl": "/docs", // 末尾スラッシュなしでも正規化される
    /* ブロックコメント */
    "description": "https://example.com/see-also", // 文字列内の // を壊さないこと
  },
  "dev": { "port": 8080, },
}"#,
    )
    .unwrap();

    let rc = load(dir.path()).unwrap();
    assert_eq!(rc.config.site.title, "テストサイト");
    assert_eq!(
        rc.config.site.description.as_deref(),
        Some("https://example.com/see-also")
    );
    assert_eq!(rc.base_url, "/docs/");
    assert_eq!(rc.config.dev.port, 8080);
    // 未指定キーはデフォルトが入る
    assert_eq!(rc.config.input.dir, "content");
    assert!(rc.config.markdown.mermaid.enabled);
}

#[test]
fn 空の設定でもデフォルトで解決できる() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("yuzu.jsonc"), "{}").unwrap();

    let rc = load(dir.path()).unwrap();
    assert_eq!(rc.base_url, "/");
    assert_eq!(rc.config.dev.port, 5173);
    assert_eq!(rc.content_dir, dir.path().join("content"));
    assert_eq!(rc.output_dir, dir.path().join("dist"));
    // theme/ と public/ は存在しないので None
    assert!(rc.theme_dir.is_none());
    assert!(rc.public_dir.is_none());
}

#[test]
fn dev_の_live_reload_と_open_を読み込める() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join("yuzu.jsonc"),
        r#"{ "dev": { "liveReload": false, "open": true } }"#,
    )
    .unwrap();

    let rc = load(dir.path()).unwrap();
    assert!(!rc.config.dev.live_reload);
    assert!(rc.config.dev.open);

    // 未指定時のデフォルト
    let dir2 = tempfile::tempdir().unwrap();
    fs::write(dir2.path().join("yuzu.jsonc"), "{}").unwrap();
    let rc2 = load(dir2.path()).unwrap();
    assert!(rc2.config.dev.live_reload);
    assert!(!rc2.config.dev.open);
}

#[test]
fn build_base_url_が_site_base_url_より優先される() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join("yuzu.jsonc"),
        r#"{ "site": { "baseUrl": "/a/" }, "build": { "baseUrl": "/b/" } }"#,
    )
    .unwrap();

    let rc = load(dir.path()).unwrap();
    assert_eq!(rc.base_url, "/b/");
}

#[test]
fn プロジェクトルートを上方探索できる() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("yuzu.jsonc"), "{}").unwrap();
    let nested = dir.path().join("content/guide/deep");
    fs::create_dir_all(&nested).unwrap();

    let root = find_project_root(&nested).unwrap();
    // tempdir はシンボリックリンクを含み得るので canonicalize して比較
    assert_eq!(
        root.canonicalize().unwrap(),
        dir.path().canonicalize().unwrap()
    );
}

#[test]
fn 見つからなければエラーになる() {
    let dir = tempfile::tempdir().unwrap();
    assert!(find_project_root(dir.path()).is_err());
}

#[test]
fn 解決済み設定を_yuzu_settings_json_に書き出す() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("yuzu.jsonc"), "{}").unwrap();

    let rc = load(dir.path()).unwrap();
    let path = write_resolved(&rc).unwrap();
    assert_eq!(path, dir.path().join(".yuzu/settings.json"));

    let text = fs::read_to_string(path).unwrap();
    let value: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(value["baseUrl"], "/");
    assert_eq!(value["config"]["dev"]["port"], 5173);
}

#[test]
fn 不正な_jsonc_はエラーになる() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("yuzu.jsonc"), "{ broken").unwrap();
    assert!(load(dir.path()).is_err());
}
