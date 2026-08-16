//! packet 図（mermaid 互換）。
//!
//! ビット列のフィールドを bitsPerRow（既定 32）ごとの行に並べるパケット構造図。
//! frontmatter の `config.packet`（bitsPerRow / bitWidth / rowHeight / showBits /
//! paddingX / paddingY）を最小パースして反映する（tankan で config を読む唯一の図種。
//! インライン形式 `config: {...}` は非対応 = 既定値で描く）。

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
