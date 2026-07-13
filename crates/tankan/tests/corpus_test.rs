//! mermaid.js 公式ドキュメント例文コーパスの互換性テスト（全図種共通・テーブル駆動）。
//!
//! - `corpus/<図種>/*.mmd` — 全件が受理され（構文互換）、well-formed で
//!   座標が viewBox 内に収まる SVG になること。代表例は insta スナップショット
//! - `corpus/fallback/*.mmd` — 未対応構文・図種が `Err` かつ
//!   `is_unsupported()` でフォールバック判定できること

use std::fs;
use std::path::PathBuf;

/// (図種ディレクトリ, スナップショットを取る代表例)
const CORPORA: &[(&str, &[&str])] = &[
    (
        "sequence",
        &[
            "01-basic",
            "05-arrows",
            "06-activations",
            "07-notes",
            "09-alt-opt",
            "13-autonumber",
            "14-title-frontmatter",
            "15-japanese",
        ],
    ),
    (
        "flowchart",
        &[
            "01-basic",
            "02-lr",
            "03-shapes",
            "05-edge-labels",
            "07-subgraph",
            "14-japanese",
        ],
    ),
    (
        "state",
        &["01-basic", "03-composite", "08-concurrency", "10-japanese"],
    ),
    ("er", &["01-basic", "03-keys-comments", "07-japanese"]),
    ("gantt", &["01-basic", "02-excludes-tags", "05-japanese"]),
    ("class", &["01-basic", "04-relations", "08-japanese"]),
    ("pie", &["01-basic", "04-japanese"]),
];

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
fn 公式例文コーパスを全件受理して検証済み_svg_を出す() {
    for (dir, _) in CORPORA {
        for (name, source) in corpus(dir) {
            let svg = tankan::render_svg(&source, &tankan::Options::default())
                .unwrap_or_else(|e| panic!("{dir}/{name}: 受理できるはずの例文でエラー: {e}"));
            validate_svg(&format!("{dir}/{name}"), &svg);
        }
    }
}

/// well-formed・viewBox 整合・座標が範囲内・有限値、の共通検証
/// （NaN/inf は座標パース時の is_finite で検出。文字列一致だと本文の
/// "information" 等に誤マッチするため使わない）
fn validate_svg(name: &str, svg: &str) {
    let doc = roxmltree::Document::parse(svg)
        .unwrap_or_else(|e| panic!("{name}: SVG が well-formed でない: {e}\n{svg}"));
    let root = doc.root_element();
    assert_eq!(root.tag_name().name(), "svg", "{name}");
    let viewbox = root
        .attribute("viewBox")
        .unwrap_or_else(|| panic!("{name}: viewBox なし"));
    let dims: Vec<f32> = viewbox.split(' ').map(|v| v.parse().unwrap()).collect();
    let (vw, vh) = (dims[2], dims[3]);
    assert!(
        !root
            .attribute("class")
            .unwrap_or("")
            .split(' ')
            .any(|c| c == "mermaid"),
        "{name}: mermaid クラスを付けてはいけない"
    );

    // 単純座標属性が viewBox 内（marker の viewBox 内座標は除外）
    const TOL: f32 = 1.0;
    for node in doc.descendants().filter(|n| n.is_element()) {
        if node.ancestors().any(|a| a.has_tag_name("marker")) {
            continue;
        }
        for (attr, horizontal) in [
            ("x", true),
            ("x1", true),
            ("x2", true),
            ("cx", true),
            ("y", false),
            ("y1", false),
            ("y2", false),
            ("cy", false),
        ] {
            if let Some(v) = node.attribute(attr).and_then(|v| v.parse::<f32>().ok()) {
                let limit = if horizontal { vw } else { vh };
                assert!(v.is_finite(), "{name}: {attr} が有限値でない");
                assert!(
                    (-TOL..=limit + TOL).contains(&v),
                    "{name}: {} の {attr}={v} が viewBox（{limit}）外",
                    node.tag_name().name(),
                );
            }
        }
        if let Some(points) = node.attribute("points") {
            for (i, v) in points.split([' ', ',']).enumerate() {
                if let Ok(v) = v.parse::<f32>() {
                    let limit = if i % 2 == 0 { vw } else { vh };
                    assert!(v.is_finite(), "{name}: points 座標が有限値でない");
                    assert!(
                        (-TOL..=limit + TOL).contains(&v),
                        "{name}: points 座標 {v} が viewBox 外"
                    );
                }
            }
        }
    }
}

#[test]
fn 代表例文の_svg_スナップショット() {
    for (dir, representatives) in CORPORA {
        for name in *representatives {
            let source = fs::read_to_string(
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join(format!("tests/corpus/{dir}/{name}.mmd")),
            )
            .unwrap();
            let svg = tankan::render_svg(&source, &tankan::Options::default()).unwrap();
            insta::assert_snapshot!(format!("svg_{dir}_{name}"), svg);
        }
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
    for (source, expect_line) in [
        ("sequenceDiagram\n    これは矢印のない行\n", "2 行目"),
        ("flowchart TD\n    A[未閉鎖 --> B\n", "2 行目"),
    ] {
        let err = tankan::render_svg(source, &tankan::Options::default()).unwrap_err();
        assert!(!err.is_unsupported(), "Parse エラーは要注意側: {err}");
        assert!(err.to_string().contains(expect_line), "{err}");
    }
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
    for source in [
        "sequenceDiagram\n    A->>B: x\n",
        "flowchart TD\n    A --> B\n",
    ] {
        let svg = tankan::render_svg(source, &options).unwrap();
        assert!(svg.contains("var(--fg, #111)"), "{source}");
        assert!(svg.contains(r#"id="tk7-"#), "marker id の接頭辞: {source}");
    }
}
