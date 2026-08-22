//! round-trip テスト: value → to_string → from_str → value の同値と、
//! 正規化出力の恒等（normalize(parse(normalize(x))) == normalize(x)）。
//! 乱数は依存を増やさないため自作 LCG（シード固定 = 決定的）を使う。

use std::collections::BTreeMap;

use kabosu::{ArrayEncoder, Decode, DecodeContext, Encode, EncodeError, Encoder, Node, Value};

/// 動的な値（round-trip 専用）
#[derive(Debug, Clone, PartialEq)]
enum Rand {
    Str(String),
    Int(i64),
    Bool(bool),
    List(Vec<Rand>),
}

impl Encode for Rand {
    fn encode(&self, encoder: &mut Encoder<'_>) -> Result<(), EncodeError> {
        match self {
            Rand::Str(s) => encoder.string(s),
            Rand::Int(n) => encoder.integer(*n),
            Rand::Bool(b) => encoder.boolean(*b),
            Rand::List(items) => {
                let mut array: ArrayEncoder<'_> = encoder.array();
                for item in items {
                    array.element(item)?;
                }
            }
        }
        Ok(())
    }
}

impl Decode for Rand {
    // cx は再帰呼び出しにしか使わない（trait のシグネチャなので外せない）
    #[allow(clippy::only_used_in_recursion)]
    fn decode(node: &Node, cx: &mut DecodeContext<'_>) -> Option<Self> {
        match node.value() {
            Value::String(s) => Some(Rand::Str(s.clone())),
            Value::Integer(n) => Some(Rand::Int(*n)),
            Value::Boolean(b) => Some(Rand::Bool(*b)),
            Value::Array(items) => {
                let mut out = Vec::new();
                for item in items {
                    out.push(Rand::decode(item, cx)?);
                }
                Some(Rand::List(out))
            }
            // このテストの値位置にテーブルは生成しない（non_exhaustive のため包括腕）
            _ => None,
        }
    }
}

/// 依存なしの簡易乱数（LCG。シード固定なので決定的）
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0 >> 16
    }

    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

/// 引用符・バックスラッシュ・制御文字・日本語を含む文字列を生成する
fn rand_string(rng: &mut Lcg) -> String {
    const CHARS: &[&str] = &[
        "a", "Z", "0", "_", "-", " ", "\"", "\\", "\t", "\n", "あ", "柚", "🍊", "#", "'", "=", "[",
        "]",
    ];
    let len = rng.below(8) as usize;
    (0..len)
        .map(|_| CHARS[rng.below(CHARS.len() as u64) as usize])
        .collect()
}

fn rand_value(rng: &mut Lcg, depth: usize) -> Rand {
    match rng.below(if depth >= 3 { 3 } else { 4 }) {
        0 => Rand::Str(rand_string(rng)),
        1 => Rand::Int(rng.next() as i64),
        2 => Rand::Bool(rng.below(2) == 0),
        _ => {
            let len = rng.below(4) as usize;
            Rand::List((0..len).map(|_| rand_value(rng, depth + 1)).collect())
        }
    }
}

/// テーブル（キーは重複しないよう添字で生成。引用が要るキーも混ぜる）
fn rand_table(rng: &mut Lcg) -> BTreeMap<String, Rand> {
    let len = rng.below(8) as usize + 1;
    (0..len)
        .map(|i| {
            let key = match rng.below(3) {
                0 => format!("key-{i}"),
                1 => format!("キー{i}"),
                _ => format!("k {i}\""),
            };
            (key, rand_value(rng, 0))
        })
        .collect()
}

#[test]
fn 手書きケースの_round_trip() {
    let mut map: BTreeMap<String, Rand> = BTreeMap::new();
    map.insert(
        "title".into(),
        Rand::Str("柚子 \"yuzu\" \\ 改行\nタブ\t".into()),
    );
    map.insert("count".into(), Rand::Int(i64::MIN));
    map.insert("max".into(), Rand::Int(i64::MAX));
    map.insert("flag".into(), Rand::Bool(false));
    map.insert("empty".into(), Rand::List(vec![]));
    map.insert(
        "nested".into(),
        Rand::List(vec![
            Rand::List(vec![Rand::Int(1), Rand::Int(2)]),
            Rand::Str("x".into()),
        ]),
    );
    map.insert("--css-var".into(), Rand::Str("#333".into()));
    map.insert(
        "サーバー".into(),
        Rand::List(vec![Rand::Str("サーバ".into())]),
    );

    let text = kabosu::to_string(&map).unwrap();
    let report = kabosu::from_str::<BTreeMap<String, Rand>>(&text).unwrap();
    assert!(!report.has_errors(), "{text}\n{:?}", report.diagnostics());
    assert_eq!(report.value().unwrap(), &map, "\n--- 出力 ---\n{text}");
}

#[test]
fn 乱数生成の_round_trip_と正規化の恒等() {
    let mut rng = Lcg(20260820);
    for case in 0..200 {
        let map = rand_table(&mut rng);
        let text1 = kabosu::to_string(&map).unwrap();
        let report = kabosu::from_str::<BTreeMap<String, Rand>>(&text1)
            .unwrap_or_else(|e| panic!("case {case}: 正規化出力がパースできない: {e}\n{text1}"));
        assert!(!report.has_errors(), "case {case}:\n{text1}");
        assert_eq!(report.value().unwrap(), &map, "case {case}:\n{text1}");
        // 恒等: 再エンコードでバイト同一
        let text2 = kabosu::to_string(report.value().unwrap()).unwrap();
        assert_eq!(text1, text2, "case {case}: 正規化出力が安定でない");
    }
}

#[test]
fn ルートがテーブルでない値は_root_not_table() {
    let e = kabosu::to_string(&Rand::Int(1)).unwrap_err();
    assert_eq!(*e.kind(), kabosu::EncodeErrorKind::RootNotTable);
}
