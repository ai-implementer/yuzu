//! パースエラー。
//!
//! 構文エラーは最初の 1 件で停止する（kabosu.md「型変換と診断」）。
//! v0.1 で未対応の構文は一般的な構文エラーにせず、`Unsupported` として
//! 区別できる形で返す。エラー文は英語（利用側で `kind` から翻訳できる）。

use crate::model::Span;

/// 構文エラー（位置付き）。最初の 1 件で停止する
#[derive(Debug, Clone)]
pub struct ParseError {
    kind: ParseErrorKind,
    span: Span,
    /// 重複キー・テーブル競合のときの先行定義の位置
    prev_span: Option<Span>,
}

impl ParseError {
    pub(crate) fn new(kind: ParseErrorKind, span: Span) -> Self {
        Self {
            kind,
            span,
            prev_span: None,
        }
    }

    pub(crate) fn with_previous(kind: ParseErrorKind, span: Span, prev: Span) -> Self {
        Self {
            kind,
            span,
            prev_span: Some(prev),
        }
    }

    pub fn kind(&self) -> &ParseErrorKind {
        &self.kind
    }

    pub fn span(&self) -> Span {
        self.span
    }

    /// 重複キー・テーブル競合のとき、先行定義の位置
    pub fn previous_span(&self) -> Option<Span> {
        self.prev_span
    }
}

impl core::fmt::Display for ParseError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match &self.kind {
            ParseErrorKind::UnterminatedString => f.write_str("unterminated string"),
            ParseErrorKind::InvalidEscape => f.write_str("invalid escape sequence"),
            ParseErrorKind::InvalidUnicodeEscape => {
                f.write_str("invalid unicode escape (not a Unicode scalar value)")
            }
            ParseErrorKind::ControlCharInString => {
                f.write_str("control character is not allowed in a string")
            }
            ParseErrorKind::ExpectedKey => f.write_str("expected a key"),
            ParseErrorKind::ExpectedValue => f.write_str("expected a value"),
            ParseErrorKind::ExpectedEquals => f.write_str("expected `=` after the key"),
            ParseErrorKind::ExpectedNewline => f.write_str("expected a newline"),
            ParseErrorKind::UnclosedArray => f.write_str("unclosed array (missing `]`)"),
            ParseErrorKind::UnclosedTableHeader => {
                f.write_str("unclosed table header (missing `]`)")
            }
            ParseErrorKind::EmptyKey => f.write_str("empty key"),
            ParseErrorKind::IntegerOutOfRange => f.write_str("integer out of range for i64"),
            ParseErrorKind::InvalidInteger => f.write_str("invalid integer literal"),
            ParseErrorKind::InvalidLiteral => {
                f.write_str("invalid literal (not a valid TOML value)")
            }
            ParseErrorKind::DuplicateKey => f.write_str("duplicate key"),
            ParseErrorKind::TableConflict => {
                f.write_str("table conflicts with a previously defined key or table")
            }
            ParseErrorKind::DepthExceeded => f.write_str("nesting depth exceeds the limit (128)"),
            ParseErrorKind::Unsupported(feature) => {
                write!(f, "{} is not supported in kabosu v0.1", feature.as_str())
            }
        }
    }
}

impl core::error::Error for ParseError {}

/// 構文エラーの種別
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseErrorKind {
    UnterminatedString,
    InvalidEscape,
    /// `\u` / `\U` がスカラー値にならない
    InvalidUnicodeEscape,
    ControlCharInString,
    ExpectedKey,
    ExpectedValue,
    ExpectedEquals,
    ExpectedNewline,
    UnclosedArray,
    UnclosedTableHeader,
    EmptyKey,
    IntegerOutOfRange,
    InvalidInteger,
    /// float / date-time / 進数整数の**形はしているが TOML として不正**なリテラル
    /// （`1e` / `0xGG` / `1979-bad` など）。妥当なリテラルだけが `Unsupported` になる
    InvalidLiteral,
    /// 重複キー（`previous_span` に先行定義）
    DuplicateKey,
    /// テーブルの再定義・キーとテーブルの衝突・dotted key の非テーブル横断
    TableConflict,
    /// 配列・テーブル・dotted key の深さが上限 128 を超えた
    DepthExceeded,
    /// TOML としては正しいが v0.1 では未対応の構文
    Unsupported(UnsupportedFeature),
}

/// v0.1 で未対応の TOML 構文（位置付きで報告し、一般構文エラーと区別する）
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnsupportedFeature {
    Float,
    DateTime,
    /// 16 / 8 / 2 進整数（`0x` / `0o` / `0b`）
    RadixInteger,
    MultilineString,
    InlineTable,
    ArrayOfTables,
}

impl UnsupportedFeature {
    /// 英語の構文名（エラー文言用）
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Float => "float",
            Self::DateTime => "date-time",
            Self::RadixInteger => "hexadecimal/octal/binary integer",
            Self::MultilineString => "multi-line string",
            Self::InlineTable => "inline table",
            Self::ArrayOfTables => "array of tables",
        }
    }
}
