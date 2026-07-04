//! mermaid.js 公式ドキュメント例文コーパスの互換性テスト。
//!
//! - `corpus/sequence/*.mmd` — 全件が受理され（構文互換）、well-formed な
//!   SVG になること。代表例は insta スナップショットで回帰検出
//! - `corpus/fallback/*.mmd` — 未対応構文・図種が `Err` かつ
//!   `is_unsupported()` でフォールバック判定できること

use std::fs;
use std::path::PathBuf;

fn corpus(dir: &str) -> Vec<(String, String)> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!("tests/corpus/{dir}"));
    let mut files: Vec<_> = fs::read_dir(root)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "mmd"))
        .collect();
    files.sort();
    files
        .into_iter()
        .map(|p| {
            (
                p.file_stem().unwrap().to_string_lossy().into_owned(),
                fs::read_to_string(p).unwrap(),
            )
        })
        .collect()
}

#[test]
fn 公式例文コーパスを全件受理して_well_formed_な_svg_を出す() {
    for (name, source) in corpus("sequence") {
        let svg = tankan::render_svg(&source, &tankan::Options::default())
            .unwrap_or_else(|e| panic!("{name}: 受理できるはずの例文でエラー: {e}"));
        // well-formed 検証（XML エスケープ漏れ・属性引用ミスを機械検出）
        let doc = roxmltree::Document::parse(&svg)
            .unwrap_or_else(|e| panic!("{name}: SVG が well-formed でない: {e}\n{svg}"));
        let root = doc.root_element();
        assert_eq!(root.tag_name().name(), "svg", "{name}");
        assert!(root.attribute("viewBox").is_some(), "{name}");
        // mermaid.run() の再処理対象にならないこと
        assert!(
            !root
                .attribute("class")
                .unwrap_or("")
                .split(' ')
                .any(|c| c == "mermaid"),
            "{name}: mermaid クラスを付けてはいけない"
        );
    }
}

#[test]
fn 代表例文の_svg_スナップショット() {
    for name in [
        "01-basic",
        "05-arrows",
        "06-activations",
        "07-notes",
        "09-alt-opt",
        "13-autonumber",
        "14-title-frontmatter",
        "15-japanese",
    ] {
        let source = fs::read_to_string(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join(format!("tests/corpus/sequence/{name}.mmd")),
        )
        .unwrap();
        let svg = tankan::render_svg(&source, &tankan::Options::default()).unwrap();
        insta::assert_snapshot!(format!("svg_{name}"), svg);
    }
}

#[test]
fn フォールバックコーパスは_unsupported_判定になる() {
    for (name, source) in corpus("fallback") {
        let err = tankan::render_svg(&source, &tankan::Options::default())
            .expect_err(&format!("{name}: フォールバックすべき入力が受理された"));
        assert!(err.is_unsupported(), "{name}: {err}");
    }
}

#[test]
fn 構文エラーは_parse_エラーでフォールバック区別できる() {
    let err = tankan::render_svg(
        "sequenceDiagram\n    これは矢印のない行\n",
        &tankan::Options::default(),
    )
    .unwrap_err();
    assert!(!err.is_unsupported(), "Parse エラーは要注意側: {err}");
    assert!(err.to_string().contains("2 行目"), "{err}");
}

#[test]
fn テーマの_css_変数が_style_に埋め込まれる() {
    let options = tankan::Options {
        theme: tankan::Theme {
            foreground: "var(--fg, #111)".to_string(),
            ..tankan::Theme::default()
        },
        id_prefix: "tk7".to_string(),
        ..tankan::Options::default()
    };
    let svg = tankan::render_svg("sequenceDiagram\n    A->>B: x\n", &options).unwrap();
    assert!(svg.contains("var(--fg, #111)"));
    // marker id は接頭辞で一意化される
    assert!(svg.contains(r##"marker-end="url(#tk7-head)""##));
    assert!(svg.contains(r#"id="tk7-head""#));
}
