//! gantt チャートの実装（グラフレイアウト不要の定規計算）。
//!
//! - 日付は `dateFormat YYYY-MM-DD` のみ対応（日単位。時分は未対応→フォールバック）
//! - **today マーカーは描かない**（tankan は時刻を読まない = 決定的出力の原則。
//!   `todayMarker off` は受理する）
//! - `excludes`（weekends・曜日・日付）は「働き日消化」で期間を伸ばし、
//!   除外日カラムを網掛けする

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
