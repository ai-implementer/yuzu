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
    /// float / date-time / 進数整数の形はしているが TOML として不正
    /// （`1e` / `0xGG` / `1979-bad` など。`Unsupported` にすると誤った書き換え案内になる）
    InvalidLiteral,
    /// 値として解釈できない（英字始まりの未知語など）
    NotAValue,
}

/// blob を分類する。v0.1 未対応（float / date-time / 16,8,2 進）を
/// 一般構文エラーと区別するのが目的。**`Unsupported` は TOML 1.0 として妥当な
/// リテラルに限る** — 形で当たりを付けたあと字句全体を検証し、不正なら
/// `InvalidLiteral`（参照実装が構文エラーにする入力を未対応と案内しない）
pub(crate) fn classify_scalar(blob: &str) -> ScalarClass {
    match blob {
        "true" => return ScalarClass::True,
        "false" => return ScalarClass::False,
        "" => return ScalarClass::NotAValue,
        _ => {}
    }
    let signed = blob.starts_with(['+', '-']);
    let body = blob.strip_prefix(['+', '-']).unwrap_or(blob);
    // 16 / 8 / 2 進整数（符号は付けられない）
    if let Some((prefix, rest)) = body.split_at_checked(2) {
        let radix = match prefix {
            "0x" => Some(16),
            "0o" => Some(8),
            "0b" => Some(2),
            _ => None,
        };
        if let Some(radix) = radix {
            return if !signed && is_digit_run(rest, radix) {
                ScalarClass::Unsupported(UnsupportedFeature::RadixInteger)
            } else {
                ScalarClass::InvalidLiteral
            };
        }
    }
    if matches!(body, "inf" | "nan") {
        return ScalarClass::Unsupported(UnsupportedFeature::Float);
    }
    if !body.starts_with(|c: char| c.is_ascii_digit()) {
        return ScalarClass::NotAValue;
    }
    // 日付（1979-05-27）と時刻（07:32:00）の形は date-time として検証する。
    // float より先に見るのは、local time が小数秒（07:32:00.5）で `.` を含むため。
    // 空白区切りの date-time は blob が空白で切れて date 単体になるが、
    // 最初のエラーで停止する規約上 Unsupported(DateTime) で足りる
    let bytes = body.as_bytes();
    let looks_date =
        bytes.len() > 4 && bytes[..4].iter().all(u8::is_ascii_digit) && bytes[4] == b'-';
    let looks_time =
        bytes.len() > 2 && bytes[..2].iter().all(u8::is_ascii_digit) && bytes[2] == b':';
    if looks_date || looks_time {
        return if !signed && is_valid_datetime(body) {
            ScalarClass::Unsupported(UnsupportedFeature::DateTime)
        } else {
            ScalarClass::InvalidLiteral
        };
    }
    if body.contains(['.', 'e', 'E']) {
        return if is_valid_float(body) {
            ScalarClass::Unsupported(UnsupportedFeature::Float)
        } else {
            ScalarClass::InvalidLiteral
        };
    }
    if is_valid_integer(body) {
        ScalarClass::Integer
    } else {
        ScalarClass::InvalidInteger
    }
}

/// 数字列の字句検証（非空・`_` は数字の間のみ）。radix は 2 / 8 / 10 / 16
fn is_digit_run(s: &str, radix: u32) -> bool {
    let mut prev_digit = false;
    let mut any = false;
    for (i, c) in s.char_indices() {
        if c.is_digit(radix) {
            prev_digit = true;
            any = true;
        } else if c == '_' {
            let next_digit = s[i + 1..].chars().next().is_some_and(|n| n.is_digit(radix));
            if !prev_digit || !next_digit {
                return false;
            }
            prev_digit = false;
        } else {
            return false;
        }
    }
    any && prev_digit
}

/// TOML の 10 進整数本体（符号除去済み）の字句検証。
/// アンダースコアは数字の間のみ・先頭ゼロ不可（"0" 単独は可）
fn is_valid_integer(body: &str) -> bool {
    if !is_digit_run(body, 10) {
        return false;
    }
    let mut digits = body.bytes().filter(u8::is_ascii_digit);
    !(digits.next() == Some(b'0') && digits.next().is_some())
}

/// TOML の float 本体（符号除去済み・inf / nan 以外）: `int (frac | exp | frac exp)`。
/// int は整数と同じ規則、frac は `.` ＋ 数字列、exp は `[eE][+-]?` ＋ 数字列
/// （指数部は先頭ゼロ可）
fn is_valid_float(body: &str) -> bool {
    let (mantissa, exp) = match body.find(['e', 'E']) {
        Some(i) => (&body[..i], Some(&body[i + 1..])),
        None => (body, None),
    };
    let (int, frac) = match mantissa.split_once('.') {
        Some((i, f)) => (i, Some(f)),
        None => (mantissa, None),
    };
    if !is_valid_integer(int) {
        return false;
    }
    if let Some(f) = frac {
        if !is_digit_run(f, 10) {
            return false;
        }
    }
    if let Some(e) = exp {
        let e = e.strip_prefix(['+', '-']).unwrap_or(e);
        if !is_digit_run(e, 10) {
            return false;
        }
    }
    frac.is_some() || exp.is_some()
}

