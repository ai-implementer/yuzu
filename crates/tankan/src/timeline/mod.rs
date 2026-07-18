//! timeline 図（mermaid 互換）。
//!
//! 期間を等間隔カラムで横に並べ、各期間の下にイベント箱を縦積みする。
//! セクションはカラム範囲を覆う色帯。gantt と違い時間スケールは持たない
//! （期間ラベルは文字列として扱う = 日付演算なし・時刻非依存）。

mod layout;
mod model;
mod parser;
mod render;

use crate::Options;
use crate::error::Error;

pub(crate) fn render(source: &str, options: &Options) -> Result<String, Error> {
    let diagram = parser::parse(source)?;
    let layout = layout::layout(&diagram, options);
    Ok(render::to_svg(&layout, options))
}
