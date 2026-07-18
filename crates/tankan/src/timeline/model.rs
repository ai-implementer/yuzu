//! timeline の AST

#[derive(Debug, Default)]
pub(crate) struct TimelineDiagram {
    pub title: Option<String>,
    pub sections: Vec<Section>,
}

#[derive(Debug)]
pub(crate) struct Section {
    /// None = `section` 文なしの暗黙セクション（帯を描かない）
    pub name: Option<String>,
    pub periods: Vec<Period>,
}

#[derive(Debug)]
pub(crate) struct Period {
    /// 期間ラベル（`2004` / `2023-10` 等。文字列のまま = 日付演算しない）
    pub label: String,
    pub events: Vec<String>,
}
