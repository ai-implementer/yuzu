//! pie チャートのモデル

/// パース済みの pie チャート
#[derive(Debug, Default)]
pub(crate) struct PieChart {
    pub title: Option<String>,
    /// 凡例に数値も表示する（`pie showData`）
    pub show_data: bool,
    pub slices: Vec<Slice>,
}

#[derive(Debug)]
pub(crate) struct Slice {
    pub label: String,
    pub value: f32,
    /// 入力の数値文字列そのまま（showData の凡例表示用。float 整形の誤差を避ける）
    pub raw_value: String,
}

impl PieChart {
    pub fn total(&self) -> f32 {
        self.slices.iter().map(|s| s.value).sum()
    }
}
