//! classDiagram の AST

use crate::common::style::Style;

/// クラスボックス（3 区画: 名前＋アノテーション / 属性 / メソッド）
#[derive(Debug, Default)]
pub(crate) struct Class {
    /// 表示名（ジェネリクス `~T~` は `<T>` へ変換済み）
    pub display: String,
    /// アノテーション（`<<interface>>` の中身。ある場合のみ）
    pub annotation: Option<String>,
    /// 属性行（可視性接頭辞込みの表示テキスト）
    pub attributes: Vec<String>,
    /// メソッド行（`(` を含む行）
    pub methods: Vec<String>,
    /// 解決済みインラインスタイル（classDef / cssClass / `:::` / style。無ければ None）
    pub style: Option<Style>,
}

/// 関係線の端に付くマーカー形状。左右どちらの端に付くかで意味が決まる
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Marker {
    /// マーカーなし（`--` / `..` のリンク端）
    None,
    /// `<|` / `|>` 継承・実現（白抜き三角）
    Triangle,
    /// `*` コンポジション（塗り菱形）
    DiamondFilled,
    /// `o` 集約（白抜き菱形）
    DiamondHollow,
    /// `<` / `>` 関連・依存（開き矢印）
    Arrow,
}

#[derive(Debug)]
pub(crate) struct Relation {
    pub from: usize,
    pub to: usize,
    /// from 側（左端）のマーカー
    pub from_marker: Marker,
    /// to 側（右端）のマーカー
    pub to_marker: Marker,
    /// `..` = 破線 / `--` = 実線
    pub dashed: bool,
    pub label: Vec<String>,
    /// 多重度（from 側 / to 側。`"1"` `"*"` 等）
    pub from_card: Option<String>,
    pub to_card: Option<String>,
}

#[derive(Debug, Default)]
pub(crate) struct ClassDiagram {
    pub title: Option<Vec<String>>,
    pub classes: Vec<Class>,
    pub relations: Vec<Relation>,
}
