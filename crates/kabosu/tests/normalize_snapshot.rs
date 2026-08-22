//! 正規化出力の insta スナップショット。
//! 「同じ値から常に同じバイト列」の正規形そのものを目視レビュー対象として固定する。

use std::collections::BTreeMap;

use kabosu::{Encode, EncodeError, Encoder, TableEncoder};

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
