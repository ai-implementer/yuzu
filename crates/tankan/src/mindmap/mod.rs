//! mindmap 図（mermaid 互換）。
//!
//! インデント階層のツリーを、中央ルートから左右へ振り分けた tidy tree で描く
//! （トップレベルの子を偶数 = 右・奇数 = 左に交互割当）。mermaid.js の放射状
//! 配置とは異なるが、決定的で読みやすいマインドマップ表現を優先する。
//! ブランチ（ルート直下の子とその子孫）ごとにパレット色を割り当てる。

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
