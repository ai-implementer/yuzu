//! 公式 toml-test（vendor。tests/toml-test/README.md 参照）のハーネス。
//!
//! - valid/: 全件受理し、値が期待値（tagged JSON）と一致する
//! - invalid/: 全件エラーになる
//!
//! 取り込んでいるのは `files-toml-1.0.0` に載っているケースだけなので、
//! TOML 1.1 専用の構文は出てこない。
//!
//! 期待値との比較は**文字列ではなく値**で行う。float は表記が揺れ
//! （`5e+22` と `5e22`）、date-time も区切りやオフセットの書き方が揺れるため、
//! どちらも同じ経路で値に戻してから比べる。

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use kabosu::{Document, Node, Table, Value};

fn suite_dir(kind: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!("tests/toml-test/{kind}"))
}

/// `.toml` を再帰的に集める（安定した順序で返す）
fn cases(kind: &str) -> Vec<PathBuf> {
    let root = suite_dir(kind);
    let mut out = Vec::new();
    collect(&root, &mut out);
    out.sort();
    assert!(
        !out.is_empty(),
        "toml-test/{kind} が空（scripts/vendor-toml-test.sh を実行する）"
    );
    out
}

fn collect(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display())) {
        let path = entry.unwrap().path();
        if path.is_dir() {
            collect(&path, out);
        } else if path.extension().is_some_and(|e| e == "toml") {
            out.push(path);
        }
    }
}

fn case_name(path: &Path) -> String {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/toml-test");
    path.strip_prefix(&root)
        .unwrap_or(path)
        .display()
        .to_string()
}

/// 比較用の値（kabosu の値木と期待値 JSON の共通形）
#[derive(Debug, PartialEq)]
enum Expected {
    Str(String),
    Int(i64),
    /// nan 同士は等しいとみなす。それ以外はビット一致（`-0.0` と `0.0` を区別する）
    Float(f64),
    Bool(bool),
    /// 正規形の文字列（種別の違いも表記に出る）
    Datetime(String),
    Array(Vec<Expected>),
    Table(BTreeMap<String, Expected>),
}

impl Eq for Expected {}

fn float_eq(a: f64, b: f64) -> bool {
    (a.is_nan() && b.is_nan()) || a.to_bits() == b.to_bits()
}

/// kabosu の値木から
fn from_node(node: &Node) -> Expected {
    match node.value() {
        Value::String(s) => Expected::Str(s.clone()),
        Value::Integer(n) => Expected::Int(*n),
        Value::Float(f) => Expected::Float(*f),
        Value::Boolean(b) => Expected::Bool(*b),
        Value::Datetime(dt) => Expected::Datetime(dt.to_string()),
        Value::Array(items) => Expected::Array(items.iter().map(from_node).collect()),
        Value::Table(t) => from_table(t),
        other => unreachable!("未対応の値種別: {other:?}"),
    }
}

fn from_table(table: &Table) -> Expected {
    Expected::Table(
        table
            .entries()
            .map(|e| (e.key().to_string(), from_node(e.node())))
            .collect(),
    )
}

/// 期待値の tagged JSON から。
/// `{"type": ..., "value": ...}`（どちらも文字列）だけをスカラとみなし、
/// それ以外のオブジェクトはテーブルとして読む（`type` / `value` という名前の
/// キーを持つテーブルと取り違えないため）
fn from_json(v: &serde_json::Value, name: &str) -> Expected {
    match v {
        serde_json::Value::Array(items) => {
            Expected::Array(items.iter().map(|i| from_json(i, name)).collect())
        }
        serde_json::Value::Object(map) => {
            if let Some(scalar) = scalar_from_json(map, name) {
                return scalar;
            }
            Expected::Table(
                map.iter()
                    .map(|(k, v)| (k.clone(), from_json(v, name)))
                    .collect(),
            )
        }
        other => panic!("{name}: 期待値の形が想定外: {other}"),
    }
}

