//! pie チャートの実装（パース → 扇形＋凡例の SVG）

mod model;
mod parser;
mod render;

use crate::Options;
use crate::error::Error;

pub(crate) fn render(source: &str, options: &Options) -> Result<String, Error> {
    let chart = parser::parse(source)?;
    Ok(render::to_svg(&chart, options))
}
