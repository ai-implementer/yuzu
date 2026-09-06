//! 正規化出力の insta スナップショット。
//! 「同じ値から常に同じバイト列」の正規形そのものを目視レビュー対象として固定する。

use std::collections::BTreeMap;

use kabosu::{Date, Datetime, Encode, EncodeError, Encoder, Offset, TableEncoder, Time};

/// yuzu の設定を模した見本（スカラー・Option・配列・ネスト・自由キー）
struct Sample {
    title: String,
    description: Option<String>,
    port: u16,
    dark: bool,
    tags: Vec<String>,
    synonyms: Vec<Vec<String>>,
    css_vars: BTreeMap<String, String>,
    dev: Dev,
}

struct Dev {
    host: String,
    live_reload: bool,
}

impl Encode for Dev {
    fn encode(&self, encoder: &mut Encoder<'_>) -> Result<(), EncodeError> {
        let mut t: TableEncoder<'_> = encoder.table();
        t.field("host", &self.host)?;
        t.field("live_reload", &self.live_reload)?;
        Ok(())
    }
}

impl Encode for Sample {
    fn encode(&self, encoder: &mut Encoder<'_>) -> Result<(), EncodeError> {
        let mut t = encoder.table();
        t.field("title", &self.title)?;
        t.optional_field("description", &self.description)?;
        t.field("port", &self.port)?;
        t.field("dark", &self.dark)?;
        t.field("tags", &self.tags)?;
        t.field("synonyms", &self.synonyms)?;
        t.field("css_vars", &self.css_vars)?;
        t.field("dev", &self.dev)?;
        Ok(())
    }
}

fn sample(description: Option<&str>) -> Sample {
    Sample {
        title: String::from("柚子 \"yuzu\" と\\改行\n"),
        description: description.map(String::from),
        port: 5173,
        dark: true,
        tags: vec![String::from("docs"), String::from("静的サイト")],
        synonyms: vec![
            vec![String::from("ログイン"), String::from("サインイン")],
            vec![String::from("検索")],
        ],
        css_vars: {
            let mut m = BTreeMap::new();
            m.insert(String::from("--accent"), String::from("#0a6cff"));
            m.insert(String::from("フォント"), String::from("sans-serif"));
            m
        },
        dev: Dev {
            host: String::from("127.0.0.1"),
            live_reload: false,
        },
    }
}

#[test]
fn 正規化出力のスナップショット() {
    let text = kabosu::to_string(&sample(Some("説明つき"))).unwrap();
    insta::assert_snapshot!("normalize_full", text);
}

#[test]
fn option_の_none_はキーごと省略される() {
    let text = kabosu::to_string(&sample(None)).unwrap();
    assert!(!text.contains("description"));
    insta::assert_snapshot!("normalize_without_option", text);
}

#[test]
fn 空テーブルと空配列() {
    struct Empty;
    impl Encode for Empty {
        fn encode(&self, encoder: &mut Encoder<'_>) -> Result<(), EncodeError> {
            let mut t = encoder.table();
            t.field("empty_array", &Vec::<i64>::new())?;
            t.field("empty_table", &BTreeMap::<String, i64>::new())?;
            Ok(())
        }
    }
    insta::assert_snapshot!("normalize_empty", kabosu::to_string(&Empty).unwrap());
}

#[test]
fn float_の正規形() {
    struct Floats;
    impl Encode for Floats {
        fn encode(&self, encoder: &mut Encoder<'_>) -> Result<(), EncodeError> {
            let mut t = encoder.table();
            t.field("one", &1.0_f64)?;
            t.field("pi", &core::f64::consts::PI)?;
            t.field("neg_zero", &-0.0_f64)?;
            t.field("big", &1e21_f64)?;
            t.field("small", &1e-7_f64)?;
            t.field("sum", &(0.1_f64 + 0.2))?;
            t.field("max", &f64::MAX)?;
            t.field("denormal", &5e-324_f64)?;
            t.field("inf", &f64::INFINITY)?;
            t.field("neg_inf", &f64::NEG_INFINITY)?;
            t.field("nan", &-f64::NAN)?;
            t.field("list", &vec![1.5_f64, 2.0, -0.5])?;
            Ok(())
        }
    }
    insta::assert_snapshot!("normalize_floats", kabosu::to_string(&Floats).unwrap());
}

