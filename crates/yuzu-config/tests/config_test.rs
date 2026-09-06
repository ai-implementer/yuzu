//! yuzu-config の統合テスト: TOML 読み込み・上方探索・解決

use std::fs;

use yuzu_config::{ConfigError, find_project_root, load};

/// 一時ディレクトリに `yuzu.toml` を置く
fn project(text: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("yuzu.toml"), text).unwrap();
    dir
}

/// コメント・複数行配列・URL を含む TOML が読めること
#[test]
fn コメント付き_toml_を読み込める() {
    let dir = project(
        r#"# サイト設定
[site]
title = "テストサイト"
base_url = "/docs" # 末尾スラッシュなしでも正規化される
description = "https://example.com/see-also" # 文字列内の # を壊さないこと

[dev]
port = 8080

[search]
synonyms = [
  ["ログイン", "サインイン"], # 末尾カンマ可
]
"#,
    );

    let rc = load(dir.path()).unwrap();
    assert_eq!(rc.config.site.title, "テストサイト");
    assert_eq!(
        rc.config.site.description.as_deref(),
        Some("https://example.com/see-also")
    );
    assert_eq!(rc.base_url, "/docs/");
    assert_eq!(rc.config.dev.port, 8080);
    assert_eq!(
        rc.config.search.synonyms,
        vec![vec!["ログイン", "サインイン"]]
    );
    // 未指定キーはデフォルトが入る
    assert_eq!(rc.config.input.dir, "content");
    assert!(rc.config.markdown.mermaid.enabled);
}

#[test]
fn 空の設定でもデフォルトで解決できる() {
    let dir = project("");

    let rc = load(dir.path()).unwrap();
    assert_eq!(rc.base_url, "/");
    assert_eq!(rc.config.dev.port, 5173);
    assert_eq!(rc.content_dir, dir.path().join("content"));
    assert_eq!(rc.output_dir, dir.path().join("dist"));
    // theme/ と public/ は存在しないので None
    assert!(rc.theme_dir.is_none());
    assert!(rc.public_dir.is_none());
    assert!(rc.diagnostics.is_empty());
}

#[test]
fn dotted_key_でも書ける() {
    let dir = project("dev.live_reload = false\ndev.open = true\nsite.title = \"t\"\n");
    let rc = load(dir.path()).unwrap();
    assert!(!rc.config.dev.live_reload);
    assert!(rc.config.dev.open);
    assert_eq!(rc.config.site.title, "t");
}

#[test]
fn dev_の_live_reload_と_open_を読み込める() {
    let dir = project("[dev]\nlive_reload = false\nopen = true\n");

    let rc = load(dir.path()).unwrap();
    assert!(!rc.config.dev.live_reload);
    assert!(rc.config.dev.open);

    // 未指定時のデフォルト
    let dir2 = project("");
    let rc2 = load(dir2.path()).unwrap();
    assert!(rc2.config.dev.live_reload);
    assert!(!rc2.config.dev.open);
}

#[test]
fn search_設定を読み込める() {
    let dir = project(
        r#"[search]
enabled = false
dictionary = "models/custom.model.zst"

[search.typo_tolerance]
max_edits = 0

[search.shard]
max_terms_per_shard = 4096
"#,
    );

    let rc = load(dir.path()).unwrap();
    assert!(!rc.config.search.enabled);
    assert_eq!(
        rc.config.search.dictionary.as_deref(),
        Some("models/custom.model.zst")
    );
    assert_eq!(rc.config.search.typo_tolerance.max_edits, 0);
    assert_eq!(rc.config.search.shard.max_terms_per_shard, 4096);

    // 未指定時のデフォルト
    let dir2 = project("");
    let rc2 = load(dir2.path()).unwrap();
    assert!(rc2.config.search.enabled);
    assert!(rc2.config.search.dictionary.is_none());
    assert!(rc2.config.search.typo_tolerance.enabled);
    assert_eq!(rc2.config.search.typo_tolerance.max_edits, 1);
    assert_eq!(rc2.config.search.shard.max_terms_per_shard, 16384);
}