fn scalar_from_json(
    map: &serde_json::Map<String, serde_json::Value>,
    name: &str,
) -> Option<Expected> {
    if map.len() != 2 {
        return None;
    }
    let ty = map.get("type")?.as_str()?;
    let value = map.get("value")?.as_str()?;
    let parsed = match ty {
        "string" => Expected::Str(String::from(value)),
        "integer" => Expected::Int(
            value
                .parse()
                .unwrap_or_else(|e| panic!("{name}: 期待値の整数が読めない {value:?}: {e}")),
        ),
        "float" => Expected::Float(parse_float(value, name)),
        "bool" => Expected::Bool(
            value
                .parse()
                .unwrap_or_else(|e| panic!("{name}: 期待値の真偽値が読めない {value:?}: {e}")),
        ),
        // 期待値の日付・時刻も kabosu に読ませて正規形へ揃える
        // （`1987-07-05t17:45:00z` と `1987-07-05T17:45:00Z` を同じ値として扱う）
        "datetime" | "datetime-local" | "date-local" | "time-local" => {
            Expected::Datetime(normalize_datetime(value, name))
        }
        _ => return None,
    };
    Some(parsed)
}

fn parse_float(value: &str, name: &str) -> f64 {
    match value {
        "inf" | "+inf" => f64::INFINITY,
        "-inf" => f64::NEG_INFINITY,
        "nan" | "+nan" | "-nan" => f64::NAN,
        _ => value
            .parse()
            .unwrap_or_else(|e| panic!("{name}: 期待値の float が読めない {value:?}: {e}")),
    }
}

/// 期待値の日付・時刻を kabosu の正規形へ揃える
fn normalize_datetime(value: &str, name: &str) -> String {
    let doc = Document::parse(&format!("x = {value}\n"))
        .unwrap_or_else(|e| panic!("{name}: 期待値の日付が読めない {value:?}: {e}"));
    doc.root()
        .get("x")
        .and_then(|e| e.node().as_datetime())
        .unwrap_or_else(|| panic!("{name}: 期待値 {value:?} が日付・時刻にならない"))
        .to_string()
}

/// float を含む比較（`PartialEq` の導出はビット一致なので nan で落ちる）
fn same(a: &Expected, b: &Expected) -> bool {
    match (a, b) {
        (Expected::Float(x), Expected::Float(y)) => float_eq(*x, *y),
        (Expected::Array(x), Expected::Array(y)) => {
            x.len() == y.len() && x.iter().zip(y).all(|(x, y)| same(x, y))
        }
        (Expected::Table(x), Expected::Table(y)) => {
            x.len() == y.len()
                && x.iter()
                    .zip(y)
                    .all(|((xk, xv), (yk, yv))| xk == yk && same(xv, yv))
        }
        _ => a == b,
    }
}

#[test]
fn toml_test_の_valid_を全件通す() {
    let mut failures: Vec<String> = Vec::new();
    for path in cases("valid") {
        let name = case_name(&path);
        let src = fs::read_to_string(&path).unwrap_or_else(|e| panic!("{name}: {e}"));
        let doc = match Document::parse(&src) {
            Ok(doc) => doc,
            Err(e) => {
                let lc = kabosu::line_col_of(&src, e.span().start);
                failures.push(format!("{name}:{}:{}: 受理できない: {e}", lc.line, lc.col));
                continue;
            }
        };
        let expected_json = fs::read_to_string(path.with_extension("json"))
            .unwrap_or_else(|e| panic!("{name}: 期待値 JSON が読めない: {e}"));
        let expected: serde_json::Value = serde_json::from_str(&expected_json)
            .unwrap_or_else(|e| panic!("{name}: 期待値 JSON が壊れている: {e}"));
        let ours = from_table(doc.root());
        let theirs = from_json(&expected, &name);
        if !same(&ours, &theirs) {
            failures.push(format!(
                "{name}: 値が期待値と違う\n  ours={ours:?}\n  want={theirs:?}"
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} 件失敗:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

#[test]
fn toml_test_の_invalid_を全件拒否する() {
    let mut failures: Vec<String> = Vec::new();
    for path in cases("invalid") {
        let name = case_name(&path);
        // 不正な UTF-8 のケースがある。kabosu の入口は `&str` なので、
        // 文字列にできない時点で拒否できている
        let Ok(src) = fs::read_to_string(&path) else {
            continue;
        };
        if Document::parse(&src).is_ok() {
            failures.push(format!("{name}: 誤って受理した"));
        }
    }
    assert!(
        failures.is_empty(),
        "{} 件失敗:\n{}",
        failures.len(),
        failures.join("\n")
    );
}
