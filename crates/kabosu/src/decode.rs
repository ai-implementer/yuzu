//! 型変換（手書き decode）と診断。
//!
//! - 構文エラーはパースで停止済み。ここでは診断を**可能な限り蓄積**する
//! - エラーが 1 件でもあれば値を返さず（`DecodeReport::value` = None）、
//!   警告だけなら値を返す
//! - 未知キーは Warn（既定）/ Deny / Ignore の 3 方針
//! - 診断は主 span の開始位置で安定ソートし、既定 100 件で打ち切って
//!   省略件数を最後に示す
//! - 組み込みのエラー文は英語。利用側は `DiagnosticCode` / `KeyPath` / `Span` から
//!   独自の文言に翻訳できる

use alloc::borrow::Cow;
use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use crate::model::{Document, Entry, KeyPath, KeySegment, Node, Span, Table, Value, ValueKind};

/// 未知キーの扱い
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum UnknownKeys {
    /// 警告にする（既定）
    #[default]
    Warn,
    /// エラーにする
    Deny,
    /// 無視する
    Ignore,
}

/// decode の動作オプション
#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct DecodeOptions {
    pub unknown_keys: UnknownKeys,
    /// 診断の蓄積上限（超過分は省略件数として最後に 1 件で示す）
    pub max_diagnostics: usize,
}

impl Default for DecodeOptions {
    fn default() -> Self {
        Self {
            unknown_keys: UnknownKeys::Warn,
            max_diagnostics: 100,
        }
    }
}

/// 診断の重大度
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Warning,
    Error,
}

