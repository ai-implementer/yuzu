//! 低レベル走査（span 付き）。
//!
//! TOML はキー位置と値位置で同じ字面の解釈が変わる（`123` は bare key でも
//! 整数でもある）ため、トークン列を先に作らず、パーサが文脈に応じて
//! 読み取りメソッドを呼ぶカーソル方式にする。
//! 値位置のスカラーは一旦「blob」として切り出してから分類し、
//! v0.1 未対応構文（float / date-time / 16,8,2 進整数）を
//! 一般構文エラーと区別する（`classify_scalar`）。

use alloc::string::String;

use crate::error::{ParseError, ParseErrorKind, UnsupportedFeature};
use crate::model::{KeySegment, Span};

pub(crate) struct Cursor<'a> {
    src: &'a str,
    pos: usize,
}

impl<'a> Cursor<'a> {
    pub fn new(src: &'a str) -> Self {
        Self { src, pos: 0 }
    }

    pub fn pos(&self) -> usize {
        self.pos
    }

    pub fn is_eof(&self) -> bool {
        self.pos >= self.src.len()
    }

    pub fn peek(&self) -> Option<u8> {
        self.src.as_bytes().get(self.pos).copied()
    }

    fn peek_at(&self, n: usize) -> Option<u8> {
        self.src.as_bytes().get(self.pos + n).copied()
    }

    fn bump(&mut self) {
        self.pos += 1;
    }

    /// 次のバイトが `b` なら消費して true
    pub fn eat(&mut self, b: u8) -> bool {
        if self.peek() == Some(b) {
            self.bump();
            true
        } else {
            false
        }
    }