/// RFC 3339 の形（local date / local time / `T` 区切りの date-time ＋ 任意の offset）。
/// 値の範囲は月 01-12・日 01-31・時 00-23・分 00-59・秒 00-60（うるう秒）まで見る
/// （参照実装が弾く `1979-13-01` を妥当扱いしないため）。暦の妥当性（2 月 30 日等）は見ない
fn is_valid_datetime(body: &str) -> bool {
    if let Some((date, rest)) = body.split_once(['T', 't']) {
        return is_valid_date(date) && is_valid_time_with_offset(rest);
    }
    is_valid_date(body) || is_valid_time(body)
}

/// `YYYY-MM-DD`
fn is_valid_date(s: &str) -> bool {
    let b = s.as_bytes();
    if b.len() != 10 || b[4] != b'-' || b[7] != b'-' {
        return false;
    }
    let month = two_digits(&b[5..7]);
    let day = two_digits(&b[8..10]);
    b[..4].iter().all(u8::is_ascii_digit)
        && month.is_some_and(|m| (1..=12).contains(&m))
        && day.is_some_and(|d| (1..=31).contains(&d))
}

/// `HH:MM:SS` ＋ 任意の `.` 小数秒
fn is_valid_time(s: &str) -> bool {
    let (clock, frac) = match s.split_once('.') {
        Some((c, f)) => (c, Some(f)),
        None => (s, None),
    };
    let b = clock.as_bytes();
    if b.len() != 8 || b[2] != b':' || b[5] != b':' {
        return false;
    }
    let hour = two_digits(&b[0..2]);
    let minute = two_digits(&b[3..5]);
    let second = two_digits(&b[6..8]);
    hour.is_some_and(|h| h <= 23)
        && minute.is_some_and(|m| m <= 59)
        && second.is_some_and(|s| s <= 60)
        && frac.is_none_or(|f| !f.is_empty() && f.bytes().all(|c| c.is_ascii_digit()))
}

/// time ＋ 任意の offset（`Z` / `z` / `±HH:MM`）
fn is_valid_time_with_offset(s: &str) -> bool {
    if let Some(t) = s.strip_suffix(['Z', 'z']) {
        return is_valid_time(t);
    }
    if s.len() > 6 {
        let (t, off) = s.split_at(s.len() - 6);
        let b = off.as_bytes();
        if matches!(b[0], b'+' | b'-') && b[3] == b':' {
            let hour = two_digits(&b[1..3]);
            let minute = two_digits(&b[4..6]);
            return hour.is_some_and(|h| h <= 23)
                && minute.is_some_and(|m| m <= 59)
                && is_valid_time(t);
        }
    }
    is_valid_time(s)
}

/// 2 桁の ASCII 数字を数値へ（それ以外は None）
fn two_digits(b: &[u8]) -> Option<u8> {
    match b {
        [a, c] if a.is_ascii_digit() && c.is_ascii_digit() => Some((a - b'0') * 10 + (c - b'0')),
        _ => None,
    }
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

    /// `Unsupported` は TOML として妥当なリテラルに限る（参照実装が構文エラーに
    /// する入力を「未対応」と案内しない）
    #[test]
    fn 不正なリテラルは_unsupported_にならない() {
        // 妥当側（参照実装が受理する）
        for (src, feature) in [
            ("6.02e23", UnsupportedFeature::Float),
            ("1_000.5", UnsupportedFeature::Float),
            ("-0.0", UnsupportedFeature::Float),
            ("1e-3", UnsupportedFeature::Float),
            ("1E+00", UnsupportedFeature::Float),
            ("+inf", UnsupportedFeature::Float),
            ("0xDEAD_BEEF", UnsupportedFeature::RadixInteger),
            ("0o755", UnsupportedFeature::RadixInteger),
            ("0b1010", UnsupportedFeature::RadixInteger),
            ("1979-05-27T07:32:00Z", UnsupportedFeature::DateTime),
            (
                "1979-05-27T00:32:00.999999-07:00",
                UnsupportedFeature::DateTime,
            ),
            ("1979-05-27t07:32:00+09:00", UnsupportedFeature::DateTime),
            ("07:32:00.5", UnsupportedFeature::DateTime),
            ("23:59:60", UnsupportedFeature::DateTime),
        ] {
            assert_eq!(
                classify_scalar(src),
                ScalarClass::Unsupported(feature),
                "{src}"
            );
        }
        // 不正側（参照実装も構文エラー）
        for src in [
            "1e",
            "1.",
            "1.e5",
            "1_.0",
            "1._5",
            "01.5",
            "1e5.5",
            "0xGG",
            "0x",
            "+0x1",
            "0b12",
            "0o_7",
            "1979-bad",
            "1979-13-01",
            "1979-05-32",
            "1979-5-27",
            "07:60:00",
            "24:00:00",
            "07:32",
            "07:32:00.",
            "1979-05-27T",
            "1979-05-27T07:32:00+9:00",
            "1979-05-27T07:32:00-24:00",
            "-1979-05-27",
        ] {
            assert_eq!(classify_scalar(src), ScalarClass::InvalidLiteral, "{src}");
        }
        // 数字で始まらないものは従来どおり NotAValue（= ExpectedValue）
        assert_eq!(classify_scalar(".5"), ScalarClass::NotAValue);
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