/// 診断の構造化された種別
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiagnosticCode {
    TypeMismatch {
        expected: ValueKind,
        found: ValueKind,
    },
    IntegerOutOfRange,
    MissingKey,
    UnknownKey {
        /// その階層で受理されるキー（`TableDecoder` への要求順）。
        /// 利用側が独自の文言（翻訳・候補提示）を組み立てられるよう構造化して持つ
        known_keys: Vec<String>,
    },
    /// 上限超過で省略した件数（末尾に 1 件だけ置かれる）
    TooManyDiagnostics {
        omitted: usize,
    },
    /// 利用側の独自コード
    Custom(Cow<'static, str>),
}

/// 1 件の診断（種別・重大度・英語メッセージ・キー経路・span）
#[derive(Debug, Clone)]
pub struct Diagnostic {
    code: DiagnosticCode,
    severity: Severity,
    message: String,
    key_path: KeyPath,
    span: Span,
}

impl Diagnostic {
    /// decode 実装から独自の診断を作る
    pub fn new(
        code: DiagnosticCode,
        severity: Severity,
        message: String,
        key_path: KeyPath,
        span: Span,
    ) -> Self {
        Self {
            code,
            severity,
            message,
            key_path,
            span,
        }
    }

    pub fn code(&self) -> &DiagnosticCode {
        &self.code
    }

    pub fn severity(&self) -> Severity {
        self.severity
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub fn key_path(&self) -> &KeyPath {
        &self.key_path
    }

    pub fn span(&self) -> Span {
        self.span
    }
}

/// decode の結果（値と診断）
#[derive(Debug)]
pub struct DecodeReport<T> {
    value: Option<T>,
    diagnostics: Vec<Diagnostic>,
}

impl<T> DecodeReport<T> {
    /// エラーが 1 件でもあれば None
    pub fn value(&self) -> Option<&T> {
        self.value.as_ref()
    }

    /// 主 span の開始位置で安定ソート済み
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    pub fn has_errors(&self) -> bool {
        self.diagnostics
            .iter()
            .any(|d| d.severity == Severity::Error)
    }

    pub fn into_parts(self) -> (Option<T>, Vec<Diagnostic>) {
        (self.value, self.diagnostics)
    }
}

/// decode の実行状態（診断 sink・現在のキー経路）
pub struct DecodeContext<'a> {
    doc: &'a Document,
    options: DecodeOptions,
    path: KeyPath,
    diagnostics: Vec<Diagnostic>,
    omitted: usize,
    has_error: bool,
}

impl<'a> DecodeContext<'a> {
    pub(crate) fn new(doc: &'a Document, options: DecodeOptions) -> Self {
        Self {
            doc,
            options,
            path: KeyPath::default(),
            diagnostics: Vec::new(),
            omitted: 0,
            has_error: false,
        }
    }

    /// 診断を追加する（decode 実装からの独自診断もここを通す）
    pub fn diagnostic(&mut self, diagnostic: Diagnostic) {
        if diagnostic.severity == Severity::Error {
            self.has_error = true;
        }
        if self.diagnostics.len() >= self.options.max_diagnostics {
            self.omitted += 1;
        } else {
            self.diagnostics.push(diagnostic);
        }
    }

    /// 型不一致の Error 診断を積む（標準実装と独自実装の共通ヘルパ）
    pub fn type_mismatch(&mut self, expected: ValueKind, node: &Node) {
        let found = node.kind();
        let message = format!("expected {}, found {}", expected.as_str(), found.as_str());
        let d = Diagnostic::new(
            DiagnosticCode::TypeMismatch { expected, found },
            Severity::Error,
            message,
            self.path.clone(),
            node.span(),
        );
        self.diagnostic(d);
    }

    /// 現在のキー経路
    pub fn key_path(&self) -> &KeyPath {
        &self.path
    }

    /// span → line/col 変換などに使う
    pub fn document(&self) -> &Document {
        self.doc
    }

    pub(crate) fn push_segment(&mut self, segment: KeySegment) {
        self.path.push(segment);
    }

    pub(crate) fn pop_segment(&mut self) {
        self.path.pop();
    }

    pub(crate) fn finish_report<T>(mut self, value: Option<T>) -> DecodeReport<T> {
        // 安定ソート（同一位置は挿入順を保つ）→ 省略通知は末尾
        self.diagnostics.sort_by_key(|d| d.span.start);
        if self.omitted > 0 {
            let omitted = self.omitted;
            self.diagnostics.push(Diagnostic::new(
                DiagnosticCode::TooManyDiagnostics { omitted },
                Severity::Warning,
                format!("{omitted} more diagnostics omitted"),
                KeyPath::default(),
                Span::point(self.doc.source().len()),
            ));
        }
        let value = if self.has_error { None } else { value };
        DecodeReport {
            value,
            diagnostics: self.diagnostics,
        }
    }
}

/// TOML の値から Self を組み立てる（手書き実装。derive は提供しない）
pub trait Decode: Sized {
    /// 失敗時は `cx` へ Error 診断を積んで None を返す
    /// （兄弟キーの decode は続行され、診断が蓄積される）
    fn decode(node: &Node, cx: &mut DecodeContext<'_>) -> Option<Self>;
}

/// テーブルをキー単位で読み取るデコーダ。
/// `finish()` が未消費キーへ未知キー方針を適用する。
/// テーブル借用（`'t`）と診断 sink の借用（`'c`）は独立で、
/// `raw` で取ったエントリは `finish()` 後も使える（独自診断の組み立て用）
pub struct TableDecoder<'t, 'c, 'doc> {
    table: &'t Table,
    cx: &'c mut DecodeContext<'doc>,
    /// required / optional / raw で要求されたキー（未知キー診断の対象外＋既知キー一覧）
    known: Vec<String>,
}

impl<'t, 'c, 'doc> TableDecoder<'t, 'c, 'doc> {
    /// node がテーブルでなければ TypeMismatch を積んで None
    pub fn new(node: &'t Node, cx: &'c mut DecodeContext<'doc>) -> Option<Self> {
        match node.value() {
            Value::Table(table) => Some(Self {
                table,
                cx,
                known: Vec::new(),
            }),
            _ => {
                cx.type_mismatch(ValueKind::Table, node);
                None
            }
        }
    }

    /// 必須キー。欠落は所属テーブル末尾の長さ 0 span で MissingKey（Error）
    pub fn required<T: Decode>(&mut self, key: &str) -> Option<T> {
        self.known.push(String::from(key));
        let table: &'t Table = self.table;
        match table.get(key) {
            Some(entry) => self.decode_entry(entry),
            None => {
                let span = table.end_span();
                let mut path = self.cx.path.clone();
                path.push(KeySegment::new(String::from(key), span));
                let d = Diagnostic::new(
                    DiagnosticCode::MissingKey,
                    Severity::Error,
                    format!("missing required key `{key}`"),
                    path,
                    span,
                );
                self.cx.diagnostic(d);
                None
            }
        }
    }

    /// 任意キー。欠落は診断なしで None（呼び出し側が既定値で埋める）。
    /// 型不一致は診断を積んで None
    pub fn optional<T: Decode>(&mut self, key: &str) -> Option<T> {
        self.known.push(String::from(key));
        let table: &'t Table = self.table;
        let entry = table.get(key)?;
        self.decode_entry(entry)
    }

    /// 生ノードを取り出して独自 decode する（span 付き検証用。キーは消費済み扱い）
    pub fn raw(&mut self, key: &str) -> Option<&'t Entry> {
        self.known.push(String::from(key));
        let table: &'t Table = self.table;
        table.get(key)
    }

    fn decode_entry<T: Decode>(&mut self, entry: &Entry) -> Option<T> {
        self.cx.push_segment(entry.key_segment().clone());
        let value = T::decode(entry.node(), self.cx);
        self.cx.pop_segment();
        value
    }

    /// 未消費キーへ未知キー方針（Warn / Deny / Ignore）を適用して終了する
    pub fn finish(self) {
        let severity = match self.cx.options.unknown_keys {
            UnknownKeys::Ignore => return,
            UnknownKeys::Warn => Severity::Warning,
            UnknownKeys::Deny => Severity::Error,
        };
        for entry in self.table.entries() {
            if self.known.iter().any(|k| k == entry.key()) {
                continue;
            }
            let message = if self.known.is_empty() {
                format!("unknown key `{}`", entry.key())
            } else {
                format!(
                    "unknown key `{}` (known keys: {})",
                    entry.key(),
                    self.known.join(", ")
                )
            };
            let mut path = self.cx.path.clone();
            path.push(entry.key_segment().clone());
            let d = Diagnostic::new(
                DiagnosticCode::UnknownKey {
                    known_keys: self.known.clone(),
                },
                severity,
                message,
                path,
                entry.key_span(),
            );
            self.cx.diagnostic(d);
        }
    }
}

