//! 型 → TOML の生成（手書き encode）。
//!
//! `Encode` 実装が内部の値木を組み立て、`normalize` が決定的なバイト列へ
//! シリアライズする。エンコードは最初の `EncodeError` で停止し、部分出力しない。

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;

use crate::model::{KeyPath, KeySegment, Span};

/// エンコード中に組み立てる値木（正規化出力の入力）
#[derive(Debug)]
pub(crate) enum EncValue {
    String(String),
    Integer(i64),
    Boolean(bool),
    Array(Vec<EncValue>),
    Table(EncTable),
}

/// 追加順を保持するテーブル（重複キーは field で検出する）
#[derive(Debug, Default)]
pub(crate) struct EncTable {
    pub(crate) entries: Vec<(String, EncValue)>,
}

impl EncTable {
    fn contains(&self, key: &str) -> bool {
        self.entries.iter().any(|(k, _)| k == key)
    }
}

/// エンコードエラー（最初の 1 件で停止）
#[derive(Debug, Clone)]
pub struct EncodeError {
    kind: EncodeErrorKind,
    path: KeyPath,
}

impl EncodeError {
    fn new(kind: EncodeErrorKind, path: KeyPath) -> Self {
        Self { kind, path }
    }

    pub fn kind(&self) -> &EncodeErrorKind {
        &self.kind
    }

    pub fn key_path(&self) -> &KeyPath {
        &self.path
    }
}

impl core::fmt::Display for EncodeError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match &self.kind {
            EncodeErrorKind::DuplicateKey => write!(f, "duplicate key `{}`", self.path),
            EncodeErrorKind::RootNotTable => f.write_str("root value must be a table"),
            EncodeErrorKind::DepthExceeded => {
                write!(
                    f,
                    "nesting depth exceeds the limit (128) at `{}`",
                    self.path
                )
            }
            EncodeErrorKind::TableInArray => write!(
                f,
                "a table inside an array cannot be encoded (v0.1 has no inline tables) at `{}`",
                self.path
            ),
        }
    }
}

impl core::error::Error for EncodeError {}

/// エンコードエラーの種別
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EncodeErrorKind {
    DuplicateKey,
    RootNotTable,
    DepthExceeded,
    /// 配列の中のテーブル（inline table 非対応の v0.1 では表現できない）
    TableInArray,
}

const MAX_DEPTH: usize = 128;

/// 自分自身を TOML の値として書き込む（手書き実装。derive は提供しない）
pub trait Encode {
    /// 値を 1 つだけ書き込む。何も書かなければキーごと省略される
    fn encode(&self, encoder: &mut Encoder<'_>) -> Result<(), EncodeError>;
}

/// 値 1 つ分の書き込み先
pub struct Encoder<'a> {
    slot: &'a mut Option<EncValue>,
    path: &'a mut KeyPath,
    depth: usize,
}

impl Encoder<'_> {
    pub fn string(&mut self, value: &str) {
        *self.slot = Some(EncValue::String(String::from(value)));
    }

    pub fn integer(&mut self, value: i64) {
        *self.slot = Some(EncValue::Integer(value));
    }

    pub fn boolean(&mut self, value: bool) {
        *self.slot = Some(EncValue::Boolean(value));
    }

    pub fn array(&mut self) -> ArrayEncoder<'_> {
        *self.slot = Some(EncValue::Array(Vec::new()));
        let Some(EncValue::Array(items)) = self.slot.as_mut() else {
            unreachable!("直前に配列を置いた");
        };
        ArrayEncoder {
            items,
            path: self.path,
            depth: self.depth,
        }
    }

    pub fn table(&mut self) -> TableEncoder<'_> {
        *self.slot = Some(EncValue::Table(EncTable::default()));
        let Some(EncValue::Table(table)) = self.slot.as_mut() else {
            unreachable!("直前にテーブルを置いた");
        };
        TableEncoder {
            table,
            path: self.path,
            depth: self.depth,
        }
    }
}

/// テーブルの書き込み。キーは追加順が出力順になる
pub struct TableEncoder<'a> {
    table: &'a mut EncTable,
    path: &'a mut KeyPath,
    depth: usize,
}

