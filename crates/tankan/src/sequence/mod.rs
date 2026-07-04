//! sequenceDiagram の実装（パース → 定規レイアウト → SVG）。
//!
//! sequence 図はグラフレイアウト不要の「定規計算」で決定的に描ける
//! （参加者は横一列、時間は縦一方向）。

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