#[test]
fn mermaid_backend_を読み込める() {
    use yuzu_config::MermaidBackend;

    let dir = project("[markdown.mermaid]\nbackend = \"ssr\"\n");
    let rc = load(dir.path()).unwrap();
    assert_eq!(rc.config.markdown.mermaid.backend, MermaidBackend::Ssr);

    // 未指定は client
    let dir2 = project("");
    let rc2 = load(dir2.path()).unwrap();
    assert_eq!(rc2.config.markdown.mermaid.backend, MermaidBackend::Client);

    // 不正値は設定エラー（位置付き）
    let dir3 = project("[markdown.mermaid]\nbackend = \"server\"\n");
    let err = load(dir3.path()).expect_err("不正値は拒否する");
    assert!(matches!(err, ConfigError::Invalid { .. }), "{err:?}");
    assert!(err.to_string().contains(":2:11:"), "{err}");
}

#[test]
fn math_設定を読み込める() {
    let dir = project("[markdown.math]\nenabled = false\n");
    let rc = load(dir.path()).unwrap();
    assert!(!rc.config.markdown.math.enabled);

    // 未指定時のデフォルトは有効
    let dir2 = project("");
    assert!(load(dir2.path()).unwrap().config.markdown.math.enabled);
}

#[test]
fn site_logo_を読み込める() {
    let dir = project("[site]\nlogo = \"/images/yuzu-logo.svg\"\n");
    let rc = load(dir.path()).unwrap();
    assert_eq!(
        rc.config.site.logo.as_deref(),
        Some("/images/yuzu-logo.svg")
    );

    // 未指定時は None（テーマ既定の絵文字ロゴ）
    let dir2 = project("");
    assert!(load(dir2.path()).unwrap().config.site.logo.is_none());
}

#[test]
fn llms_設定を読み込める() {
    let dir = project("[llms]\nfull = false\n");

    let rc = load(dir.path()).unwrap();
    assert!(rc.config.llms.enabled);
    assert!(!rc.config.llms.full);

    // 未指定時のデフォルトは両方 true
    let dir2 = project("");
    let rc2 = load(dir2.path()).unwrap();
    assert!(rc2.config.llms.enabled);
    assert!(rc2.config.llms.full);
}

#[test]
fn lint_設定を読み込める() {
    let dir =
        project("[lint]\nmax_directory_depth = 1\n\n[lint.terms]\n\"サーバー\" = [\"サーバ\"]\n");

    let rc = load(dir.path()).unwrap();
    assert_eq!(rc.config.lint.max_directory_depth, Some(1));
    assert_eq!(
        rc.config.lint.terms.get("サーバー").map(Vec::as_slice),
        Some(&["サーバ".to_string()][..])
    );

    // 未指定時のデフォルトは無制限（None）
    let dir2 = project("");
    let rc2 = load(dir2.path()).unwrap();
    assert!(rc2.config.lint.max_directory_depth.is_none());
}

#[test]
fn theme_の_css_変数を読み込める() {
    let dir = project(
        "[theme.css_vars]\naccent = \"#0a6cff\"\n\n[theme.css_vars_dark]\naccent = \"#7fb2ff\"\n",
    );
    let rc = load(dir.path()).unwrap();
    assert_eq!(rc.config.theme.css_vars["accent"], "#0a6cff");
    assert_eq!(rc.config.theme.css_vars_dark["accent"], "#7fb2ff");
}

#[test]
fn build_base_url_が_site_base_url_より優先される() {
    let dir = project("[site]\nbase_url = \"/a/\"\n[build]\nbase_url = \"/b/\"\n");

    let rc = load(dir.path()).unwrap();
    assert_eq!(rc.base_url, "/b/");
}

#[test]
fn プロジェクトルートを上方探索できる() {
    let dir = project("");
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
    let err = find_project_root(dir.path()).expect_err("マーカーが無い");
    assert!(err.to_string().contains("yuzu.toml"), "{err}");
}

#[test]
fn 旧来の_jsonc_はマーカーにならない() {
    // v0.14 で yuzu.jsonc から yuzu.toml へ移行した。互換読み込みは作らない
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("yuzu.jsonc"), "{}").unwrap();
    assert!(find_project_root(dir.path()).is_err());
}

#[test]
fn 不正な_toml_は位置付きの構文エラーになる() {
    let dir = project("[site\ntitle = \"a\"\n");
    let err = load(dir.path()).expect_err("構文エラー");
    assert!(
        matches!(err, ConfigError::Syntax { line: 1, .. }),
        "{err:?}"
    );
    assert!(err.to_string().contains("yuzu.toml:1:"), "{err}");
}

