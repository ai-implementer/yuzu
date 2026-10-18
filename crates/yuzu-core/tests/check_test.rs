//! check_links（内部リンク・アンカーの静的検査）の統合テスト

use std::fs;
use std::path::Path;

use yuzu_core::{
    Diagnostic, LinkReport, MarkdownOptions, OutputSwitches, build_source_pages, check_links,
};

/// 既定の設定で出す生成物（llms・ハイライト・検索あり。base_url がパスなので sitemap なし）
const DEFAULT_OUTPUTS: OutputSwitches = OutputSwitches {
    llms: true,
    llms_full: true,
    sitemap: false,
    highlight: true,
    search: true,
    build_id: false,
};

/// content/・public/ を持つ一時プロジェクトで check_links を実行し、内部リンクの診断を返す
fn check(files: &[(&str, &str)], public: &[&str]) -> Vec<Diagnostic> {
    report(files, public).diags
}

/// 診断と外部リンク一覧の両方
fn report(files: &[(&str, &str)], public: &[&str]) -> LinkReport {
    report_with(files, public, &DEFAULT_OUTPUTS)
}

/// 出す生成物を指定して検査する
fn report_with(files: &[(&str, &str)], public: &[&str], outputs: &OutputSwitches) -> LinkReport {
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
    check_links(
        &pages,
        Some(&public_dir),
        &content,
        &[],
        false,
        outputs,
        &opts,
    )
    .unwrap()
}

