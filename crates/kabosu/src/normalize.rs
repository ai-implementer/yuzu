//! 正規化出力（同じ値から常に同じバイト列）。
//!
//! - 改行は LF、末尾にも改行を付ける
//! - 空配列は `[]`、スカラーだけの配列は 1 行、配列を含むネスト配列は
//!   スペース 2 個のインデントと末尾カンマ（行幅による自動折り返しはしない）
//! - 文字列は常に単行の basic string。引用符・バックスラッシュ・制御文字だけを
//!   エスケープし、表示可能な Unicode はそのまま出力する（改行は `\n`。
//!   複数行文字列の形では出力しない）
//! - 整数は 10 進、float は往復可能な最短表現（`.` も `e` も無ければ `.0` を補う。
//!   `inf` / `-inf` / `nan`。`nan` の符号は落とす）
//! - 日付・時刻は RFC 3339 の正規形（区切りは大文字 `T`・小数秒の末尾ゼロは落とす・
//!   オフセット 0 は `Z`）。書式は `Datetime` の `Display` に 1 実装で持たせる
//! - `[A-Za-z0-9_-]+` に一致するキーは bare key、それ以外は basic string で引用
//! - 親テーブルの値を子テーブルより先に出力し、各グループでは追加順を維持する
//! - 要素が全部テーブルの配列は `[[a]]` へ展開する。インラインテーブルは
//!   ヘッダ形式で書けない位置（配列の中）でだけ使う

use alloc::string::{String, ToString};
use alloc::vec::Vec;

use crate::encode::{EncTable, EncValue};

