//! 低レベル走査（span 付き）。
//!
//! TOML はキー位置と値位置で同じ字面の解釈が変わる（`123` は bare key でも
//! 整数でもある）ため、トークン列を先に作らず、パーサが文脈に応じて
//! 読み取りメソッドを呼ぶカーソル方式にする。
//! 値位置のスカラーは一旦「blob」として切り出してから分類し（`classify_scalar`）、
//! 整数 / 16,8,2 進整数 / float はここで値へ変換する。まだ未対応の date-time は
//! 一般構文エラーと区別して `Unsupported` にする。

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

    /// 現在位置から `b` が 3 つ連続しているか（複数行文字列の区切り）
    fn at_triple(&self, b: u8) -> bool {
        self.peek() == Some(b) && self.peek_at(1) == Some(b) && self.peek_at(2) == Some(b)
    }

    /// 現在位置から `b` が何個連続しているか
    fn count_run(&self, b: u8) -> usize {
        let mut n = 0;
        while self.peek_at(n) == Some(b) {
            n += 1;
        }
        n
    }

    /// 値位置の文字列（単行 / 複数行 × basic / literal の 4 種を振り分ける）
    pub fn read_string_value(&mut self) -> Result<(String, Span), ParseError> {
        match self.peek() {
            Some(b'"') if self.at_triple(b'"') => self.read_multiline_basic_string(),
            Some(b'"') => self.read_basic_string(),
            Some(b'\'') if self.at_triple(b'\'') => self.read_multiline_literal_string(),
            _ => self.read_literal_string(),
        }
    }

    /// 単行 basic string（`"..."`）。キー位置でも使うので `"""` はエラー
    /// （複数行文字列はキーになれない。値位置は `read_string_value` が先に振り分ける）
    pub fn read_basic_string(&mut self) -> Result<(String, Span), ParseError> {
        let start = self.pos;
        debug_assert_eq!(self.peek(), Some(b'"'));
        if self.at_triple(b'"') {
            return Err(ParseError::new(
                ParseErrorKind::MultilineStringAsKey,
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
                b'\\' => self.read_escape(start, &mut out, false)?,
                // タブ以外の制御文字は不可（TOML 1.0）
                0x00..=0x08 | 0x0B | 0x0C | 0x0E..=0x1F | 0x7F => {
                    return Err(ParseError::new(
                        ParseErrorKind::ControlCharInString,
                        Span::point(self.pos),
                    ));
                }
                _ => self.copy_char(&mut out),
            }
        }
    }

    /// 複数行 basic string（`"""..."""`）。
    /// 開始直後の改行は捨てる・行末 `\` は続く空白と改行をまとめて捨てる・
    /// 閉じ区切りの直前に `"` を 2 つまで置ける（`""""` / `"""""`）
    fn read_multiline_basic_string(&mut self) -> Result<(String, Span), ParseError> {
        let start = self.pos;
        self.pos += 3;
        if self.at_newline() {
            self.eat_newline()?;
        }
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
                    let run = self.count_run(b'"');
                    if run < 3 {
                        for _ in 0..run {
                            out.push('"');
                        }
                        self.pos += run;
                        continue;
                    }
                    if run > 5 {
                        return Err(ParseError::new(
                            ParseErrorKind::TooManyQuotes,
                            Span {
                                start: self.pos,
                                end: self.pos + run,
                            },
                        ));
                    }
                    for _ in 0..run - 3 {
                        out.push('"');
                    }
                    self.pos += run;
                    return Ok((
                        out,
                        Span {
                            start,
                            end: self.pos,
                        },
                    ));
                }
                b'\\' => self.read_escape(start, &mut out, true)?,
                b'\n' => {
                    out.push('\n');
                    self.bump();
                }
                // CRLF は原文どおり保持する（正規化出力は LF で書き出す）
                b'\r' if self.peek_at(1) == Some(b'\n') => {
                    out.push_str("\r\n");
                    self.pos += 2;
                }
                0x00..=0x08 | 0x0B..=0x1F | 0x7F => {
                    return Err(ParseError::new(
                        ParseErrorKind::ControlCharInString,
                        Span::point(self.pos),
                    ));
                }
                _ => self.copy_char(&mut out),
            }
        }
    }

    /// `\` の直後から 1 エスケープを読む（`\` は消費済みでない = 現在位置が `\`）。
    /// `multiline` なら行末 `\`（続く空白と改行をすべて捨てる）も受理する
    fn read_escape(
        &mut self,
        string_start: usize,
        out: &mut String,
        multiline: bool,
    ) -> Result<(), ParseError> {
        let esc_start = self.pos;
        self.bump(); // `\`
        let Some(e) = self.peek() else {
            return Err(ParseError::new(
                ParseErrorKind::UnterminatedString,
                Span {
                    start: string_start,
                    end: self.pos,
                },
            ));
        };
        if multiline && matches!(e, b' ' | b'\t' | b'\n' | b'\r') {
            // 行末のバックスラッシュ: 改行までは空白だけが許される
            self.skip_ws();
            if !self.at_newline() {
                return Err(ParseError::new(
                    ParseErrorKind::InvalidEscape,
                    Span {
                        start: esc_start,
                        end: self.pos,
                    },
                ));
            }
            loop {
                if self.at_newline() {
                    self.eat_newline()?;
                } else if matches!(self.peek(), Some(b' ' | b'\t')) {
                    self.bump();
                } else {
                    break;
                }
            }
            return Ok(());
        }
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
        Ok(())
    }

    /// 現在位置の UTF-8 文字を 1 つ写す
    fn copy_char(&mut self, out: &mut String) {
        let ch_start = self.pos;
        let ch = self.src[ch_start..].chars().next().expect("境界は保証済み");
        self.pos += ch.len_utf8();
        out.push(ch);
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

    /// 複数行 literal string（`'''...'''`）。エスケープなし・開始直後の改行は捨てる・
    /// 閉じ区切りの直前に `'` を 2 つまで置ける
    fn read_multiline_literal_string(&mut self) -> Result<(String, Span), ParseError> {
        let start = self.pos;
        self.pos += 3;
        if self.at_newline() {
            self.eat_newline()?;
        }
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
                b'\'' => {
                    let run = self.count_run(b'\'');
                    if run < 3 {
                        for _ in 0..run {
                            out.push('\'');
                        }
                        self.pos += run;
                        continue;
                    }
                    if run > 5 {
                        return Err(ParseError::new(
                            ParseErrorKind::TooManyQuotes,
                            Span {
                                start: self.pos,
                                end: self.pos + run,
                            },
                        ));
                    }
                    for _ in 0..run - 3 {
                        out.push('\'');
                    }
                    self.pos += run;
                    return Ok((
                        out,
                        Span {
                            start,
                            end: self.pos,
                        },
                    ));
                }
                b'\n' => {
                    out.push('\n');
                    self.bump();
                }
                b'\r' if self.peek_at(1) == Some(b'\n') => {
                    out.push_str("\r\n");
                    self.pos += 2;
                }
                0x00..=0x08 | 0x0B..=0x1F | 0x7F => {
                    return Err(ParseError::new(
                        ParseErrorKind::ControlCharInString,
                        Span::point(self.pos),
                    ));
                }
                _ => self.copy_char(&mut out),
            }
        }
    }

    /// 単行 literal string（`'...'`）。キー位置でも使うので `'''` はエラー
    pub fn read_literal_string(&mut self) -> Result<(String, Span), ParseError> {
        let start = self.pos;
        debug_assert_eq!(self.peek(), Some(b'\''));
        if self.at_triple(b'\'') {
            return Err(ParseError::new(
                ParseErrorKind::MultilineStringAsKey,
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
    /// 16 / 8 / 2 進整数として妥当（値は `parse_radix_integer` で得る）
    RadixInteger(u32),
    /// float として妥当（`inf` / `nan` 含む。値は `parse_float` で得る）
    Float,
    /// TOML として妥当だがまだ未対応（date-time）
    Unsupported(UnsupportedFeature),
    /// 整数系の書き間違い（先頭ゼロ・アンダースコア位置違反など）
    InvalidInteger,
    /// float / date-time / 進数整数の形はしているが TOML として不正
    /// （`1e` / `0xGG` / `1979-bad` など）
    InvalidLiteral,
    /// 値として解釈できない（英字始まりの未知語など）
    NotAValue,
}

/// blob を分類する。形で当たりを付けたあと字句全体を検証し、妥当なものだけを
/// 値の種別（Integer / RadixInteger / Float）か `Unsupported`（date-time）にする。
/// 不正なら `InvalidLiteral`（参照実装が構文エラーにする入力を未対応と案内しない）
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
                ScalarClass::RadixInteger(radix)
            } else {
                ScalarClass::InvalidLiteral
            };
        }
    }
    if matches!(body, "inf" | "nan") {
        return ScalarClass::Float;
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
            ScalarClass::Float
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
/// 値の範囲は月 01-12・月ごとの日数（閏年込み）・時 00-23・分 00-59・秒 00-60
/// （うるう秒）まで見る（参照実装が弾く `1979-13-01` / `1979-02-29` を妥当扱いしないため）
fn is_valid_datetime(body: &str) -> bool {
    if let Some((date, rest)) = body.split_once(['T', 't']) {
        return is_valid_date(date) && is_valid_time_with_offset(rest);
    }
    is_valid_date(body) || is_valid_time(body)
}

/// `YYYY-MM-DD`（暦として存在する日付だけを妥当とする）
fn is_valid_date(s: &str) -> bool {
    let b = s.as_bytes();
    if b.len() != 10 || b[4] != b'-' || b[7] != b'-' {
        return false;
    }
    if !b[..4].iter().all(u8::is_ascii_digit) {
        return false;
    }
    let year = b[..4]
        .iter()
        .fold(0u16, |acc, &d| acc * 10 + u16::from(d - b'0'));
    let (Some(month), Some(day)) = (two_digits(&b[5..7]), two_digits(&b[8..10])) else {
        return false;
    };
    (1..=12).contains(&month) && (1..=days_in_month(year, month)).contains(&day)
}

/// 月の日数。閏年は Gregorian 規則（4 の倍数。ただし 100 の倍数は 400 の倍数のときだけ）。
/// RFC 3339 は proleptic Gregorian なので 0000 年も 400 の倍数として閏年
fn days_in_month(year: u16, month: u8) -> u8 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
            if leap { 29 } else { 28 }
        }
        _ => 0,
    }
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

