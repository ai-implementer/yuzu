//! パース結果のデータモデル。
//!
//! `Document` が原文を所有し、木構造（`Table` → `Entry` → `Node` → `Value`）は
//! すべて原文へのバイト範囲 `Span` を持つ。内部フィールドは非公開で、
//! v0.1 では読み取り用アクセサーだけを公開する（非破壊編集は将来設計）。

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;

use crate::datetime::Datetime;

/// 0 始まり・終端を含まない UTF-8 バイト範囲
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    pub(crate) fn point(offset: usize) -> Self {
        Self {
            start: offset,
            end: offset,
        }
    }
}

/// 表示用の位置。1 始まり・Unicode スカラー値単位
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LineCol {
    pub line: usize,
    pub col: usize,
}

/// キー経路の 1 セグメント（名前と原文上の位置）
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeySegment {
    name: String,
    span: Span,
}

impl KeySegment {
    /// 独自診断のキー経路組み立てにも使う公開コンストラクタ
    pub fn new(name: String, span: Span) -> Self {
        Self { name, span }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn span(&self) -> Span {
        self.span
    }
}

/// キー経路。文字列へ平坦化せず、各セグメントと span を持つ
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct KeyPath {
    segments: Vec<KeySegment>,
}

impl KeyPath {
    pub fn segments(&self) -> &[KeySegment] {
        &self.segments
    }

    pub fn push(&mut self, segment: KeySegment) {
        self.segments.push(segment);
    }

    pub(crate) fn pop(&mut self) {
        self.segments.pop();
    }
}

impl core::fmt::Display for KeyPath {
    /// `a.b.c` 形式。bare key で書けないセグメントは basic string で引用する
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        for (i, seg) in self.segments.iter().enumerate() {
            if i > 0 {
                f.write_str(".")?;
            }
            if crate::normalize::is_bare_key(seg.name()) {
                f.write_str(seg.name())?;
            } else {
                f.write_str(&crate::normalize::quote_string(seg.name()))?;
            }
        }
        Ok(())
    }
}

/// TOML の値
#[non_exhaustive]
#[derive(Debug)]
pub enum Value {
    String(String),
    /// 10 進・16 / 8 / 2 進のどれで書かれても i64（表記は保持しない）
    Integer(i64),
    /// `inf` / `nan` を含む。`nan` の符号は保持しない
    Float(f64),
    Boolean(bool),
    /// 日付・時刻。offset date-time / local date-time / local date / local time を
    /// 1 型で表す（区別は [`Datetime::kind`]）
    Datetime(Datetime),
    Array(Vec<Node>),
    Table(Table),
}

/// 診断メッセージ用の値種別（`Value` と 1:1）
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueKind {
    String,
    Integer,
    Float,
    Boolean,
    Datetime,
    Array,
    Table,
}

impl ValueKind {
    /// 英語の種別名（診断文言用）
    pub fn as_str(self) -> &'static str {
        match self {
            Self::String => "string",
            Self::Integer => "integer",
            Self::Float => "float",
            Self::Boolean => "boolean",
            Self::Datetime => "datetime",
            Self::Array => "array",
            Self::Table => "table",
        }
    }
}

/// 値と原文上の位置
#[derive(Debug)]
pub struct Node {
    value: Value,
    span: Span,
}

impl Node {
    pub(crate) fn new(value: Value, span: Span) -> Self {
        Self { value, span }
    }

    pub fn value(&self) -> &Value {
        &self.value
    }

    pub(crate) fn value_mut(&mut self) -> &mut Value {
        &mut self.value
    }

    pub fn span(&self) -> Span {
        self.span
    }

