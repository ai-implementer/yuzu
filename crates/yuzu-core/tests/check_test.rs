//! check_links（内部リンク・アンカーの静的検査）の統合テスト

use std::fs;
use std::path::Path;

use yuzu_core::{Diagnostic, MarkdownOptions, build_source_pages, check_links};

/// content/・public/ を持つ一時プロジェクトで check_links を実行する
fn check(files: &[(&str, &str)], public: &[&str]) -> Vec<Diagnostic> {
    let dir = tempfile::tempdir().unwrap();
    let content = dir.path().join("content");
    let public_dir = dir.path().join("public");
    for (rel, source) in files {
        let path = content.join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, source).unwrap();
    }
    for rel in public {
        let path = public_dir.join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, b"x").unwrap();
    }
    let opts = MarkdownOptions::default();
    let pages = build_source_pages(&content, &[], &opts).unwrap();
    check_links(&pages, Some(&public_dir), &content, &opts).unwrap()
}

fn rules(diags: &[Diagnostic]) -> Vec<&str> {
    diags.iter().map(|d| d.rule).collect()
}

#[test]
fn 存在しない_md_へのリンクを行番号付きで報告() {
    let diags = check(
        &[
            (
                "index.md",
                "# t\n\n[ある](guide/a.md)\n\n[ない](guide/missing.md)\n",
            ),
            ("guide/a.md", "# a\n"),
        ],
        &[],
    );
    assert_eq!(rules(&diags), ["broken-link"]);
    assert!(
        diags[0].message.contains("guide/missing.md"),
        "{}",
        diags[0].message
    );
    assert_eq!(diags[0].span.unwrap().start_line, 5);
    assert_eq!(diags[0].rel, Path::new("index.md"));
}

#[test]
fn 未参照の脚注定義内のリンクも検査される() {
    // 既定パースだと未参照定義は AST から消えて検査をすり抜ける。
    // fmt が未参照定義を温存する以上、check も同じ AST を見る（keep_footnotes）
    let diags = check(&[("index.md", "# t\n\n[^x]: [壊れ](missing.md)\n")], &[]);
    assert_eq!(rules(&diags), ["broken-link"]);
    assert!(
        diags[0].message.contains("missing.md"),
        "{}",
        diags[0].message
    );
}

#[test]
fn 相対リンクは_親ディレクトリ基準で解決される() {
    let diags = check(
        &[
            (
                "guide/a.md",
                "# a\n\n[上へ](../index.md)\n\n[隣へ](./b.md)\n",
            ),
            ("guide/b.md", "# b\n"),
            ("index.md", "# t\n"),
        ],
        &[],
    );
    assert!(diags.is_empty(), "{diags:?}");
}

#[test]
fn アンカーはスラッグで照合する() {
    let files: &[(&str, &str)] = &[
        (
            "index.md",
            "# t\n\n[ok](a.md#使い方)\n\n[dup](a.md#使い方-1)\n\n[ng](a.md#無い見出し)\n\n[self](#節)\n\n## 節\n",
        ),
        ("a.md", "# a\n\n## 使い方\n\n本文\n\n## 使い方\n"),
    ];
    let diags = check(files, &[]);
    assert_eq!(rules(&diags), ["broken-anchor"]);
    assert!(
        diags[0].message.contains("無い見出し"),
        "{}",
        diags[0].message
    );
}

#[test]
fn percent_エンコードされたアンカーも照合できる() {
    // 「見出し」の UTF-8 percent エンコード
    let diags = check(
        &[
            (
                "index.md",
                "# t\n\n[ok](a.md#%E8%A6%8B%E5%87%BA%E3%81%97)\n",
            ),
            ("a.md", "# a\n\n## 見出し\n"),
        ],
        &[],
    );
    assert!(diags.is_empty(), "{diags:?}");
}

#[test]
fn ルート絶対パスは_public_と_route_に照合() {
    let files: &[(&str, &str)] = &[
        (
            "index.md",
            "# t\n\n![ロゴ](/images/logo.svg)\n\n[ガイド](/guide/)\n\n[末尾なし](/guide)\n\n[生成物](/llms.txt)\n\n[検索](/_search/manifest.json)\n\n[ない](/nai.png)\n",
        ),
        ("guide/index.md", "# g\n"),
    ];
    let diags = check(files, &["images/logo.svg"]);
    assert_eq!(rules(&diags), ["broken-link"]);
    assert!(
        diags[0].message.contains("/nai.png"),
        "{}",
        diags[0].message
    );
}

#[test]
fn draft_ページへのリンクは壊れ扱い() {
    let diags = check(
        &[
            ("index.md", "# t\n\n[wip](wip.md)\n"),
            ("wip.md", "---\ndraft: true\n---\n# wip\n"),
        ],
        &[],
    );
    assert_eq!(rules(&diags), ["broken-link"]);
    assert!(diags[0].message.contains("draft"), "{}", diags[0].message);
}

#[test]
fn draft_ページの中のリンクも検査される() {
    let diags = check(
        &[
            ("index.md", "# t\n"),
            (
                "wip.md",
                "---\ndraft: true\n---\n# wip\n\n[ない](missing.md)\n",
            ),
        ],
        &[],
    );
    assert_eq!(rules(&diags), ["broken-link"]);
    assert_eq!(diags[0].rel, Path::new("wip.md"));
}

#[test]
fn 外部_url_は検査しない() {
    let diags = check(
        &[(
            "index.md",
            "# t\n\n[外](https://example.com/missing)\n\n<https://example.com/a>\n\n[メール](mailto:a@example.com)\n",
        )],
        &[],
    );
    assert!(diags.is_empty(), "{diags:?}");
}

#[test]
fn 画像の相対参照は実在チェック() {
    let diags = check(
        &[
            (
                "guide/a.md",
                "# a\n\n![ある](./fig.png)\n\n![ない](./nai.png)\n",
            ),
            ("guide/fig.png", ""),
        ], // content/ 内の隣接ファイル
        &[],
    );
    assert_eq!(rules(&diags), ["broken-link"]);
    assert!(diags[0].message.contains("nai.png"), "{}", diags[0].message);
    // ディレクトリ風リンク（拡張子なし）は検査対象外
    let diags = check(&[("index.md", "# t\n\n[dir](guide/)\n")], &[]);
    assert!(diags.is_empty(), "{diags:?}");
}