#[test]
fn date_time_の正規形() {
    struct Datetimes;
    impl Encode for Datetimes {
        fn encode(&self, encoder: &mut Encoder<'_>) -> Result<(), EncodeError> {
            let date = Date::new(1979, 5, 27).unwrap();
            let noon = Time::new(7, 32, 0, 0).unwrap();
            let mut t = encoder.table();
            t.field(
                "odt_utc",
                &Datetime::offset_datetime(date, noon, Offset::UTC),
            )?;
            t.field(
                "odt_west",
                &Datetime::offset_datetime(
                    date,
                    Time::new(0, 32, 0, 999_999_000).unwrap(),
                    Offset::from_minutes(-7 * 60).unwrap(),
                ),
            )?;
            t.field(
                "odt_half_hour",
                &Datetime::offset_datetime(date, noon, Offset::from_minutes(9 * 60 + 30).unwrap()),
            )?;
            t.field(
                "ldt",
                &Datetime::local_datetime(date, Time::new(7, 32, 0, 500_000_000).unwrap()),
            )?;
            t.field("ld", &Datetime::local_date(date))?;
            t.field(
                "lt_nanos",
                &Datetime::local_time(Time::new(7, 32, 0, 123_456_789).unwrap()),
            )?;
            t.field(
                "leap_second",
                &Datetime::local_time(Time::new(23, 59, 60, 0).unwrap()),
            )?;
            t.field(
                "leap_day",
                &Datetime::local_date(Date::new(2024, 2, 29).unwrap()),
            )?;
            t.field(
                "list",
                &vec![
                    Datetime::local_date(date),
                    Datetime::local_time(Time::new(0, 0, 0, 0).unwrap()),
                ],
            )?;
            Ok(())
        }
    }
    insta::assert_snapshot!(
        "normalize_datetimes",
        kabosu::to_string(&Datetimes).unwrap()
    );
}

#[test]
fn テーブルの配列とインラインテーブルの正規形() {
    struct Product(&'static str, i64);
    impl Encode for Product {
        fn encode(&self, encoder: &mut Encoder<'_>) -> Result<(), EncodeError> {
            let mut t = encoder.table();
            t.field("name", self.0)?;
            t.field("sku", &self.1)?;
            Ok(())
        }
    }
    /// スカラとテーブルが混在した配列（ヘッダ形式では書けない）
    struct Mixed;
    impl Encode for Mixed {
        fn encode(&self, encoder: &mut Encoder<'_>) -> Result<(), EncodeError> {
            let mut a = encoder.array();
            a.element(&1_i64)?;
            a.element(&Product("Nail", 2))?;
            a.element(&BTreeMap::<String, i64>::new())?;
            Ok(())
        }
    }
    struct Doc;
    impl Encode for Doc {
        fn encode(&self, encoder: &mut Encoder<'_>) -> Result<(), EncodeError> {
            let mut t = encoder.table();
            t.field("title", "在庫")?;
            t.field("mixed", &Mixed)?;
            t.field("empty", &Vec::<i64>::new())?;
            t.field(
                "products",
                &vec![Product("Hammer", 738594937), Product("Nail", 284758393)],
            )?;
            t.field("owner", &{
                let mut m = BTreeMap::new();
                m.insert(String::from("name"), String::from("柚子"));
                m
            })?;
            Ok(())
        }
    }
    insta::assert_snapshot!("normalize_tables", kabosu::to_string(&Doc).unwrap());
}
