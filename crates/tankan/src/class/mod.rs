//! classDiagram の実装（パース → layered レイアウト → 3 区画ボックス＋関係マーカー）

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