#[test]
fn 重複キーは構文エラーになる() {
    let dir = project("[site]\ntitle = \"A\"\ntitle = \"B\"\n");
    let err = load(dir.path()).expect_err("TOML の重複キーは拒否する");
    assert!(
        matches!(err, ConfigError::Syntax { line: 3, .. }),
        "{err:?}"
    );
    assert!(err.to_string().contains("重複"), "{err}");
}

/// 旧形式（camelCase）のキーは無言で無視せず、対応キーを案内して止める
#[test]
fn 旧_camelcase_キーは設定エラーになり正しいキーが案内される() {
    let dir = project("[site]\nbaseUrl = \"/docs/\"\n");
    let err = load(dir.path()).expect_err("未知キーは Deny");
    let ConfigError::Invalid { issues, .. } = &err else {
        panic!("Invalid を期待: {err:?}");
    };
    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0].key_path, "site.baseUrl");
    assert_eq!((issues[0].line, issues[0].col), (2, 1));
    assert!(
        issues[0].message.contains("base_url"),
        "正しい snake_case キーが対応キー一覧に出る: {}",
        issues[0].message
    );
    assert!(err.to_string().contains("yuzu.toml:2:1:"), "{err}");
}

#[test]
fn 複数の問題は全件まとめて報告される() {
    let dir = project("[site]\ntitel = \"a\"\n[dev]\nport = \"x\"\n");
    let err = load(dir.path()).expect_err("2 件の問題");
    let ConfigError::Invalid { issues, .. } = &err else {
        panic!("Invalid を期待: {err:?}");
    };
    assert_eq!(issues.len(), 2, "{issues:?}");
    assert!(err.to_string().contains("（2 件）"), "{err}");
}

#[test]
fn インラインテーブルで書いた設定も読める() {
    let dir = project("[lint]\nterms = { \"サーバ\" = [\"サーバー\"] }\n");
    let resolved = load(dir.path()).expect("インラインテーブルは 0.2 で読める");
    assert_eq!(resolved.config.lint.terms["サーバ"], ["サーバー"]);
}

/// `output.clean` は既定 true で出力ディレクトリを丸ごと再帰削除するため、
/// ルート外・ルート自身を指す値は load で弾く
#[test]
fn output_dir_にルート外を指す値は拒否される() {
    for bad in [
        "/etc",
        "/tmp/dangerous",
        "../site",
        "a/../../x",
        "",
        ".",
        "./",
    ] {
        let dir = project(&format!("[output]\ndir = \"{bad}\"\n"));
        let err = load(dir.path()).expect_err(&format!("拒否されるべき: {bad:?}"));
        assert!(
            err.to_string().contains("output.dir"),
            "エラー文言に output.dir が含まれること: {err}"
        );
    }
}

/// 保護対象（原稿・public・theme・.yuzu）と**重なる**出力先はすべて拒否する。
/// 片方向比較や未正規化の比較だと、子ディレクトリや `..` 入りの値がすり抜ける
#[test]
fn output_dir_が原稿やキャッシュを飲み込む指定は拒否される() {
    for bad in [
        "content",
        ".yuzu",
        "public",
        "theme",
        // 保護対象の子（片方向比較のすり抜け）
        "content/sub",
        ".yuzu/cache",
    ] {
        let dir = project(&format!("[output]\ndir = \"{bad}\"\n"));
        assert!(load(dir.path()).is_err(), "拒否されるべき: {bad:?}");
    }
}

#[test]
fn output_dir_は先頭のカレント参照を吸収する() {
    let dir = project("[output]\ndir = \"./public_html\"\n");

    let rc = load(dir.path()).unwrap();
    assert_eq!(rc.output_dir, dir.path().join("public_html"));
}

/// input.dir は読むだけで削除経路がないため、ルート外でも警告に留める
#[test]
fn input_dir_がルート外なら診断を出すが読み込みは成功する() {
    let dir = project("[input]\ndir = \"../shared-docs\"\n");

    let rc = load(dir.path()).unwrap();
    let diag = rc
        .diagnostics
        .iter()
        .find(|d| d.rule == "config-path-outside-root")
        .expect("診断が出ること");
    assert_eq!(diag.key_path, "input.dir");
    assert_eq!((diag.line, diag.col), (2, 1), "キーの位置を指すこと");
}

