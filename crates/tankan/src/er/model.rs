//! erDiagram の AST

use crate::common::style::Style;

#[derive(Debug)]
pub(crate) struct Entity {
    /// 表示名（エイリアス `E[表示名]` があればそちら）
    pub display: String,
    pub attributes: Vec<Attribute>,
    /// 解決済みインラインスタイル（classDef / class / `:::` / style。無ければ None）
    pub style: Option<Style>,
}

#[derive(Debug)]
pub(crate) struct Attribute {
    pub type_name: String,
    pub name: String,
    /// PK / FK / UK
    pub keys: Vec<String>,
    pub comment: Option<String>,
}

/// クロウズフットの基数
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Cardinality {
    /// `|o` / `o|` — 0 または 1
    ZeroOne,
    /// `||` — ちょうど 1
    One,
    /// `}o` / `o{` — 0 以上
    ZeroMany,
    /// `}|` / `|{` — 1 以上
    OneMany,
}

#[derive(Debug)]
pub(crate) struct Relation {
    pub from: usize,
    pub to: usize,
    pub from_card: Cardinality,
    pub to_card: Cardinality,
    /// `--` = 識別（実線）/ `..` = 非識別（破線）
    pub identifying: bool,
    pub label: Vec<String>,
}

#[derive(Debug, Default)]
pub(crate) struct ErDiagram {
    pub title: Option<Vec<String>>,
    pub entities: Vec<Entity>,
    pub relations: Vec<Relation>,
}