/// ドキュメント全体を T へ decode する（`kabosu::decode` の実体）
pub(crate) fn run<T: Decode>(doc: &Document, options: DecodeOptions) -> DecodeReport<T> {
    let mut cx = DecodeContext::new(doc, options);
    let value = T::decode(doc.root_node(), &mut cx);
    cx.finish_report(value)
}

// ---- 標準実装 ----

impl Decode for String {
    fn decode(node: &Node, cx: &mut DecodeContext<'_>) -> Option<Self> {
        match node.value() {
            Value::String(s) => Some(s.clone()),
            _ => {
                cx.type_mismatch(ValueKind::String, node);
                None
            }
        }
    }
}

impl Decode for bool {
    fn decode(node: &Node, cx: &mut DecodeContext<'_>) -> Option<Self> {
        match node.value() {
            Value::Boolean(b) => Some(*b),
            _ => {
                cx.type_mismatch(ValueKind::Boolean, node);
                None
            }
        }
    }
}

impl Decode for i64 {
    fn decode(node: &Node, cx: &mut DecodeContext<'_>) -> Option<Self> {
        match node.value() {
            Value::Integer(n) => Some(*n),
            _ => {
                cx.type_mismatch(ValueKind::Integer, node);
                None
            }
        }
    }
}

/// i64 から狭い整数型への変換（範囲外は IntegerOutOfRange の Error 診断）
macro_rules! decode_int {
    ($($ty:ty),+) => {$(
        impl Decode for $ty {
            fn decode(node: &Node, cx: &mut DecodeContext<'_>) -> Option<Self> {
                let n = i64::decode(node, cx)?;
                match <$ty>::try_from(n) {
                    Ok(v) => Some(v),
                    Err(_) => {
                        let d = Diagnostic::new(
                            DiagnosticCode::IntegerOutOfRange,
                            Severity::Error,
                            format!(
                                "integer {n} is out of range ({}..={})",
                                <$ty>::MIN,
                                <$ty>::MAX
                            ),
                            cx.path.clone(),
                            node.span(),
                        );
                        cx.diagnostic(d);
                        None
                    }
                }
            }
        }
    )+};
}
decode_int!(i32, u8, u16, u32);

impl<T: Decode> Decode for Option<T> {
    /// 透過（キー欠落は `TableDecoder::optional` 側が担う）
    fn decode(node: &Node, cx: &mut DecodeContext<'_>) -> Option<Self> {
        T::decode(node, cx).map(Some)
    }
}

impl<T: Decode> Decode for Vec<T> {
    fn decode(node: &Node, cx: &mut DecodeContext<'_>) -> Option<Self> {
        let Value::Array(items) = node.value() else {
            cx.type_mismatch(ValueKind::Array, node);
            return None;
        };
        // 診断蓄積のため、失敗した要素があっても全要素を decode する
        let mut out = Vec::with_capacity(items.len());
        let mut ok = true;
        for (i, item) in items.iter().enumerate() {
            cx.push_segment(KeySegment::new(format!("{i}"), item.span()));
            match T::decode(item, cx) {
                Some(v) => out.push(v),
                None => ok = false,
            }
            cx.pop_segment();
        }
        ok.then_some(out)
    }
}

impl<T: Decode> Decode for BTreeMap<String, T> {
    /// テーブル全体を自由キーとして読む（未知キー検査の対象外）
    fn decode(node: &Node, cx: &mut DecodeContext<'_>) -> Option<Self> {
        let Value::Table(table) = node.value() else {
            cx.type_mismatch(ValueKind::Table, node);
            return None;
        };
        let mut out = BTreeMap::new();
        let mut ok = true;
        for entry in table.entries() {
            cx.push_segment(entry.key_segment().clone());
            match T::decode(entry.node(), cx) {
                Some(v) => {
                    out.insert(String::from(entry.key()), v);
                }
                None => ok = false,
            }
            cx.pop_segment();
        }
        ok.then_some(out)
    }
}
