//! stateDiagram-v2 の実装。
//!
//! パース結果を flowchart のモデル（状態 = 角丸ノード、composite = クラスタ、
//! concurrency 領域 = region クラスタ、[*] = StateStart/StateEnd、
//! fork/join = ForkBar、note = NoteBox＋点線エッジ）へ**変換**し、
//! layered レイアウトと SVG レンダラを完全共用する。

mod parser;

use crate::Options;
use crate::error::Error;
use crate::flowchart::{layout as fc_layout, render as fc_render};

pub(crate) fn render(source: &str, options: &Options) -> Result<String, Error> {
    let diagram = parser::parse(source)?;
    let layout = fc_layout::layout(&diagram, options);
    Ok(fc_render::to_svg(
        &layout,
        options,
        "tankan-state",
        "State diagram",
    ))
}
