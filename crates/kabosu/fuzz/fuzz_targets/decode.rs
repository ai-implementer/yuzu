//! 全型を含む動的構造への decode が任意入力で panic しないこと

#![no_main]

use std::collections::BTreeMap;

use kabosu::{Decode, DecodeContext, Node, Value};
use libfuzzer_sys::fuzz_target;

enum Dyn {
    #[allow(dead_code)]
    Str(String),
    #[allow(dead_code)]
    Int(i64),
    #[allow(dead_code)]
    Float(f64),
    #[allow(dead_code)]
    Bool(bool),
    #[allow(dead_code)]
    List(Vec<Dyn>),
    #[allow(dead_code)]
    Table(BTreeMap<String, Dyn>),
}

impl Decode for Dyn {
    fn decode(node: &Node, cx: &mut DecodeContext<'_>) -> Option<Self> {
        match node.value() {
            Value::String(s) => Some(Dyn::Str(s.clone())),
            Value::Integer(n) => Some(Dyn::Int(*n)),
            Value::Float(f) => Some(Dyn::Float(*f)),
            Value::Boolean(b) => Some(Dyn::Bool(*b)),
            Value::Array(_) => Vec::<Dyn>::decode(node, cx).map(Dyn::List),
            Value::Table(_) => BTreeMap::<String, Dyn>::decode(node, cx).map(Dyn::Table),
            _ => None,
        }
    }
}

fuzz_target!(|data: &[u8]| {
    if let Ok(src) = core::str::from_utf8(data) {
        let _ = kabosu::from_str::<BTreeMap<String, Dyn>>(src);
    }
});
