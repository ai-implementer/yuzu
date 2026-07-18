//! mindmap のレイアウト済みプリミティブ → SVG。
//!
//! ブランチ（ルート直下の子とその子孫）ごとにパレット色を割り当て、
//! クラス（`tk-mm-b0`〜）＋ CSS 変数（`--tankan-mindmap-1`〜、未定義時は既定
//! パレット）でページ側のテーマから上書きできる。ノードの塗りは fill-opacity で
//! 淡くし、枠線・エッジはブランチ色。テキストは常に foreground（ダークモード追従）。
//! Bang（破線円）・Cloud（楕円）は近似形状で描く

use std::fmt::Write;

use crate::Options;
use crate::common::svg::{SvgBuilder, fmt_num};
use crate::common::text::escape_xml;
use crate::mindmap::layout::Layout;
use crate::mindmap::model::NodeShape;

/// 既定パレット（カテゴリカル 8 色。9 個目以降は循環。pie と同値）
const PALETTE: [&str; 8] = [
    "#5778a4", "#e49444", "#d1615d", "#85b6b2", "#6a9f58", "#e7ca60", "#a87c9f", "#967662",
];

pub(crate) fn to_svg(layout: &Layout, options: &Options) -> String {
    let t = &options.theme;
    let fs = options.font_size;

    let mut out = String::new();
    let _ = write!(
        out,
        concat!(
            r#"<svg class="tankan tankan-mindmap" xmlns="http://www.w3.org/2000/svg" "#,
            r#"viewBox="0 0 {vw} {vh}" width="{w}" height="{h}" role="img" aria-label="{label}" "#,
            r#"font-family="{font}" font-size="{fs}">"#,
        ),
        vw = fmt_num(layout.width),
        vh = fmt_num(layout.height),
        w = fmt_num(layout.width * options.scale),
        h = fmt_num(layout.height * options.scale),
        label = escape_xml(&layout.label),
        font = escape_xml(&options.font_family),
        fs = fmt_num(fs),
    );
    out.push('\n');

    // パレットはクラス単位で CSS 変数化（SVG 属性に直接色を書かない）。
    // `.tk-mm-edge { fill: none }` はパレットの fill を打ち消すため最後に置く
    let _ = write!(
        out,
        "<style>\n\
         .tankan text {{ fill: {fg}; }}\n\
         .tankan .tk-mm-root {{ fill: {surface}; stroke: {border}; stroke-width: 1.5; }}\n\
         .tankan .tk-mm-root-label {{ font-weight: bold; }}\n\
         .tankan .tk-mm-node {{ fill-opacity: 0.15; }}\n",
        fg = t.foreground,
        surface = t.surface,
        border = t.border,
    );
    for (i, color) in PALETTE.iter().enumerate() {
        let _ = writeln!(
            out,
            ".tankan .tk-mm-b{i} {{ fill: var(--tankan-mindmap-{n}, {color}); stroke: var(--tankan-mindmap-{n}, {color}); }}",
            n = i + 1,
        );
    }
    out.push_str(
        ".tankan .tk-mm-edge { fill: none; stroke-width: 2; stroke-opacity: 0.7; }\n</style>\n",
    );

    let mut svg = SvgBuilder::new();

    // エッジ → ノードの順（線をノードの下に敷く）
    for (d, branch) in &layout.edges {
        svg.path(
            &format!("tk-mm-edge tk-mm-b{}", branch % PALETTE.len()),
            d,
            "",
        );
    }

    for node in &layout.nodes {
        let class = match node.branch {
            Some(b) => format!("tk-mm-node tk-mm-b{}", b % PALETTE.len()),
            None => "tk-mm-root".to_string(),
        };
        let (cx, cy) = (node.x + node.w / 2.0, node.y + node.h / 2.0);
        match node.shape {
            NodeShape::Square => svg.rect(&class, node.x, node.y, node.w, node.h, r#" rx="3""#),
            NodeShape::Rounded => svg.rect(&class, node.x, node.y, node.w, node.h, r#" rx="8""#),
            NodeShape::Circle => svg.circle_with(&class, cx, cy, node.w / 2.0, ""),
            NodeShape::Bang => {
                // バンは破線円の近似
                svg.circle_with(&class, cx, cy, node.w / 2.0, r#" stroke-dasharray="6 3""#)
            }
            NodeShape::Cloud => {
                // 雲は楕円の近似
                svg.ellipse(&class, cx, cy, node.w / 2.0, node.h / 2.0, "")
            }
            NodeShape::Hexagon => {
                let inset = node.h * 0.3;
                svg.polygon_with(
                    &class,
                    &[
                        (node.x, cy),
                        (node.x + inset, node.y),
                        (node.x + node.w - inset, node.y),
                        (node.x + node.w, cy),
                        (node.x + node.w - inset, node.y + node.h),
                        (node.x + inset, node.y + node.h),
                    ],
                    "",
                );
            }
            // 無印はスタジアム風（角丸を高さの半分に）
            NodeShape::Default => svg.rect(
                &class,
                node.x,
                node.y,
                node.w,
                node.h,
                &format!(r#" rx="{}""#, fmt_num(node.h / 2.0)),
            ),
        }

        let text_class = if node.branch.is_none() {
            "tk-mm-root-label"
        } else {
            "tk-mm-label"
        };
        let text_top = cy - (node.lines.len() as f32 - 1.0) * layout.line_h / 2.0 + fs * 0.35;
        svg.text_lines(
            text_class,
            cx,
            text_top,
            layout.line_h,
            "middle",
            &node.lines,
        );
    }

    out.push_str(&svg.finish());
    out.push_str("</svg>");
    out
}
