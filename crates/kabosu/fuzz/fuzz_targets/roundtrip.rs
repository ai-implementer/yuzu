//! 受理された入力の正規形が「再パース可能」かつ「恒等」であること
//! （normalize(parse(normalize(x))) == normalize(x)）

#![no_main]

use kabosu::{Document, Node, Table, Value};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Ok(src) = core::str::from_utf8(data) else {
        return;
    };
    let Ok(doc) = Document::parse(src) else {
        return;
    };
    let text1 = reencode(&doc);
    let doc2 = Document::parse(&text1).expect("正規形は再パースできるはず");
    let text2 = reencode(&doc2);
    assert_eq!(text1, text2, "正規形が恒等でない");
});

/// Document の値木をそのまま Encode して正規形を得る
fn reencode(doc: &Document) -> String {
    struct Bridge<'a>(&'a Table);
    impl kabosu::Encode for Bridge<'_> {
        fn encode(&self, encoder: &mut kabosu::Encoder<'_>) -> Result<(), kabosu::EncodeError> {
            encode_table(self.0, &mut encoder.table())
        }
    }
    struct NodeBridge<'a>(&'a Node);
    impl kabosu::Encode for NodeBridge<'_> {
        fn encode(&self, encoder: &mut kabosu::Encoder<'_>) -> Result<(), kabosu::EncodeError> {
            match self.0.value() {
                Value::String(s) => encoder.string(s),
                Value::Integer(n) => encoder.integer(*n),
                Value::Boolean(b) => encoder.boolean(*b),
                Value::Array(items) => {
                    let mut array = encoder.array();
                    for item in items {
                        array.element(&NodeBridge(item))?;
                    }
                }
                Value::Table(t) => return encode_table(t, &mut encoder.table()),
                other => unreachable!("v0.1 に無い値種別: {other:?}"),
            }
            Ok(())
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
    kabosu::to_string(&Bridge(doc.root())).expect("パース済みの木は必ずエンコードできる")
}