impl TableEncoder<'_> {
    pub fn field<T: Encode + ?Sized>(&mut self, key: &str, value: &T) -> Result<(), EncodeError> {
        self.path
            .push(KeySegment::new(String::from(key), Span::point(0)));
        let result = self.field_inner(key, value);
        self.path.pop();
        result
    }

    fn field_inner<T: Encode + ?Sized>(&mut self, key: &str, value: &T) -> Result<(), EncodeError> {
        if self.depth + 1 > MAX_DEPTH {
            return Err(EncodeError::new(
                EncodeErrorKind::DepthExceeded,
                self.path.clone(),
            ));
        }
        if self.table.contains(key) {
            return Err(EncodeError::new(
                EncodeErrorKind::DuplicateKey,
                self.path.clone(),
            ));
        }
        let mut slot: Option<EncValue> = None;
        value.encode(&mut Encoder {
            slot: &mut slot,
            path: self.path,
            depth: self.depth + 1,
        })?;
        if let Some(v) = slot {
            self.table.entries.push((String::from(key), v));
        }
        Ok(())
    }

    /// None ならキーを出力しない（TOML に null が無いことへの対応）
    pub fn optional_field<T: Encode>(
        &mut self,
        key: &str,
        value: &Option<T>,
    ) -> Result<(), EncodeError> {
        match value {
            Some(v) => self.field(key, v),
            None => Ok(()),
        }
    }
}

/// 配列の書き込み
pub struct ArrayEncoder<'a> {
    items: &'a mut Vec<EncValue>,
    path: &'a mut KeyPath,
    depth: usize,
}

impl ArrayEncoder<'_> {
    pub fn element<T: Encode + ?Sized>(&mut self, value: &T) -> Result<(), EncodeError> {
        if self.depth + 1 > MAX_DEPTH {
            return Err(EncodeError::new(
                EncodeErrorKind::DepthExceeded,
                self.path.clone(),
            ));
        }
        let mut slot: Option<EncValue> = None;
        value.encode(&mut Encoder {
            slot: &mut slot,
            path: self.path,
            depth: self.depth + 1,
        })?;
        match slot {
            Some(EncValue::Table(_)) => Err(EncodeError::new(
                EncodeErrorKind::TableInArray,
                self.path.clone(),
            )),
            Some(v) => {
                self.items.push(v);
                Ok(())
            }
            None => Ok(()),
        }
    }
}

/// 値を正規形の TOML 文字列へ変換する（ルートはテーブルであること）
pub(crate) fn to_string_impl<T: Encode + ?Sized>(value: &T) -> Result<String, EncodeError> {
    let mut slot: Option<EncValue> = None;
    let mut path = KeyPath::default();
    value.encode(&mut Encoder {
        slot: &mut slot,
        path: &mut path,
        depth: 0,
    })?;
    match slot {
        Some(EncValue::Table(table)) => Ok(crate::normalize::render(&table)),
        _ => Err(EncodeError::new(
            EncodeErrorKind::RootNotTable,
            KeyPath::default(),
        )),
    }
}

// ---- 標準実装 ----

impl Encode for str {
    fn encode(&self, encoder: &mut Encoder<'_>) -> Result<(), EncodeError> {
        encoder.string(self);
        Ok(())
    }
}

impl Encode for String {
    fn encode(&self, encoder: &mut Encoder<'_>) -> Result<(), EncodeError> {
        encoder.string(self);
        Ok(())
    }
}

impl Encode for bool {
    fn encode(&self, encoder: &mut Encoder<'_>) -> Result<(), EncodeError> {
        encoder.boolean(*self);
        Ok(())
    }
}

impl Encode for i64 {
    fn encode(&self, encoder: &mut Encoder<'_>) -> Result<(), EncodeError> {
        encoder.integer(*self);
        Ok(())
    }
}

macro_rules! encode_int {
    ($($ty:ty),+) => {$(
        impl Encode for $ty {
            fn encode(&self, encoder: &mut Encoder<'_>) -> Result<(), EncodeError> {
                encoder.integer(i64::from(*self));
                Ok(())
            }
        }
    )+};
}
encode_int!(i32, u8, u16, u32);

impl<T: Encode> Encode for [T] {
    fn encode(&self, encoder: &mut Encoder<'_>) -> Result<(), EncodeError> {
        let mut array = encoder.array();
        for item in self {
            array.element(item)?;
        }
        Ok(())
    }
}

impl<T: Encode> Encode for Vec<T> {
    fn encode(&self, encoder: &mut Encoder<'_>) -> Result<(), EncodeError> {
        self.as_slice().encode(encoder)
    }
}

impl<T: Encode> Encode for BTreeMap<String, T> {
    /// キー順（BTreeMap の昇順）で出力する = 決定的
    fn encode(&self, encoder: &mut Encoder<'_>) -> Result<(), EncodeError> {
        let mut table = encoder.table();
        for (key, value) in self {
            table.field(key, value)?;
        }
        Ok(())
    }
}
