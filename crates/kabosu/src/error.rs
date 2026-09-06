//! パースエラー。
//!
//! 構文エラーは最初の 1 件で停止する（kabosu.md「型変換と診断」）。
//! エラー文は英語（利用側で `kind` から翻訳できる）。

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
            ParseErrorKind::ControlCharInComment => {
                f.write_str("control character is not allowed in a comment")
            }
            ParseErrorKind::TooManyQuotes => f.write_str(
                "three or more adjacent quotation marks inside a multi-line string must be escaped",
            ),
            ParseErrorKind::MultilineStringAsKey => {
                f.write_str("a multi-line string cannot be used as a key")
            }
            ParseErrorKind::ExpectedKey => f.write_str("expected a key"),
            ParseErrorKind::ExpectedValue => f.write_str("expected a value"),
            ParseErrorKind::ExpectedEquals => f.write_str("expected `=` after the key"),
            ParseErrorKind::ExpectedNewline => f.write_str("expected a newline"),
            ParseErrorKind::UnclosedArray => f.write_str("unclosed array (missing `]`)"),
            ParseErrorKind::UnclosedInlineTable => f.write_str(
                "unclosed inline table (missing `}`; TOML 1.0 allows neither newlines nor a trailing comma inside `{ }`)",
            ),
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
            ParseErrorKind::Unsupported(feature) => write!(
                f,
                "{} is TOML 1.1 syntax (kabosu parses TOML 1.0)",
                feature.as_str()
            ),
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
    /// コメントの中のタブ以外の制御文字
    ControlCharInComment,
    /// 複数行文字列の中に閉じ区切り文字が 3 つ以上連続した（`""""""` 等。
    /// 閉じ区切りの直前に置ける引用符は 2 つまで）
    TooManyQuotes,
    /// キー位置に複数行文字列（`"""` / `'''`）が書かれた
    MultilineStringAsKey,
    ExpectedKey,
    ExpectedValue,
    ExpectedEquals,
    ExpectedNewline,
    UnclosedArray,
    /// インラインテーブルが閉じていない（`}` が無い・改行が入った・末尾カンマ）
    UnclosedInlineTable,
    UnclosedTableHeader,
    EmptyKey,
    IntegerOutOfRange,
    InvalidInteger,
    /// float / date-time / 進数整数の**形はしているが TOML として不正**なリテラル
    /// （`1e` / `0xGG` / `1979-bad` など）
    InvalidLiteral,
    /// 重複キー（`previous_span` に先行定義）
    DuplicateKey,
    /// テーブルの再定義・キーとテーブルの衝突・dotted key の非テーブル横断
    TableConflict,
    /// 配列・テーブル・dotted key の深さが上限 128 を超えた
    DepthExceeded,
    /// TOML 1.1 で追加された構文（kabosu は TOML 1.0）。
    /// 一般的な構文エラーと区別して「1.0 には無い記法」と案内できるようにする
    Unsupported(TomlV11),
}

/// TOML 1.1 で追加された構文。kabosu は TOML 1.0 のパーサなので受理しないが、
/// 「書き間違い」ではなく「新しい版の記法」だと位置付きで伝える。
///
/// **variant を足すときは「1.0 では invalid・1.1 では valid」であることを
/// 公式 toml-test で裏付けること**（`files-toml-1.0.0` の invalid 一覧にあり、
/// `files-toml-1.1.0` の invalid 一覧に無いケース）。引用符なしの非 ASCII キーは
/// 1.1 でも許されないため、ここには入らない
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TomlV11 {
    /// `\e`（ESC）と `\xHH` のエスケープ。
    /// toml-test: `invalid/string/basic-byte-escapes`
    Escape,
    /// インラインテーブルの中の改行・コメント・末尾カンマ。
    /// toml-test: `invalid/inline-table/linebreak-01..04`・`trailing-comma`
    InlineTable,
    /// 秒を省略した時刻（`07:32`）。
    /// toml-test: `invalid/{datetime,local-datetime,local-time}/no-secs`
    TimeWithoutSeconds,
}

impl TomlV11 {
    /// 英語の構文名（エラー文言用）
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Escape => "the `\\e` and `\\xHH` escapes",
            Self::InlineTable => "newlines, comments and trailing commas in inline tables",
            Self::TimeWithoutSeconds => "a time without seconds",
        }
    }
}