    pub fn kind(&self) -> ValueKind {
        match &self.value {
            Value::String(_) => ValueKind::String,
            Value::Integer(_) => ValueKind::Integer,
            Value::Float(_) => ValueKind::Float,
            Value::Boolean(_) => ValueKind::Boolean,
            Value::Datetime(_) => ValueKind::Datetime,
            Value::Array(_) => ValueKind::Array,
            Value::Table(_) => ValueKind::Table,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match &self.value {
            Value::String(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_integer(&self) -> Option<i64> {
        match &self.value {
            Value::Integer(n) => Some(*n),
            _ => None,
        }
    }

    /// float だけを返す（整数リテラルは `None`。TOML は integer と float を区別する）
    pub fn as_float(&self) -> Option<f64> {
        match &self.value {
            Value::Float(f) => Some(*f),
            _ => None,
        }
    }

    pub fn as_boolean(&self) -> Option<bool> {
        match &self.value {
            Value::Boolean(b) => Some(*b),
            _ => None,
        }
    }

    /// 日付・時刻。4 種の区別は [`Datetime::kind`] で見る
    pub fn as_datetime(&self) -> Option<Datetime> {
        match &self.value {
            Value::Datetime(dt) => Some(*dt),
            _ => None,
        }
    }

    pub fn as_array(&self) -> Option<&[Node]> {
        match &self.value {
            Value::Array(items) => Some(items),
            _ => None,
        }
    }

    pub fn as_table(&self) -> Option<&Table> {
        match &self.value {
            Value::Table(t) => Some(t),
            _ => None,
        }
    }
}

/// テーブルの生成元（重複・競合検査用の内部状態）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TableOrigin {
    /// ルートテーブル
    Root,
    /// `[a.b]` ヘッダで明示定義された
    Header,
    /// ヘッダ経路の中間として暗黙に作られた（後から `[a]` で定義できる）
    HeaderImplicit,
    /// dotted key（`a.b = 1`）の中間として作られた（ヘッダでの再定義は不可）
    Dotted,
    /// インラインテーブル `{ ... }`（閉じている = 後から拡張できない）
    Inline,
    /// `[[a]]` が作った配列の要素（ヘッダ経路は最後の要素へ降りる）
    ArrayHeader,
}

/// キーと値のペア。入力順を保持する
#[derive(Debug)]
pub struct Entry {
    key: KeySegment,
    node: Node,
}

impl Entry {
    pub fn key(&self) -> &str {
        self.key.name()
    }

    pub fn key_span(&self) -> Span {
        self.key.span()
    }

    pub(crate) fn key_segment(&self) -> &KeySegment {
        &self.key
    }

    pub fn node(&self) -> &Node {
        &self.node
    }

    pub(crate) fn node_mut(&mut self) -> &mut Node {
        &mut self.node
    }
}

/// テーブル。エントリは入力順を保持し、検索用の内部索引は公開しない
#[derive(Debug)]
pub struct Table {
    entries: Vec<Entry>,
    index: BTreeMap<String, usize>,
    origin: TableOrigin,
    /// テーブル末尾（次のヘッダ直前 or EOF）の長さ 0 span。
    /// 必須キー欠落診断の置き場（kabosu.md「診断」参照）
    end_span: Span,
}

impl Table {
    pub(crate) fn new(origin: TableOrigin) -> Self {
        Self {
            entries: Vec::new(),
            index: BTreeMap::new(),
            origin,
            end_span: Span::point(0),
        }
    }

    /// エントリを入力順で列挙する
    pub fn entries(&self) -> impl Iterator<Item = &Entry> {
        self.entries.iter()
    }

    pub fn get(&self, key: &str) -> Option<&Entry> {
        self.index.get(key).map(|&i| &self.entries[i])
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// テーブル末尾の長さ 0 span（必須キー欠落診断の位置）
    pub fn end_span(&self) -> Span {
        self.end_span
    }

    pub(crate) fn origin(&self) -> TableOrigin {
        self.origin
    }

    pub(crate) fn set_origin(&mut self, origin: TableOrigin) {
        self.origin = origin;
    }

    pub(crate) fn set_end_span(&mut self, span: Span) {
        self.end_span = span;
    }

    pub(crate) fn get_mut(&mut self, key: &str) -> Option<&mut Entry> {
        let i = *self.index.get(key)?;
        Some(&mut self.entries[i])
    }

    /// 存在しないことを呼び出し側が保証して挿入する
    pub(crate) fn insert(&mut self, key: KeySegment, node: Node) {
        let name = String::from(key.name());
        self.index.insert(name, self.entries.len());
        self.entries.push(Entry { key, node });
    }
}

/// コメント（原文スライスと span）
#[derive(Debug, Clone, Copy)]
pub struct Comment<'a> {
    text: &'a str,
    span: Span,
}

impl Comment<'_> {
    /// `#` を含むコメント全文（行末の改行は含まない）
    pub fn text(&self) -> &str {
        self.text
    }

    pub fn span(&self) -> Span {
        self.span
    }
}

/// パース済みドキュメント。原文を所有する
#[derive(Debug)]
pub struct Document {
    src: String,
    /// 常に `Value::Table`（ルートテーブル）
    root: Node,
    comments: Vec<Span>,
}

impl Document {
    /// 原文をコピーしてパースする
    pub fn parse(src: &str) -> Result<Document, crate::ParseError> {
        Self::parse_owned(String::from(src))
    }

    /// 原文の所有権を受け取ってパースする
    pub fn parse_owned(src: String) -> Result<Document, crate::ParseError> {
        let (root, comments) = crate::parser::parse(&src)?;
        Ok(Document {
            src,
            root,
            comments,
        })
    }

    pub fn source(&self) -> &str {
        &self.src
    }

    pub fn root(&self) -> &Table {
        match self.root.value() {
            Value::Table(t) => t,
            _ => unreachable!("ルートは常にテーブル"),
        }
    }

    pub(crate) fn root_node(&self) -> &Node {
        &self.root
    }

    pub fn comments(&self) -> impl Iterator<Item = Comment<'_>> {
        self.comments.iter().map(|&span| Comment {
            text: &self.src[span.start..span.end],
            span,
        })
    }

    /// バイトオフセット → 表示用の 1 始まり (行, 列)。列は Unicode スカラー値単位。
    /// 保存せず表示時に算出する（設定ファイルは小さく、診断表示時にしか呼ばれない）
    pub fn line_col(&self, offset: usize) -> LineCol {
        line_col_of(&self.src, offset)
    }
}

/// `Document::line_col` の実体（パースエラー表示用に原文だけでも使えるよう分離）
pub fn line_col_of(src: &str, offset: usize) -> LineCol {
    let mut line = 1;
    let mut col = 1;
    for (i, c) in src.char_indices() {
        if i >= offset {
            break;
        }
        if c == '\n' {
            line += 1;
            col = 1;
        } else {
            col += 1;
        }
    }
    LineCol { line, col }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_col_は_1_始まりで日本語をスカラー単位で数える() {
        let src = "ab\nあい=1\n";
        // "あ" の直後（"い" の位置）: 2 行目・2 文字目
        let offset = 3 + "あ".len();
        assert_eq!(line_col_of(src, offset), LineCol { line: 2, col: 2 });
        assert_eq!(line_col_of(src, 0), LineCol { line: 1, col: 1 });
        // 範囲外は末尾扱い
        assert_eq!(line_col_of("a", 100).line, 1);
    }

    #[test]
    fn keypath_の表示は引用が必要なセグメントだけ引用する() {
        let mut p = KeyPath::default();
        p.push(KeySegment::new("lint".into(), Span::point(0)));
        p.push(KeySegment::new("サーバー".into(), Span::point(0)));
        assert_eq!(alloc::format!("{p}"), "lint.\"サーバー\"");
    }
}