/// 分類済みの 16 / 8 / 2 進整数 blob（`0x` 等の接頭辞付き・符号なし）を i64 へ変換する。
/// i64 の正の範囲を超えるもの（`0xFFFF_FFFF_FFFF_FFFF` 等）は `IntegerOutOfRange`
pub(crate) fn parse_radix_integer(blob: &str, radix: u32, span: Span) -> Result<i64, ParseError> {
    let digits: String = blob[2..].chars().filter(|&c| c != '_').collect();
    i64::from_str_radix(&digits, radix)
        .map_err(|_| ParseError::new(ParseErrorKind::IntegerOutOfRange, span))
}

/// 分類済みの float blob を f64 へ変換する。
/// `inf` / `nan` は符号付きで受理し、`nan` の符号は落とす（`Value::Float` は符号を
/// 保持しない）。指数が大きすぎる値（`1e400`）は Rust の `f64` パーサと同じく無限大になる
pub(crate) fn parse_float(blob: &str, span: Span) -> Result<f64, ParseError> {
    let (negative, body) = match blob.strip_prefix('-') {
        Some(b) => (true, b),
        None => (false, blob.strip_prefix('+').unwrap_or(blob)),
    };
    let value = match body {
        "inf" => f64::INFINITY,
        "nan" => f64::NAN,
        _ => {
            let cleaned: String = body.chars().filter(|&c| c != '_').collect();
            cleaned
                .parse::<f64>()
                .map_err(|_| ParseError::new(ParseErrorKind::InvalidLiteral, span))?
        }
    };
    Ok(if negative && !value.is_nan() {
        -value
    } else {
        value
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn スカラー分類() {
        assert_eq!(classify_scalar("true"), ScalarClass::True);
        assert_eq!(classify_scalar("42"), ScalarClass::Integer);
        assert_eq!(classify_scalar("-17"), ScalarClass::Integer);
        assert_eq!(classify_scalar("1_000_000"), ScalarClass::Integer);
        assert_eq!(classify_scalar("3.14"), ScalarClass::Float);
        assert_eq!(classify_scalar("1e6"), ScalarClass::Float);
        assert_eq!(classify_scalar("inf"), ScalarClass::Float);
        assert_eq!(classify_scalar("0xFF"), ScalarClass::RadixInteger(16));
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

    /// 妥当なリテラルだけが値種別 / `Unsupported` になる（参照実装が構文エラーに
    /// する入力を受理したり「未対応」と案内したりしない）
    #[test]
    fn 不正なリテラルは_invalid_literal_になる() {
        // 妥当側（参照実装が受理する）
        for (src, class) in [
            ("6.02e23", ScalarClass::Float),
            ("1_000.5", ScalarClass::Float),
            ("-0.0", ScalarClass::Float),
            ("1e-3", ScalarClass::Float),
            ("1E+00", ScalarClass::Float),
            ("+inf", ScalarClass::Float),
            ("-nan", ScalarClass::Float),
            ("0xDEAD_BEEF", ScalarClass::RadixInteger(16)),
            ("0o755", ScalarClass::RadixInteger(8)),
            ("0b1010", ScalarClass::RadixInteger(2)),
            (
                "1979-05-27T07:32:00Z",
                ScalarClass::Unsupported(UnsupportedFeature::DateTime),
            ),
            (
                "1979-05-27T00:32:00.999999-07:00",
                ScalarClass::Unsupported(UnsupportedFeature::DateTime),
            ),
            (
                "1979-05-27t07:32:00+09:00",
                ScalarClass::Unsupported(UnsupportedFeature::DateTime),
            ),
            (
                "07:32:00.5",
                ScalarClass::Unsupported(UnsupportedFeature::DateTime),
            ),
            (
                "23:59:60",
                ScalarClass::Unsupported(UnsupportedFeature::DateTime),
            ),
            // 暦として存在する日付（閏年は 4 の倍数、100 の倍数は 400 の倍数のときだけ）
            (
                "2000-02-29",
                ScalarClass::Unsupported(UnsupportedFeature::DateTime),
            ),
            (
                "2024-02-29",
                ScalarClass::Unsupported(UnsupportedFeature::DateTime),
            ),
            (
                "1979-04-30",
                ScalarClass::Unsupported(UnsupportedFeature::DateTime),
            ),
            (
                "1979-01-31",
                ScalarClass::Unsupported(UnsupportedFeature::DateTime),
            ),
            (
                "0000-02-29",
                ScalarClass::Unsupported(UnsupportedFeature::DateTime),
            ),
        ] {
            assert_eq!(classify_scalar(src), class, "{src}");
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
            // 暦として存在しない日付（参照実装も拒否する）
            "1979-02-29",
            "2000-02-30",
            "1979-04-31",
            "1900-02-29",
            "2100-02-29",
            "1979-06-31",
            "1979-02-00",
            "1979-02-29T00:00:00Z",
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
    fn 三連引用符はキー位置では使えない() {
        let mut c = Cursor::new(r#""""multi""""#);
        assert_eq!(
            *c.read_basic_string().unwrap_err().kind(),
            ParseErrorKind::MultilineStringAsKey
        );
        let mut c = Cursor::new("'''multi'''");
        assert_eq!(
            *c.read_literal_string().unwrap_err().kind(),
            ParseErrorKind::MultilineStringAsKey
        );
    }

    fn value(src: &str) -> String {
        let mut c = Cursor::new(src);
        let (s, span) = c.read_string_value().unwrap();
        assert_eq!(span.start, 0);
        assert_eq!(span.end, src.len(), "文字列全体を消費する: {src:?}");
        s
    }

    fn value_err(src: &str) -> ParseErrorKind {
        Cursor::new(src)
            .read_string_value()
            .unwrap_err()
            .kind()
            .clone()
    }

    #[test]
    fn 複数行_basic_string() {
        // 開始直後の改行は捨てる。途中の改行は保持
        assert_eq!(
            value("\"\"\"\nRoses are red\nViolets are blue\"\"\""),
            "Roses are red\nViolets are blue"
        );
        assert_eq!(value("\"\"\"a\nb\"\"\""), "a\nb");
        // CRLF は原文どおり
        assert_eq!(value("\"\"\"\r\na\r\nb\"\"\""), "a\r\nb");
        // 行末 `\` は続く空白と改行をすべて捨てる
        assert_eq!(
            value("\"\"\"\nThe quick brown \\\n\n\n  fox jumps over \\\n    the lazy dog.\"\"\""),
            "The quick brown fox jumps over the lazy dog."
        );
        assert_eq!(value("\"\"\"a \\   \n   b\"\"\""), "a b");
        // 引用符は 1〜2 個なら中に書ける。閉じ直前も 2 個まで
        assert_eq!(
            value("\"\"\"Here are two quotation marks: \"\". Simple.\"\"\""),
            "Here are two quotation marks: \"\". Simple."
        );
        assert_eq!(
            value("\"\"\"\"This,\" she said, \"is just a pointless statement.\"\"\"\""),
            "\"This,\" she said, \"is just a pointless statement.\""
        );
        assert_eq!(value("\"\"\"a\"\"\"\"\""), "a\"\"");
        // エスケープは単行と同じ
        assert_eq!(value("\"\"\"a\\tb\\u3042\\\"\"\"\""), "a\tbあ\"");
        // 空
        assert_eq!(value("\"\"\"\"\"\""), "");
    }

    #[test]
    fn 複数行_basic_string_のエラー() {
        assert_eq!(
            value_err("\"\"\"a\"\"\"\"\"\""),
            ParseErrorKind::TooManyQuotes
        );
        assert_eq!(value_err("\"\"\"a"), ParseErrorKind::UnterminatedString);
        assert_eq!(
            value_err("\"\"\"a\\ b\"\"\""),
            ParseErrorKind::InvalidEscape
        );
        assert_eq!(
            value_err("\"\"\"a\\qb\"\"\""),
            ParseErrorKind::InvalidEscape
        );
        assert_eq!(
            value_err("\"\"\"a\u{0007}\"\"\""),
            ParseErrorKind::ControlCharInString
        );
        // 単独の CR は制御文字
        assert_eq!(
            value_err("\"\"\"a\rb\"\"\""),
            ParseErrorKind::ControlCharInString
        );
    }

    #[test]
    fn 複数行_literal_string() {
        assert_eq!(
            value("'''\nThe first newline is\ntrimmed in raw strings.\n'''"),
            "The first newline is\ntrimmed in raw strings.\n"
        );
        // エスケープしない
        assert_eq!(
            value("'''C:\\Users\\nodejs\\templates'''"),
            "C:\\Users\\nodejs\\templates"
        );
        assert_eq!(value("'''\\ \n  x'''"), "\\ \n  x");
        // 引用符は 1〜2 個なら中に書ける。閉じ直前も 2 個まで
        assert_eq!(
            value("''''That,' she said, 'is still pointless.''''"),
            "'That,' she said, 'is still pointless.'"
        );
        assert_eq!(value("'''a'''''"), "a''");
        assert_eq!(value("'''\r\na\r\n'''"), "a\r\n");
        assert_eq!(value_err("'''a''''''"), ParseErrorKind::TooManyQuotes);
        assert_eq!(value_err("'''a"), ParseErrorKind::UnterminatedString);
        assert_eq!(
            value_err("'''a\u{0001}'''"),
            ParseErrorKind::ControlCharInString
        );
    }

    #[test]
    fn float_の変換() {
        let span = Span { start: 0, end: 1 };
        let f = |s: &str| parse_float(s, span).unwrap();
        assert_eq!(f("2.5"), 2.5);
        assert_eq!(f("+1.0"), 1.0);
        assert_eq!(f("-0.01"), -0.01);
        assert_eq!(f("5e+22"), 5e22);
        assert_eq!(f("1e06"), 1e6);
        assert_eq!(f("-2E-2"), -2e-2);
        assert_eq!(f("6.626e-34"), 6.626e-34);
        assert_eq!(f("224_617.445_991_228"), 224617.445991228);
        assert_eq!(f("inf"), f64::INFINITY);
        assert_eq!(f("+inf"), f64::INFINITY);
        assert_eq!(f("-inf"), f64::NEG_INFINITY);
        assert!(f("nan").is_nan());
        assert!(f("+nan").is_nan());
        // nan の符号は落とす
        assert!(f("-nan").is_nan() && !f("-nan").is_sign_negative());
        // -0.0 は温存
        assert!(f("-0.0").is_sign_negative());
        // 指数が大きすぎる値は無限大（Rust の f64 パーサと同じ）
        assert_eq!(f("1e400"), f64::INFINITY);
    }

    #[test]
    fn 進数整数の変換() {
        let span = Span { start: 0, end: 1 };
        assert_eq!(
            parse_radix_integer("0xDEADBEEF", 16, span).unwrap(),
            0xDEAD_BEEF
        );
        assert_eq!(
            parse_radix_integer("0xdead_beef", 16, span).unwrap(),
            0xDEAD_BEEF
        );
        assert_eq!(parse_radix_integer("0o755", 8, span).unwrap(), 0o755);
        assert_eq!(
            parse_radix_integer("0b1101_0110", 2, span).unwrap(),
            0b1101_0110
        );
        assert_eq!(
            parse_radix_integer("0x7FFF_FFFF_FFFF_FFFF", 16, span).unwrap(),
            i64::MAX
        );
        // i64 の正の範囲を超える
        assert!(parse_radix_integer("0x8000_0000_0000_0000", 16, span).is_err());
        assert!(parse_radix_integer("0xFFFF_FFFF_FFFF_FFFF", 16, span).is_err());
    }
}
