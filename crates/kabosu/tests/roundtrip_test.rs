//! round-trip テスト: value → to_string → from_str → value の同値と、
//! 正規化出力の恒等（normalize(parse(normalize(x))) == normalize(x)）。
//! 乱数は依存を増やさないため自作 LCG（シード固定 = 決定的）を使う。

use std::collections::BTreeMap;

use kabosu::{
    ArrayEncoder, Date, Datetime, Decode, DecodeContext, Encode, EncodeError, Encoder, Node,
    Offset, Time, Value,
};

/// 動的な値（round-trip 専用）
#[derive(Debug, Clone, PartialEq)]
enum Rand {
    Str(String),
    Int(i64),
    /// nan は生成しない（`nan != nan` で同値比較できない。別テストで往復を見る）
    Float(f64),
    Bool(bool),
    Dt(Datetime),
    List(Vec<Rand>),
}

impl Encode for Rand {
    fn encode(&self, encoder: &mut Encoder<'_>) -> Result<(), EncodeError> {
        match self {
            Rand::Str(s) => encoder.string(s),
            Rand::Int(n) => encoder.integer(*n),
            Rand::Float(f) => encoder.float(*f),
            Rand::Bool(b) => encoder.boolean(*b),
            Rand::Dt(dt) => encoder.datetime(*dt),
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
            Value::Float(f) => Some(Rand::Float(*f)),
            Value::Boolean(b) => Some(Rand::Bool(*b)),
            Value::Datetime(dt) => Some(Rand::Dt(*dt)),
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

/// 特殊値と乱数ビット列から float を生成する（nan は除く）
fn rand_float(rng: &mut Lcg) -> f64 {
    const SPECIAL: &[f64] = &[
        0.0,
        -0.0,
        1.0,
        -1.5,
        core::f64::consts::PI,
        1e21,
        1e-7,
        6.02e23,
        0.1 + 0.2,
        f64::MAX,
        f64::MIN_POSITIVE,
        5e-324,
        f64::INFINITY,
        f64::NEG_INFINITY,
    ];
    if rng.below(2) == 0 {
        return SPECIAL[rng.below(SPECIAL.len() as u64) as usize];
    }
    loop {
        let f = f64::from_bits(rng.next() << 16 | rng.below(1 << 16));
        if !f.is_nan() {
            return f;
        }
    }
}

/// 暦として存在する日付（生成した日をその月の末日まで戻す）
fn rand_date(rng: &mut Lcg) -> Date {
    let year = rng.below(10_000) as u16;
    let month = rng.below(12) as u8 + 1;
    let mut day = rng.below(31) as u8 + 1;
    while Date::new(year, month, day).is_none() {
        day -= 1;
    }
    Date::new(year, month, day).expect("存在する日まで戻した")
}

/// 範囲内の時刻（秒 60 = うるう秒と小数秒の桁数違いを混ぜる）
fn rand_time(rng: &mut Lcg) -> Time {
    let nanosecond = match rng.below(4) {
        0 => 0,
        1 => 500_000_000,
        2 => rng.below(1_000) as u32 * 1_000_000,
        _ => rng.below(1_000_000_000) as u32,
    };
    Time::new(
        rng.below(24) as u8,
        rng.below(60) as u8,
        rng.below(61) as u8,
        nanosecond,
    )
    .expect("範囲内で生成した")
}

/// 4 種の日付・時刻
fn rand_datetime(rng: &mut Lcg) -> Datetime {
    match rng.below(4) {
        0 => {
            let minutes = rng.below(2 * (23 * 60 + 59) + 1) as i16 - (23 * 60 + 59);
            let offset = Offset::from_minutes(minutes).expect("範囲内で生成した");
            Datetime::offset_datetime(rand_date(rng), rand_time(rng), offset)
        }
        1 => Datetime::local_datetime(rand_date(rng), rand_time(rng)),
        2 => Datetime::local_date(rand_date(rng)),
        _ => Datetime::local_time(rand_time(rng)),
    }
}

fn rand_value(rng: &mut Lcg, depth: usize) -> Rand {
    match rng.below(if depth >= 3 { 5 } else { 6 }) {
        0 => Rand::Str(rand_string(rng)),
        1 => Rand::Int(rng.next() as i64),
        2 => Rand::Bool(rng.below(2) == 0),
        3 => Rand::Float(rand_float(rng)),
        4 => Rand::Dt(rand_datetime(rng)),
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
    map.insert("ratio".into(), Rand::Float(0.1 + 0.2));
    map.insert("neg_zero".into(), Rand::Float(-0.0));
    map.insert("huge".into(), Rand::Float(f64::MAX));
    map.insert("tiny".into(), Rand::Float(5e-324));
    map.insert("inf".into(), Rand::Float(f64::NEG_INFINITY));
    map.insert(
        "odt".into(),
        Rand::Dt(Datetime::offset_datetime(
            Date::new(1979, 5, 27).unwrap(),
            Time::new(0, 32, 0, 999_999_000).unwrap(),
            Offset::from_minutes(-7 * 60).unwrap(),
        )),
    );
    // オフセット 0 は正規形で `Z` になり、往復しても同じ値のまま
    map.insert(
        "utc".into(),
        Rand::Dt(Datetime::offset_datetime(
            Date::new(2026, 9, 6).unwrap(),
            Time::new(23, 59, 60, 0).unwrap(),
            Offset::UTC,
        )),
    );
    map.insert(
        "leap_day".into(),
        Rand::Dt(Datetime::local_date(Date::new(2024, 2, 29).unwrap())),
    );
    map.insert(
        "lt".into(),
        Rand::Dt(Datetime::local_time(
            Time::new(7, 32, 0, 123_456_789).unwrap(),
        )),
    );
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

#[test]
fn nan_と負のゼロの往復() {
    // nan は同値比較できないので個別に見る。符号は落ちて `nan` になる
    let mut map: BTreeMap<String, Rand> = BTreeMap::new();
    map.insert("n".into(), Rand::Float(-f64::NAN));
    map.insert("z".into(), Rand::Float(-0.0));
    let text = kabosu::to_string(&map).unwrap();
    assert_eq!(text, "n = nan\nz = -0.0\n");
    let report = kabosu::from_str::<BTreeMap<String, Rand>>(&text).unwrap();
    let value = report.value().unwrap();
    match value.get("n") {
        Some(Rand::Float(f)) => assert!(f.is_nan() && !f.is_sign_negative()),
        other => panic!("{other:?}"),
    }
    // -0.0 == 0.0 なので符号ビットで見る
    match value.get("z") {
        Some(Rand::Float(f)) => assert!(f.is_sign_negative()),
        other => panic!("{other:?}"),
    }
}
