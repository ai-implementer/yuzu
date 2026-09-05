//! 参照実装（`toml` crate。dev 依存）との差分テスト。
//! v0.1 の対応範囲内の入力について「kabosu と toml の値解釈が一致」
//! 「kabosu がエラーにする入力は toml もエラー」を照合する。

use std::fs;
use std::path::PathBuf;

use kabosu::{Document, Node, Table, Value};

/// kabosu の値木を toml::Value へ写す（比較用）
fn to_toml_value(node: &Node) -> toml::Value {
    match node.value() {
        Value::String(s) => toml::Value::String(s.clone()),
        Value::Integer(n) => toml::Value::Integer(*n),
        Value::Float(f) => toml::Value::Float(*f),
        Value::Boolean(b) => toml::Value::Boolean(*b),
        Value::Array(items) => toml::Value::Array(items.iter().map(to_toml_value).collect()),
        Value::Table(t) => table_to_toml(t),
        other => unreachable!("未対応の値種別: {other:?}"),
    }
}

fn table_to_toml(table: &Table) -> toml::Value {
    let mut map = toml::map::Map::new();
    for entry in table.entries() {
        map.insert(entry.key().to_string(), to_toml_value(entry.node()));
    }
    toml::Value::Table(map)
}

/// `nan` は `nan != nan` なので等値比較できない。比較用に文字列へ置き換える
/// （kabosu 側も参照実装側も同じ変換を通す）
fn canon(v: toml::Value) -> toml::Value {
    match v {
        toml::Value::Float(f) if f.is_nan() => toml::Value::String(String::from("<nan>")),
        toml::Value::Array(items) => toml::Value::Array(items.into_iter().map(canon).collect()),
        toml::Value::Table(t) => {
            toml::Value::Table(t.into_iter().map(|(k, v)| (k, canon(v))).collect())
        }
        other => other,
    }
}

fn corpus(dir: &str) -> Vec<(String, String)> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!("tests/corpus/{dir}"));
    let mut files: Vec<_> = fs::read_dir(root)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "toml"))
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
fn valid_corpus_の値解釈が参照実装と一致する() {
    for (name, src) in corpus("valid") {
        let doc =
            Document::parse(&src).unwrap_or_else(|e| panic!("{name}: kabosu が受理しない: {e}"));
        let ours = canon(table_to_toml(doc.root()));
        let theirs: toml::Table = src
            .parse()
            .unwrap_or_else(|e| panic!("{name}: 参照実装が受理しない: {e}"));
        assert_eq!(
            ours,
            canon(toml::Value::Table(theirs)),
            "{name}: 値解釈が参照実装と一致しない"
        );
    }
}

#[test]
fn invalid_corpus_は参照実装でもエラーになる() {
    for (name, src) in corpus("invalid") {
        assert!(
            Document::parse(&src).is_err(),
            "{name}: kabosu が誤って受理した"
        );
        assert!(
            src.parse::<toml::Table>().is_err(),
            "{name}: 参照実装は受理する = kabosu の誤検出の疑い"
        );
    }
}

#[test]
fn unsupported_corpus_は参照実装では受理される() {
    // 「TOML として正しいが v0.1 では未対応」の裏取り:
    // 参照実装が受理することで、invalid（書き間違い）との区別が正しいことを縛る
    for (name, src) in corpus("unsupported") {
        assert!(
            src.parse::<toml::Table>().is_ok(),
            "{name}: 参照実装が受理しない = これは unsupported ではなく invalid"
        );
    }
}

#[test]
fn 正規化出力は参照実装でも同じ値に読める() {
    for (name, src) in corpus("valid") {
        let doc = Document::parse(&src).unwrap();
        let ours = canon(table_to_toml(doc.root()));
        // Document → 値 → 正規形はまだ無い（decode 型が要る）ので、
        // 参照実装の値を kabosu の正規形と突き合わせる代わりに
        // 参照実装で正規形を再パースして値一致を見る
        let normalized = reencode(&doc);
        let reparsed: toml::Table = normalized
            .parse()
            .unwrap_or_else(|e| panic!("{name}: 正規形を参照実装が受理しない: {e}\n{normalized}"));
        assert_eq!(
            ours,
            canon(toml::Value::Table(reparsed)),
            "{name}: 正規形の値が変わった\n{normalized}"
        );
    }
}

/// Document の値木をそのまま Encode して正規形を得る（テスト用ブリッジ）
fn reencode(doc: &Document) -> String {
    struct Bridge<'a>(&'a Table);
    impl kabosu::Encode for Bridge<'_> {
        fn encode(&self, encoder: &mut kabosu::Encoder<'_>) -> Result<(), kabosu::EncodeError> {
            encode_table(self.0, &mut encoder.table())
        }
    }
    fn encode_table(
        table: &Table,
        out: &mut kabosu::TableEncoder<'_>,
    ) -> Result<(), kabosu::EncodeError> {
        for entry in table.entries() {
            out.field(entry.key(), &NodeBridge(entry.node()))?;
        }
        Ok(())
    }
    struct NodeBridge<'a>(&'a Node);
    impl kabosu::Encode for NodeBridge<'_> {
        fn encode(&self, encoder: &mut kabosu::Encoder<'_>) -> Result<(), kabosu::EncodeError> {
            match self.0.value() {
                Value::String(s) => encoder.string(s),
                Value::Integer(n) => encoder.integer(*n),
                Value::Float(f) => encoder.float(*f),
                Value::Boolean(b) => encoder.boolean(*b),
                Value::Array(items) => {
                    let mut array = encoder.array();
                    for item in items {
                        array.element(&NodeBridge(item))?;
                    }
                }
                Value::Table(t) => {
                    return encode_table(t, &mut encoder.table());
                }
                other => unreachable!("未対応の値種別: {other:?}"),
            }
            Ok(())
        }
    }
    kabosu::to_string(&Bridge(doc.root())).unwrap()
}