/// input.dir 側に `..` が入っていても、出力先との重なりを検出できること
/// （`root.join()` したままだと文字列前方一致にならない）
#[test]
fn 正規化前は重ならない_input_dir_でも拒否される() {
    let dir =
        project("[input]\ndir = \"a/../dist/content\"\n[output]\ndir = \"dist\"\nclean = true\n");

    let err = load(dir.path()).expect_err("原稿を飲み込む出力先は拒否する");
    assert!(err.to_string().contains("原稿"), "{err}");
}

#[test]
fn lint_rules_の部分マップを読み込める() {
    let dir = project("[lint.rules]\nkatakana-choon = false\n");
    let rc = load(dir.path()).unwrap();
    assert_eq!(rc.config.lint.rules.get("katakana-choon"), Some(&false));
    // 部分マップは既定を丸ごと置き換える（他 ID は不在 = 有効の規約）
    assert_eq!(rc.config.lint.rules.len(), 1);
    assert!(rc.diagnostics.is_empty(), "{:?}", rc.diagnostics);
}

#[test]
fn lint_rules_未指定なら既定の全ルール有効マップになる() {
    let dir = project("");
    let rc = load(dir.path()).unwrap();
    let keys: Vec<&str> = rc.config.lint.rules.keys().map(String::as_str).collect();
    assert_eq!(keys, yuzu_config::DISABLEABLE_RULES, "既定キーは一覧と一致");
    assert!(rc.config.lint.rules.values().all(|&enabled| enabled));
}

#[test]
fn lint_rules_の_true_指定は_no_op_として受理する() {
    let dir = project("[lint.rules]\nterm-variant = true\n");
    let rc = load(dir.path()).unwrap();
    assert_eq!(rc.config.lint.rules.get("term-variant"), Some(&true));
    assert!(rc.diagnostics.is_empty(), "{:?}", rc.diagnostics);
}

#[test]
fn lint_rules_の値が_bool_以外なら設定エラーになる() {
    let dir = project("[lint.rules]\nterm-variant = \"off\"\n");
    let err = load(dir.path()).expect_err("型不一致");
    assert!(matches!(err, ConfigError::Invalid { .. }), "{err:?}");
    assert!(err.to_string().contains("lint.rules.term-variant"), "{err}");
}

/// ルール ID のタイポ・旧形式のキー・無効化不可の ID は位置付きの設定エラー
/// （黙って受理すると「無効化したつもりが効いていない」事故になる）
#[test]
fn lint_rules_の未知の_id_は設定エラーになる() {
    for (text, bad) in [
        ("[lint.rules]\nkatakanaChoon = false\n", "katakanaChoon"),
        ("[lint.rules]\nbroken-link = false\n", "broken-link"),
    ] {
        let dir = project(text);
        let err = load(dir.path()).expect_err(bad);
        let ConfigError::Invalid { issues, .. } = &err else {
            panic!("Invalid を期待: {err:?}");
        };
        assert_eq!(issues.len(), 1, "{issues:?}");
        assert_eq!(issues[0].key_path, format!("lint.rules.{bad}"));
        assert_eq!((issues[0].line, issues[0].col), (2, 1));
        assert!(
            issues[0].message.contains("katakana-choon"),
            "正しい kebab-case ID が一覧に出る: {}",
            issues[0].message
        );
    }
}

#[test]
fn 正規化出力は読み戻すと同じ設定になる() {
    let dir = project(
        "[site]\ntitle = \"t\"\nbase_url = \"/docs/\"\n[search]\nsynonyms = [[\"a\", \"b\"]]\n[lint.terms]\n\"サーバー\" = [\"サーバ\"]\n",
    );
    let rc = load(dir.path()).unwrap();
    let toml = rc.config.to_toml();

    let dir2 = project(&toml);
    let rc2 = load(dir2.path()).unwrap();
    assert_eq!(rc2.config.to_toml(), toml, "往復で同一バイト列");
    assert_eq!(rc2.config.site.title, "t");
    assert_eq!(rc2.config.search.synonyms, vec![vec!["a", "b"]]);
    assert_eq!(rc2.base_url, "/docs/");
}