fn rules(diags: &[Diagnostic]) -> Vec<&str> {
    diags.iter().map(|d| d.rule.id).collect()
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

/// ビルドの生成物へのリンクは、今回出すものだけを通す（10-12 レビューの R1）。
/// 404.html は必ず出すのに壊れたリンクにしていた・llms を止めても `/llms-full.txt` が通っていた
#[test]
fn 生成物へのリンクは今回出すものだけを通す() {
    let files: &[(&str, &str)] = &[(
        "index.md",
        "# t\n\n[404](/404.html)\n\n[full](/llms-full.txt)\n\n[map](/sitemap.xml)\n\n[css](/_assets/css/syntect.css)\n\n[検索](/_search/manifest.json)\n",
    )];
    let broken = |outputs: &OutputSwitches| -> Vec<String> {
        report_with(files, &[], outputs)
            .diags
            .iter()
            .map(|d| {
                let (_, rest) = d.message.split_once('`').unwrap();
                rest.split('`').next().unwrap().to_string()
            })
            .collect()
    };
    // 既定: base_url がパスなので sitemap.xml だけ出さない
    assert_eq!(broken(&DEFAULT_OUTPUTS), ["/sitemap.xml"]);
    // 全部止める（404.html は常に出す。syntect.css は常に出す _assets/ の中で、
    // theme/static/ が同じパスに書き出せるので通す = PR #52 のレビュー指摘）
    let off = OutputSwitches {
        llms: false,
        llms_full: false,
        sitemap: false,
        highlight: false,
        search: false,
        build_id: false,
    };
    assert_eq!(
        broken(&off),
        ["/llms-full.txt", "/sitemap.xml", "/_search/manifest.json"]
    );
    // public/ に置いたファイルは止めていても有効なリンク先
    let with_public = report_with(files, &["llms-full.txt"], &off).diags;
    assert!(
        with_public
            .iter()
            .all(|d| !d.message.contains("/llms-full.txt")),
        "{with_public:?}"
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

/// クエリに URL を含むサイト内リンクは外部 URL ではない（PR #66 のレビュー指摘。文字列のどこかに
/// `://` があると外部扱いにして、リンク先を照合していなかった）
#[test]
fn クエリに_url_を含むサイト内リンクも照合する() {
    let diags = check(
        &[
            (
                "index.md",
                "# t\n\n[ある](/guide/?q=https://example.com/)\n\n[ない](/nai/?q=https://example.com/)\n",
            ),
            ("guide/index.md", "# g\n"),
        ],
        &[],
    );
    assert_eq!(rules(&diags), ["broken-link"]);
    assert!(diags[0].message.contains("/nai/"), "{}", diags[0].message);
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

/// 同伴アセットは build がコピーするファイルに照合する。隠しファイル・`input.ignore` の
/// 対象は build がコピーしないので、ディスクにあっても公開サイトでは切れる（Phase 85）
#[test]
fn 画像は_build_がコピーするファイルに照合する() {
    let dir = tempfile::tempdir().unwrap();
    let content = dir.path().join("content");
    for (rel, source) in [
        (
            "index.md",
            "# t\n\n![ある](fig.png)\n\n![隠し](.hidden.png)\n\n![除外](drafts/x.png)\n",
        ),
        ("fig.png", ""),
        (".hidden.png", ""),
        ("drafts/x.png", ""),
    ] {
        let path = content.join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, source).unwrap();
    }
    let opts = MarkdownOptions::default();
    let ignore = vec!["drafts/**".to_string()];
    let pages = build_source_pages(&content, &ignore, &opts).unwrap();
    let diags = check_links(
        &pages,
        None,
        &content,
        &ignore,
        false,
        &DEFAULT_OUTPUTS,
        &opts,
    )
    .unwrap()
    .diags;
    let messages: Vec<&str> = diags.iter().map(|d| d.message.as_str()).collect();
    assert_eq!(
        rules(&diags),
        ["broken-link", "broken-link"],
        "{messages:?}"
    );
    assert!(messages[0].contains(".hidden.png"), "{messages:?}");
    assert!(messages[1].contains("drafts/x.png"), "{messages:?}");
}

/// `--drafts` のビルド（dev --drafts 等）では draft も出力されるので、リンク先として有効
#[test]
fn drafts_を含むビルドでは_draft_へのリンクは切れていない() {
    let dir = tempfile::tempdir().unwrap();
    let content = dir.path().join("content");
    fs::create_dir_all(&content).unwrap();
    fs::write(
        content.join("index.md"),
        "# t\n\n[wip](wip.md)\n\n[/wip](/wip/)\n",
    )
    .unwrap();
    fs::write(content.join("wip.md"), "---\ndraft: true\n---\n# wip\n").unwrap();
    let opts = MarkdownOptions::default();
    let pages = build_source_pages(&content, &[], &opts).unwrap();
    let published =
        check_links(&pages, None, &content, &[], false, &DEFAULT_OUTPUTS, &opts).unwrap();
    assert_eq!(rules(&published.diags), ["broken-link", "broken-link"]);
    let with_drafts =
        check_links(&pages, None, &content, &[], true, &DEFAULT_OUTPUTS, &opts).unwrap();
    assert!(with_drafts.diags.is_empty(), "{:?}", with_drafts.diags);
}

#[test]
fn エンコード済みで書いたリンクはデコードして照合する() {
    // ブラウザで辿れるリンクは検査も通る（`[x](my%20page.md)` は CommonMark で
    // 空白を書く唯一の素の形。従来は `my%20page.md` の実在検査に失敗していた）
    let diags = check(
        &[
            (
                "index.md",
                "# t\n\n\
                 [空白](my%20page.md)\n\n\
                 [日本語](%E8%A8%AD%E8%A8%88/%E6%A6%82%E8%A6%81.md#%E6%A6%82%E8%A6%81)\n\n\
                 [絶対](/%E8%A8%AD%E8%A8%88/%E6%A6%82%E8%A6%81/)\n\n\
                 [パーセント](a%2523b.md)\n\n\
                 ![画像](img/my%20image.png)\n\n\
                 [public](/files/my%20file.pdf)\n",
            ),
            ("my page.md", "# p\n"),
            ("設計/概要.md", "# 概要\n"),
            ("a%23b.md", "# percent\n"),
            ("img/my image.png", ""),
        ],
        &["files/my file.pdf"],
    );
    assert!(diags.is_empty(), "{diags:?}");
}

#[test]
fn デコードしても無いものは壊れ扱い() {
    let diags = check(
        &[(
            "index.md",
            "# t\n\n[無い](no%20page.md)\n\n[絶対](/%E7%84%A1%E3%81%84/)\n",
        )],
        &[],
    );
    assert_eq!(rules(&diags), ["broken-link", "broken-link"]);
}

#[test]
fn 絶対_相対の分類はデコード前の文字列で行う() {
    // render（UrlResolver::rewrite）は元の文字列が `/` 始まりかで分類してから
    // デコードする。`%2Flogo.png` はデコードすると `/logo.png` だが相対参照として
    // `guide/logo.png` へ解決されるので、check も同じ分類でないと誤報・誤通過になる
    let diags = check(
        &[
            ("guide/page.md", "# p\n\n![x](%2Flogo.png)\n"),
            ("guide/logo.png", ""),
        ],
        &["logo.png"],
    );
    assert!(
        diags.is_empty(),
        "相対として guide/logo.png を見る: {diags:?}"
    );
    let diags = check(
        &[("guide/page.md", "# p\n\n![x](%2Flogo.png)\n")],
        &["logo.png"],
    );
    assert_eq!(
        rules(&diags),
        ["broken-link"],
        "public/logo.png があっても相対参照なので壊れ扱い"
    );
}

#[test]
fn 外部リンクは_http_と_https_だけを位置付きで返す() {
    // core はネットワークに触れない = 診断は出さず、cli の opt-in 検査に渡す一覧だけ返す
    let LinkReport { diags, external } = report(
        &[
            (
                "index.md",
                "# t\n\n[a](https://example.com/x)\n\n![img](HTTP://example.com/i.png)\n\n\
                 [mail](mailto:a@example.com)\n\n[ftp](ftp://example.com/f)\n\n[内部](guide/a.md)\n",
            ),
            ("guide/a.md", "# a\n\n<https://example.com/x>\n"),
        ],
        &[],
    );
    assert!(diags.is_empty(), "{diags:?}");
    let seen: Vec<(String, String, usize, bool)> = external
        .iter()
        .map(|e| {
            (
                // `/` 区切りで比べる（`to_string_lossy` だと Windows で `guide\a.md` になる。Phase 87）
                yuzu_core::urlpath::rel_to_slash(&e.rel),
                e.url.clone(),
                e.span.start_line,
                e.is_image,
            )
        })
        .collect();
    assert_eq!(
        seen,
        [
            (
                "guide/a.md".to_string(),
                "https://example.com/x".to_string(),
                3,
                false
            ),
            (
                "index.md".to_string(),
                "https://example.com/x".to_string(),
                3,
                false
            ),
            (
                "index.md".to_string(),
                "HTTP://example.com/i.png".to_string(),
                5,
                true
            ),
        ],
        "mailto / ftp / 内部リンクは含まれない。同じ URL の重複はそのまま（cli が畳む）"
    );
}