    /// 半角スペースとタブを読み飛ばす
    pub fn skip_ws(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\t')) {
            self.bump();
        }
    }

    pub fn at_comment(&self) -> bool {
        self.peek() == Some(b'#')
    }

    /// コメントを行末（改行の手前）まで読み、span を返す。CR は含めない
    pub fn read_comment(&mut self) -> Span {
        let start = self.pos;
        while let Some(c) = self.peek() {
            if c == b'\n' {
                break;
            }
            self.bump();
        }
        let mut end = self.pos;
        if end > start && self.src.as_bytes()[end - 1] == b'\r' {
            end -= 1;
        }
        Span { start, end }
    }

    /// 改行（LF / CRLF）を 1 つ消費する。EOF も改行相当として受理する
    pub fn eat_newline(&mut self) -> Result<(), ParseError> {
        match self.peek() {
            None => Ok(()),
            Some(b'\n') => {
                self.bump();
                Ok(())
            }
            Some(b'\r') if self.peek_at(1) == Some(b'\n') => {
                self.bump();
                self.bump();
                Ok(())
            }
            Some(_) => Err(ParseError::new(
                ParseErrorKind::ExpectedNewline,
                Span::point(self.pos),
            )),
        }
    }

    pub fn at_newline(&self) -> bool {
        matches!(self.peek(), Some(b'\n'))
            || (self.peek() == Some(b'\r') && self.peek_at(1) == Some(b'\n'))
    }

    /// キーセグメント 1 つ（bare / basic 引用 / literal 引用）を読む
    pub fn read_key_segment(&mut self) -> Result<KeySegment, ParseError> {
        match self.peek() {
            Some(b'"') => {
                let (s, span) = self.read_basic_string()?;
                Ok(KeySegment::new(s, span))
            }
            Some(b'\'') => {
                let (s, span) = self.read_literal_string()?;
                Ok(KeySegment::new(s, span))
            }
            _ => {
                let start = self.pos;
                while let Some(c) = self.peek() {
                    if c.is_ascii_alphanumeric() || c == b'_' || c == b'-' {
                        self.bump();
                    } else {
                        break;
                    }
                }
                if self.pos == start {
                    return Err(ParseError::new(
                        ParseErrorKind::ExpectedKey,
                        Span::point(start),
                    ));
                }
                let span = Span {
                    start,
                    end: self.pos,
                };
                Ok(KeySegment::new(
                    String::from(&self.src[start..self.pos]),
                    span,
                ))
            }
        }
    }

    /// 単行 basic string（`"..."`）。`"""` は v0.1 未対応として区別する
    pub fn read_basic_string(&mut self) -> Result<(String, Span), ParseError> {
        let start = self.pos;
        debug_assert_eq!(self.peek(), Some(b'"'));
        if self.peek_at(1) == Some(b'"') && self.peek_at(2) == Some(b'"') {
            return Err(ParseError::new(
                ParseErrorKind::Unsupported(UnsupportedFeature::MultilineString),
                Span {
                    start,
                    end: start + 3,
                },
            ));
        }
        self.bump(); // 開き引用符
        let mut out = String::new();
        loop {
            let Some(c) = self.peek() else {
                return Err(ParseError::new(
                    ParseErrorKind::UnterminatedString,
                    Span {
                        start,
                        end: self.pos,
                    },
                ));
            };
            match c {
                b'"' => {
                    self.bump();
                    return Ok((
                        out,
                        Span {
                            start,
                            end: self.pos,
                        },
                    ));
                }
                b'\n' | b'\r' => {
                    return Err(ParseError::new(
                        ParseErrorKind::UnterminatedString,
                        Span {
                            start,
                            end: self.pos,
                        },
                    ));
                }
                b'\\' => {
                    let esc_start = self.pos;
                    self.bump();
                    let Some(e) = self.peek() else {
                        return Err(ParseError::new(
                            ParseErrorKind::UnterminatedString,
                            Span {
                                start,
                                end: self.pos,
                            },
                        ));
                    };
                    self.bump();
                    match e {
                        b'b' => out.push('\u{0008}'),
                        b't' => out.push('\t'),
                        b'n' => out.push('\n'),
                        b'f' => out.push('\u{000C}'),
                        b'r' => out.push('\r'),
                        b'"' => out.push('"'),
                        b'\\' => out.push('\\'),
                        b'u' => out.push(self.read_unicode_escape(esc_start, 4)?),
                        b'U' => out.push(self.read_unicode_escape(esc_start, 8)?),
                        _ => {
                            return Err(ParseError::new(
                                ParseErrorKind::InvalidEscape,
                                Span {
                                    start: esc_start,
                                    end: self.pos,
                                },
                            ));
                        }
                    }
                }
                // タブ以外の制御文字は不可（TOML 1.0）
                0x00..=0x08 | 0x0B | 0x0C | 0x0E..=0x1F | 0x7F => {
                    return Err(ParseError::new(
                        ParseErrorKind::ControlCharInString,
                        Span::point(self.pos),
                    ));
                }
                _ => {
                    // UTF-8 マルチバイトはそのまま写す
                    let ch_start = self.pos;
                    let ch = self.src[ch_start..].chars().next().expect("境界は保証済み");
                    self.pos += ch.len_utf8();
                    out.push(ch);
                }
            }
        }
    }

    fn read_unicode_escape(&mut self, esc_start: usize, digits: usize) -> Result<char, ParseError> {
        let hex_start = self.pos;
        for _ in 0..digits {
            match self.peek() {
                Some(c) if c.is_ascii_hexdigit() => self.bump(),
                _ => {
                    return Err(ParseError::new(
                        ParseErrorKind::InvalidUnicodeEscape,
                        Span {
                            start: esc_start,
                            end: self.pos,
                        },
                    ));
                }
            }
        }
        let code = u32::from_str_radix(&self.src[hex_start..self.pos], 16)
            .expect("16 進数字のみ読んでいる");
        char::from_u32(code).ok_or_else(|| {
            ParseError::new(
                ParseErrorKind::InvalidUnicodeEscape,
                Span {
                    start: esc_start,
                    end: self.pos,
                },
            )
        })
    }

    /// 単行 literal string（`'...'`）。`'''` は v0.1 未対応として区別する
    pub fn read_literal_string(&mut self) -> Result<(String, Span), ParseError> {
        let start = self.pos;
        debug_assert_eq!(self.peek(), Some(b'\''));
        if self.peek_at(1) == Some(b'\'') && self.peek_at(2) == Some(b'\'') {
            return Err(ParseError::new(
                ParseErrorKind::Unsupported(UnsupportedFeature::MultilineString),
                Span {
                    start,
                    end: start + 3,
                },
            ));
        }
        self.bump();
        let content_start = self.pos;
        loop {
            let Some(c) = self.peek() else {
                return Err(ParseError::new(
                    ParseErrorKind::UnterminatedString,
                    Span {
                        start,
                        end: self.pos,
                    },
                ));
            };
            match c {
                b'\'' => {
                    let s = String::from(&self.src[content_start..self.pos]);
                    self.bump();
                    return Ok((
                        s,
                        Span {
                            start,
                            end: self.pos,
                        },
                    ));
                }
                b'\n' | b'\r' => {
                    return Err(ParseError::new(
                        ParseErrorKind::UnterminatedString,
                        Span {
                            start,
                            end: self.pos,
                        },
                    ));
                }
                // タブ以外の制御文字は不可
                0x00..=0x08 | 0x0B..=0x1F | 0x7F => {
                    return Err(ParseError::new(
                        ParseErrorKind::ControlCharInString,
                        Span::point(self.pos),
                    ));
                }
                _ => self.bump(),
            }
        }
    }

    /// 値位置のスカラー字句（数値・真偽値・日付など）を 1 塊として切り出す
    pub fn read_scalar_blob(&mut self) -> (&'a str, Span) {
        let start = self.pos;
        while let Some(c) = self.peek() {
            if c.is_ascii_alphanumeric() || matches!(c, b'_' | b'+' | b'-' | b':' | b'.') {
                self.bump();
            } else {
                break;
            }
        }
        (
            &self.src[start..self.pos],
            Span {
                start,
                end: self.pos,
            },
        )
    }
}

