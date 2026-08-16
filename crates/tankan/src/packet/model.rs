//! packet の AST

#[derive(Debug, Default)]
pub(crate) struct PacketDiagram {
    pub title: Option<String>,
    /// ビット 0 から連続・重複なしがパース時に保証済み（start 昇順）
    pub fields: Vec<Field>,
    /// frontmatter の `config.packet`（未指定は mermaid 既定値）
    pub config: PacketConfig,
}

#[derive(Debug)]
pub(crate) struct Field {
    /// 開始ビット（0 始まり・inclusive）
    pub start: u32,
    /// 終了ビット（inclusive。start <= end 保証済み）
    pub end: u32,
    /// 表示ラベル（引用符剥がし済み。空文字可）
    pub label: String,
}

/// mermaid の packet 既定 config と同値
#[derive(Debug, Clone)]
pub(crate) struct PacketConfig {
    pub bits_per_row: u32,
    pub bit_width: f32,
    pub row_height: f32,
    pub show_bits: bool,
    pub padding_x: f32,
    pub padding_y: f32,
}

impl Default for PacketConfig {
    fn default() -> Self {
        Self {
            bits_per_row: 32,
            bit_width: 32.0,
            row_height: 32.0,
            show_bits: true,
            padding_x: 5.0,
            padding_y: 5.0,
        }
    }
}
