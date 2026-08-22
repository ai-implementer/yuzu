//! 正規化出力（同じ値から常に同じバイト列）。
//!
//! - 改行は LF、末尾にも改行を付ける
//! - 空配列は `[]`、スカラーだけの配列は 1 行、配列を含むネスト配列は
//!   スペース 2 個のインデントと末尾カンマ（行幅による自動折り返しはしない）
//! - 文字列は常に basic string。引用符・バックスラッシュ・制御文字だけを
//!   エスケープし、表示可能な Unicode はそのまま出力する
//! - `[A-Za-z0-9_-]+` に一致するキーは bare key、それ以外は basic string で引用
//! - 親テーブルの値を子テーブルより先に出力し、各グループでは追加順を維持する

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

fn render_table(out: &mut String, table: &EncTable, path: &mut Vec<String>) {
    // 親テーブルの値（非テーブル）を先に、追加順で
    for (key, value) in &table.entries {
        if !matches!(value, EncValue::Table(_)) {
            out.push_str(&render_key(key));
            out.push_str(" = ");
            render_value(out, value, 0);
            out.push('\n');
        }
    }
    // 子テーブルはヘッダを付けて再帰
    for (key, value) in &table.entries {
        let EncValue::Table(sub) = value else {
            continue;
        };
        path.push(String::from(key));
        if !out.is_empty() {
            out.push('\n');
        }
        out.push('[');
        for (i, seg) in path.iter().enumerate() {
            if i > 0 {
                out.push('.');
            }
            out.push_str(&render_key(seg));
        }
        out.push_str("]\n");
        render_table(out, sub, path);
        path.pop();
    }
}

fn render_value(out: &mut String, value: &EncValue, indent: usize) {
    match value {
        EncValue::String(s) => out.push_str(&quote_string(s)),
        EncValue::Integer(n) => out.push_str(&n.to_string()),
        EncValue::Boolean(b) => out.push_str(if *b { "true" } else { "false" }),
        EncValue::Array(items) => render_array(out, items, indent),
        EncValue::Table(_) => unreachable!("テーブルは render_table 側で描画する"),
    }
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
}