/// 値位置スカラーの分類結果
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum ScalarClass {
    True,
    False,
    /// TOML の 10 進整数として妥当（値は `parse_integer` で得る）
    Integer,
    Unsupported(UnsupportedFeature),
    /// 整数系の書き間違い（先頭ゼロ・アンダースコア位置違反など）
    InvalidInteger,
    /// 値として解釈できない（英字始まりの未知語など）
    NotAValue,
}

/// blob を分類する。v0.1 未対応（float / date-time / 16,8,2 進）を
/// 一般構文エラーと区別するのが目的
pub(crate) fn classify_scalar(blob: &str) -> ScalarClass {
    match blob {
        "true" => return ScalarClass::True,
        "false" => return ScalarClass::False,
        "" => return ScalarClass::NotAValue,
        _ => {}
    }
    let body = blob.strip_prefix(['+', '-']).unwrap_or(blob);
    if body.starts_with("0x") || body.starts_with("0o") || body.starts_with("0b") {
        return ScalarClass::Unsupported(UnsupportedFeature::RadixInteger);
    }
    if matches!(body, "inf" | "nan") {
        return ScalarClass::Unsupported(UnsupportedFeature::Float);
    }
    if !body.starts_with(|c: char| c.is_ascii_digit()) {
        return ScalarClass::NotAValue;
    }
    if body.contains(['.', 'e', 'E']) {
        return ScalarClass::Unsupported(UnsupportedFeature::Float);
    }
    // 日付（1979-05-27）と時刻（07:32:00）の形だけを date-time と判定する
    let bytes = body.as_bytes();
    let looks_date =
        bytes.len() > 4 && bytes[..4].iter().all(u8::is_ascii_digit) && bytes[4] == b'-';
    let looks_time =
        bytes.len() > 2 && bytes[..2].iter().all(u8::is_ascii_digit) && bytes[2] == b':';
    if looks_date || looks_time {
        return ScalarClass::Unsupported(UnsupportedFeature::DateTime);
    }
    if is_valid_integer(body) {
        ScalarClass::Integer
    } else {
        ScalarClass::InvalidInteger
    }
}

/// TOML の 10 進整数本体（符号除去済み）の字句検証。
/// アンダースコアは数字の間のみ・先頭ゼロ不可
fn is_valid_integer(body: &str) -> bool {
    let bytes = body.as_bytes();
    if bytes.is_empty() {
        return false;
    }
    let mut prev_digit = false;
    for (i, &b) in bytes.iter().enumerate() {
        match b {
            b'0'..=b'9' => prev_digit = true,
            b'_' => {
                let next_digit = bytes.get(i + 1).is_some_and(u8::is_ascii_digit);
                if !prev_digit || !next_digit {
                    return false;
                }
                prev_digit = false;
            }
            _ => return false,
        }
    }
    if !prev_digit {
        return false;
    }
    // 先頭ゼロの禁止（"0" 単独は可）
    let digits: alloc::vec::Vec<u8> = bytes.iter().copied().filter(u8::is_ascii_digit).collect();
    !(digits.len() > 1 && digits[0] == b'0')
}