/// bare key として書ける名前か
pub(crate) fn is_bare_key(s: &str) -> bool {
    !s.is_empty()
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

/// basic string としてエスケープして引用する
pub(crate) fn quote_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{0008}' => out.push_str("\\b"),
            '\t' => out.push_str("\\t"),
            '\n' => out.push_str("\\n"),
            '\u{000C}' => out.push_str("\\f"),
            '\r' => out.push_str("\\r"),
            c if (c as u32) < 0x20 || c == '\u{007F}' => {
                out.push_str("\\u");
                let code = c as u32;
                for shift in [12u32, 8, 4, 0] {
                    let digit = (code >> shift) & 0xF;
                    out.push(
                        char::from_digit(digit, 16)
                            .expect("0..=15")
                            .to_ascii_uppercase(),
                    );
                }
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn render_key(s: &str) -> String {
    if is_bare_key(s) {
        String::from(s)
    } else {
        quote_string(s)
    }
}

/// ルートテーブルを正規形の TOML 文字列へ描画する
pub(crate) fn render(root: &EncTable) -> String {
    let mut out = String::new();
    let mut path: Vec<String> = Vec::new();
    render_table(&mut out, root, &mut path);
    out
}

/// ヘッダ形式で出す値か（子テーブルと、要素が全部テーブルの配列）
fn is_header_form(value: &EncValue) -> bool {
    match value {
        EncValue::Table(_) => true,
        EncValue::Array(items) => is_array_of_tables(items),
        _ => false,
    }
}

/// 要素が全部テーブルの（空でない）配列 = `[[a]]` で書ける
fn is_array_of_tables(items: &[EncValue]) -> bool {
    !items.is_empty() && items.iter().all(|v| matches!(v, EncValue::Table(_)))
}

/// `[a.b]` / `[[a.b]]` のヘッダ行を書く
fn render_header(out: &mut String, path: &[String], array: bool) {
    if !out.is_empty() {
        out.push('\n');
    }
    out.push_str(if array { "[[" } else { "[" });
    for (i, seg) in path.iter().enumerate() {
        if i > 0 {
            out.push('.');
        }
        out.push_str(&render_key(seg));
    }
    out.push_str(if array { "]]\n" } else { "]\n" });
}

fn render_table(out: &mut String, table: &EncTable, path: &mut Vec<String>) {
    // ヘッダを出す前に、このテーブル自身の値を追加順で書く
    // （ヘッダ以降の `key = value` は後続のテーブルに属してしまうため）
    for (key, value) in &table.entries {
        if !is_header_form(value) {
            out.push_str(&render_key(key));
            out.push_str(" = ");
            render_value(out, value, 0);
            out.push('\n');
        }
    }
    // 子テーブルと `[[...]]` はヘッダを付けて再帰（互いの追加順は保つ）
    for (key, value) in &table.entries {
        match value {
            EncValue::Table(sub) => {
                path.push(String::from(key));
                render_header(out, path, false);
                render_table(out, sub, path);
                path.pop();
            }
            EncValue::Array(items) if is_array_of_tables(items) => {
                path.push(String::from(key));
                for item in items {
                    let EncValue::Table(sub) = item else {
                        unreachable!("全要素がテーブルであることを検査済み");
                    };
                    render_header(out, path, true);
                    render_table(out, sub, path);
                }
                path.pop();
            }
            _ => {}
        }
    }
}

/// float の正規形。Rust の `{:?}`（往復可能な最短表現）を基本に、TOML の float に
/// 必須の `.` か `e` が無ければ `.0` を補う。`-0.0` は温存、`nan` の符号は落とす
pub(crate) fn render_float(f: f64) -> String {
    if f.is_nan() {
        return String::from("nan");
    }
    if f.is_infinite() {
        return String::from(if f > 0.0 { "inf" } else { "-inf" });
    }
    let mut s = alloc::format!("{f:?}");
    if !s.contains(['.', 'e', 'E']) {
        s.push_str(".0");
    }
    s
}

fn render_value(out: &mut String, value: &EncValue, indent: usize) {
    match value {
        EncValue::String(s) => out.push_str(&quote_string(s)),
        EncValue::Integer(n) => out.push_str(&n.to_string()),
        EncValue::Float(f) => out.push_str(&render_float(*f)),
        EncValue::Boolean(b) => out.push_str(if *b { "true" } else { "false" }),
        EncValue::Datetime(dt) => out.push_str(&dt.to_string()),
        EncValue::Array(items) => render_array(out, items, indent),
        // 値位置（配列の中）のテーブルはヘッダ形式で書けないのでインラインにする
        EncValue::Table(table) => render_inline_table(out, table),
    }
}

/// インラインテーブル `{ k = v }`（空なら `{}`）。
/// 配列の中など、ヘッダ形式が使えない位置でだけ使う
fn render_inline_table(out: &mut String, table: &EncTable) {
    if table.entries.is_empty() {
        out.push_str("{}");
        return;
    }
    out.push_str("{ ");
    for (i, (key, value)) in table.entries.iter().enumerate() {
        if i > 0 {
            out.push_str(", ");
        }
        out.push_str(&render_key(key));
        out.push_str(" = ");
        render_value(out, value, 0);
    }
    out.push_str(" }");
}

fn render_array(out: &mut String, items: &[EncValue], indent: usize) {
    if items.is_empty() {
        out.push_str("[]");
        return;
    }
    let has_array = items.iter().any(|v| matches!(v, EncValue::Array(_)));
    if !has_array {
        // スカラーだけの配列は 1 行
        out.push('[');
        for (i, item) in items.iter().enumerate() {
            if i > 0 {
                out.push_str(", ");
            }
            render_value(out, item, indent);
        }
        out.push(']');
        return;
    }
    // 配列を含むネスト配列は複数行＋末尾カンマ
    out.push_str("[\n");
    let inner = indent + 2;
    for item in items {
        for _ in 0..inner {
            out.push(' ');
        }
        render_value(out, item, inner);
        out.push_str(",\n");
    }
    for _ in 0..indent {
        out.push(' ');
    }
    out.push(']');
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bare_key_の判定() {
        assert!(is_bare_key("abc-DEF_123"));
        assert!(is_bare_key("--accent")); // 先頭ハイフンも [A-Za-z0-9_-]+ に一致する
        assert!(!is_bare_key(""));
        assert!(!is_bare_key("サーバー"));
        assert!(!is_bare_key("a.b"));
    }

    #[test]
    fn 文字列のエスケープ() {
        assert_eq!(quote_string("a\"b\\c"), r#""a\"b\\c""#);
        assert_eq!(quote_string("改行\nタブ\t"), "\"改行\\nタブ\\t\"");
        assert_eq!(quote_string("\u{0001}"), "\"\\u0001\"");
    }

    #[test]
    fn float_の正規形() {
        assert_eq!(render_float(1.0), "1.0");
        assert_eq!(render_float(2.5), "2.5");
        assert_eq!(render_float(-0.0), "-0.0");
        assert_eq!(render_float(1e21), "1e21");
        assert_eq!(render_float(1e-7), "1e-7");
        assert_eq!(render_float(0.1 + 0.2), "0.30000000000000004");
        assert_eq!(render_float(f64::INFINITY), "inf");
        assert_eq!(render_float(f64::NEG_INFINITY), "-inf");
        assert_eq!(render_float(f64::NAN), "nan");
        assert_eq!(render_float(-f64::NAN), "nan");
        assert_eq!(render_float(f64::MAX), "1.7976931348623157e308");
        assert_eq!(render_float(f64::MIN_POSITIVE), "2.2250738585072014e-308");
        assert_eq!(render_float(5e-324), "5e-324");
    }
}
