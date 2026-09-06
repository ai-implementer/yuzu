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
    /// テーブル。値位置ならインラインテーブル、キー直下なら `[a]`、
    /// 配列の要素が全部これなら `[[a]]` として出力される
    Map(BTreeMap<String, Rand>),
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
            Rand::Map(map) => return map.encode(encoder),
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
            Value::Table(_) => BTreeMap::<String, Rand>::decode(node, cx).map(Rand::Map),
            // Value は non_exhaustive なので包括腕が要る
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
    match rng.below(if depth >= 3 { 5 } else { 7 }) {
        0 => Rand::Str(rand_string(rng)),
        1 => Rand::Int(rng.next() as i64),
        2 => Rand::Bool(rng.below(2) == 0),
        3 => Rand::Float(rand_float(rng)),
        4 => Rand::Dt(rand_datetime(rng)),
        5 => {
            // 要素が全部テーブルの配列（`[[a]]` へ展開される）も混ぜたいので、
            // 半分はテーブルだけの配列にする
            let len = rng.below(4) as usize;
            let tables_only = rng.below(2) == 0;
            Rand::List(
                (0..len)
                    .map(|_| {
                        if tables_only {
                            Rand::Map(rand_map(rng, depth + 1))
                        } else {
                            rand_value(rng, depth + 1)
                        }
                    })
                    .collect(),
            )
        }
        _ => Rand::Map(rand_map(rng, depth + 1)),
    }
}

/// ネストしたテーブル（空も混ぜる）
fn rand_map(rng: &mut Lcg, depth: usize) -> BTreeMap<String, Rand> {
    let len = rng.below(3) as usize;
    (0..len)
        .map(|i| (format!("k{i}"), rand_value(rng, depth)))
        .collect()
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

/// `[[x]]` は「配列」と「その要素テーブル」で 2 段。経路のセグメント数で
/// 代用すると「パースできたのにエンコードできない」木が作れる（fuzz が見つけた）
#[test]
fn 配列ヘッダーの下の深いキーも読み書きの上限が一致する() {
    for header in ["[[x]]", "[x]", "[[x.y]]", "[[x]]\n[[x.y]]"] {
        for n in 118..=132 {
            let key = std::iter::repeat_n("k", n).collect::<Vec<_>>().join(".");
            let src = format!("{header}\n{key} = 1\n");
            let Ok(report) = kabosu::from_str::<BTreeMap<String, Rand>>(&src) else {
                continue; // パースが上限で断ったぶんは対象外
            };
            let Some(value) = report.value() else {
                continue;
            };
            kabosu::to_string(value).unwrap_or_else(|e| {
                panic!("{header:?} n={n}: パースできたのに再エンコードできない: {e}")
            });
        }
    }
}

#[test]
fn テーブルだけの配列はヘッダ形式_混在はインラインになる() {
    let table = |pairs: &[(&str, &str)]| {
        Rand::Map(
            pairs
                .iter()
                .map(|(k, v)| (String::from(*k), Rand::Str(String::from(*v))))
                .collect(),
        )
    };
    let mut map: BTreeMap<String, Rand> = BTreeMap::new();
    map.insert(
        "products".into(),
        Rand::List(vec![
            table(&[("name", "Hammer")]),
            table(&[("name", "Nail")]),
        ]),
    );
    map.insert(
        "mixed".into(),
        Rand::List(vec![Rand::Int(1), table(&[("b", "x")])]),
    );
    map.insert("empty_list".into(), Rand::List(vec![]));

    let text = kabosu::to_string(&map).unwrap();
    assert_eq!(
        text,
        "empty_list = []\n\
         mixed = [1, { b = \"x\" }]\n\
         \n\
         [[products]]\n\
         name = \"Hammer\"\n\
         \n\
         [[products]]\n\
         name = \"Nail\"\n"
    );
    // 往復しても同じ値・同じバイト列
    let report = kabosu::from_str::<BTreeMap<String, Rand>>(&text).unwrap();
    assert_eq!(report.value().unwrap(), &map);
    assert_eq!(kabosu::to_string(report.value().unwrap()).unwrap(), text);
}

/// `inner` を n 段の単一キーテーブルで包む
fn wrap(n: usize, inner: Rand) -> Rand {
    let mut value = inner;
    for _ in 0..n {
        value = Rand::Map(BTreeMap::from([(String::from("k"), value)]));
    }
    value
}

#[test]
fn 上限付近のネストでも正規形は再パースできる() {
    // 整数と混在させるとヘッダ形式が使えず、インラインテーブルとして出力される。
    // エンコーダが通した出力は必ず再パースできる = 読み書きの深度上限が揃っている。
    // **最深部が空テーブル・空配列のときも同じ**（空のコンテナは入れ子を
    // 1 段も増やさないので、パース側だけが 1 段数えると境界がずれる）
    for inner in [Rand::Int(1), Rand::Map(BTreeMap::new()), Rand::List(vec![])] {
        for n in 100..=130 {
            let mut map: BTreeMap<String, Rand> = BTreeMap::new();
            map.insert(
                "mixed".into(),
                Rand::List(vec![Rand::Int(1), wrap(n, inner.clone())]),
            );
            let Ok(text) = kabosu::to_string(&map) else {
                continue; // エンコード側が上限で断ったぶんは対象外
            };
            let report = kabosu::from_str::<BTreeMap<String, Rand>>(&text)
                .unwrap_or_else(|e| panic!("inner={inner:?} n={n}: 正規形が再パースできない: {e}"));
            assert!(
                !report.has_errors(),
                "inner={inner:?} n={n}: {:?}",
                report.diagnostics()
            );
            assert_eq!(report.value().unwrap(), &map, "inner={inner:?} n={n}");
        }
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