/// 分類済みの整数 blob を i64 へ変換する（範囲外は `IntegerOutOfRange`）
pub(crate) fn parse_integer(blob: &str, span: Span) -> Result<i64, ParseError> {
    let cleaned: String = blob.chars().filter(|&c| c != '_').collect();
    cleaned
        .parse::<i64>()
        .map_err(|_| ParseError::new(ParseErrorKind::IntegerOutOfRange, span))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn スカラー分類が_v0_1_未対応を区別する() {
        assert_eq!(classify_scalar("true"), ScalarClass::True);
        assert_eq!(classify_scalar("42"), ScalarClass::Integer);
        assert_eq!(classify_scalar("-17"), ScalarClass::Integer);
        assert_eq!(classify_scalar("1_000_000"), ScalarClass::Integer);
        assert_eq!(
            classify_scalar("3.14"),
            ScalarClass::Unsupported(UnsupportedFeature::Float)
        );
        assert_eq!(
            classify_scalar("1e6"),
            ScalarClass::Unsupported(UnsupportedFeature::Float)
        );
        assert_eq!(
            classify_scalar("inf"),
            ScalarClass::Unsupported(UnsupportedFeature::Float)
        );
        assert_eq!(
            classify_scalar("0xFF"),
            ScalarClass::Unsupported(UnsupportedFeature::RadixInteger)
        );
        assert_eq!(
            classify_scalar("1979-05-27"),
            ScalarClass::Unsupported(UnsupportedFeature::DateTime)
        );
        assert_eq!(
            classify_scalar("07:32:00"),
            ScalarClass::Unsupported(UnsupportedFeature::DateTime)
        );
        assert_eq!(classify_scalar("042"), ScalarClass::InvalidInteger);
        assert_eq!(classify_scalar("1__0"), ScalarClass::InvalidInteger);
        assert_eq!(classify_scalar("_1"), ScalarClass::NotAValue);
        assert_eq!(classify_scalar("hello"), ScalarClass::NotAValue);
    }

    #[test]
    fn 整数の範囲外は_out_of_range() {
        let span = Span { start: 0, end: 1 };
        assert_eq!(
            parse_integer("9223372036854775807", span).unwrap(),
            i64::MAX
        );
        assert!(parse_integer("9223372036854775808", span).is_err());
        assert_eq!(
            parse_integer("-9223372036854775808", span).unwrap(),
            i64::MIN
        );
    }

    #[test]
    fn basic_string_のエスケープと制御文字() {
        let mut c = Cursor::new(r#""a\tb\u3042""#);
        let (s, span) = c.read_basic_string().unwrap();
        assert_eq!(s, "a\tbあ");
        assert_eq!(span.start, 0);
        assert!(c.is_eof());

        let mut c = Cursor::new("\"a\u{0007}b\"");
        let e = c.read_basic_string().unwrap_err();
        assert_eq!(*e.kind(), ParseErrorKind::ControlCharInString);

        let mut c = Cursor::new(r#""\uD800""#);
        let e = c.read_basic_string().unwrap_err();
        assert_eq!(*e.kind(), ParseErrorKind::InvalidUnicodeEscape);

        let mut c = Cursor::new(r#""未終端"#);
        assert!(matches!(
            c.read_basic_string().unwrap_err().kind(),
            ParseErrorKind::UnterminatedString
        ));
    }

    #[test]
    fn literal_string_はエスケープしない() {
        let mut c = Cursor::new(r"'C:\path\to'");
        let (s, _) = c.read_literal_string().unwrap();
        assert_eq!(s, r"C:\path\to");
    }

    #[test]
    fn 三連引用符は未対応として区別される() {
        let mut c = Cursor::new(r#""""multi""""#);
        assert!(matches!(
            c.read_basic_string().unwrap_err().kind(),
            ParseErrorKind::Unsupported(UnsupportedFeature::MultilineString)
        ));
    }
}
