//! flowchart のレイアウト済みプリミティブ → SVG。
//! クラス体系・marker の一意化・`<style>` テーマは sequence と同じ流儀

use std::fmt::Write;

use crate::Options;
use crate::common::path::rounded_polyline;
use crate::common::style::{box_attr, line_attr, text_attr};
use crate::common::svg::{SvgBuilder, fmt_num};
use crate::common::text::{escape_xml, max_width};
use crate::flowchart::layout::{Layout, NodeBox};
use crate::flowchart::model::{EdgeLine, EdgeTip, NodeShape};

/// flowchart 系レイアウトの共通レンダラ（stateDiagram も同じ経路を使う。
/// `svg_class` / `fallback_label` で図種の見た目を切り替える）
pub(crate) fn to_svg(
    layout: &Layout,
    options: &Options,
    svg_class: &str,
    fallback_label: &str,
) -> String {
    let p = &options.id_prefix;
    let t = &options.theme;

    let mut out = String::new();
    let _ = write!(
        out,
        concat!(
            r#"<svg class="tankan {class}" xmlns="http://www.w3.org/2000/svg" "#,
            r#"viewBox="0 0 {vw} {vh}" width="{w}" height="{h}" role="img" aria-label="{label}" "#,
            r#"font-family="{font}" font-size="{fs}">"#,
        ),
        class = svg_class,
        vw = fmt_num(layout.width),
        vh = fmt_num(layout.height),
        w = fmt_num(layout.width * options.scale),
        h = fmt_num(layout.height * options.scale),
        label = escape_xml(
            &layout
                .title
                .as_ref()
                .map(|t| t.join(" "))
                .unwrap_or_else(|| fallback_label.to_string())
        ),
        font = escape_xml(&options.font_family),
        fs = fmt_num(options.font_size),
    );
    out.push('\n');

    let _ = write!(
        out,
        "<style>\n\
         .tankan text {{ fill: {fg}; }}\n\
         .tankan .tk-node {{ fill: {surface}; stroke: {border}; }}\n\
         .tankan .tk-node-line {{ stroke: {border}; fill: none; }}\n\
         .tankan .tk-edge {{ stroke: {fg}; fill: none; }}\n\
         .tankan .tk-head-fill {{ fill: {fg}; }}\n\
         .tankan .tk-head-line {{ stroke: {fg}; fill: none; stroke-width: 1.5; }}\n\
         .tankan .tk-edge-label rect {{ fill: {bg}; stroke: none; }}\n\
         .tankan .tk-cluster {{ fill: {surface}; fill-opacity: 0.4; stroke: {border}; }}\n\
         .tankan .tk-cluster-title {{ fill: {muted}; }}\n\
         .tankan .tk-region {{ fill: none; stroke: {border}; stroke-dasharray: 4,4; }}\n\
         .tankan .tk-state-dot {{ fill: {fg}; stroke: none; }}\n\
         .tankan .tk-notebox {{ fill: {surface}; stroke: {border}; stroke-dasharray: 3,3; }}\n\
         </style>\n",
        fg = t.foreground,
        bg = t.background,
        border = t.border,
        muted = t.muted,
        surface = t.surface,
    );

    let _ = write!(
        out,
        concat!(
            "<defs>\n",
            r#"<marker id="{p}-arrow" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" orient="auto-start-reverse"><path class="tk-head-fill" d="M0,0 L10,5 L0,10 z"/></marker>"#,
            "\n",
            r#"<marker id="{p}-circle" viewBox="0 0 10 10" refX="5" refY="5" markerWidth="9" markerHeight="9" orient="auto"><circle class="tk-head-fill" cx="5" cy="5" r="4"/></marker>"#,
            "\n",
            r#"<marker id="{p}-cross" viewBox="0 0 10 10" refX="5" refY="5" markerWidth="9" markerHeight="9" orient="auto"><path class="tk-head-line" d="M1,1 L9,9 M9,1 L1,9"/></marker>"#,
            "\n</defs>\n",
        ),
        p = p,
    );

    let mut svg = SvgBuilder::new();
    let fs = options.font_size;
    let line_h = layout.line_h;

    // クラスタ（背景。外側から）
    for c in &layout.clusters {
        if c.region {
            svg.rect("tk-region", c.x, c.y, c.w, c.h, r#" rx="4""#);
            continue;
        }
        svg.rect("tk-cluster", c.x, c.y, c.w, c.h, r#" rx="4""#);
        svg.text_lines(
            "tk-cluster-title",
            c.x + c.w / 2.0,
            c.y + fs + 2.0,
            line_h,
            "middle",
            &c.title,
        );
    }

    // エッジ
    for edge in &layout.edges {
        if edge.line == EdgeLine::Invisible {
            continue;
        }
        let dash = match edge.line {
            EdgeLine::Dotted => r#" stroke-dasharray="3,3""#,
            _ => "",
        };
        let width_attr = match edge.line {
            EdgeLine::Thick => r#" stroke-width="3""#,
            _ => "",
        };
        let marker_end = marker_ref(p, edge.head, "marker-end");
        let marker_start = marker_ref(p, edge.tail, "marker-start");
        let d = rounded_polyline(&edge.points, 6.0);
        svg.path(
            "tk-edge",
            &d,
            &format!("{dash}{width_attr}{marker_start}{marker_end}"),
        );

        // ラベル（背景付き）
        if let Some((lx, ly)) = edge.label_at {
            if !edge.label.is_empty() {
                let lw = max_width(&edge.label, fs);
                let lh = edge.label.len() as f32 * line_h;
                let anchor_start = edge.self_loop;
                let rect_x = if anchor_start {
                    lx - 2.0
                } else {
                    lx - lw / 2.0 - 4.0
                };
                svg.raw(r#"<g class="tk-edge-label">"#);
                svg.rect("", rect_x, ly - lh / 2.0 - 2.0, lw + 8.0, lh + 4.0, "");
                svg.text_lines(
                    "",
                    if anchor_start { lx + 2.0 } else { lx },
                    ly - lh / 2.0 + fs * 0.85,
                    line_h,
                    if anchor_start { "start" } else { "middle" },
                    &edge.label,
                );
                svg.raw("</g>");
            }
        }
    }

    // ノード
    for node in &layout.nodes {
        draw_node(&mut svg, node);
        let text_top = node.cy - (node.label.len() as f32 * line_h) / 2.0 + fs * 0.85;
        // ラベル文字色（`.tankan text { fill }` に勝たせる）
        let label_style = text_attr(node.style.as_ref());
        svg.text_lines_with(
            "tk-node-label",
            node.cx,
            text_top,
            line_h,
            "middle",
            &node.label,
            &label_style,
        );
    }

    out.push_str(&svg.finish());
    out.push_str("</svg>");
    out
}

fn marker_ref(prefix: &str, tip: EdgeTip, attr: &str) -> String {
    match tip {
        EdgeTip::None => String::new(),
        EdgeTip::Arrow => format!(r#" {attr}="url(#{prefix}-arrow)""#),
        EdgeTip::Circle => format!(r#" {attr}="url(#{prefix}-circle)""#),
        EdgeTip::Cross => format!(r#" {attr}="url(#{prefix}-cross)""#),
    }
}

fn draw_node(svg: &mut SvgBuilder, node: &NodeBox) {
    use NodeShape::*;
    let (cx, cy, w, h) = (node.cx, node.cy, node.w, node.h);
    let (l, t, r, b) = (cx - w / 2.0, cy - h / 2.0, cx + w / 2.0, cy + h / 2.0);
    // 本体形状に付けるインラインスタイル（fill/stroke/…）と、補助線に付ける
    // stroke 系のみのスタイル（補助線に fill を当てると塗り潰れる）。
    // style が None なら両者とも空文字＝既存の出力とバイト一致
    let s = box_attr(node.style.as_ref());
    let line_s = line_attr(node.style.as_ref());
    match node.shape {
        Rect => svg.rect("tk-node", l, t, w, h, &s),
        Round => svg.rect("tk-node", l, t, w, h, &format!(r#" rx="6"{s}"#)),
        Stadium => {
            let rx = h / 2.0;
            svg.rect(
                "tk-node",
                l,
                t,
                w,
                h,
                &format!(r#" rx="{}"{s}"#, fmt_num(rx)),
            );
        }
        Subroutine => {
            svg.rect("tk-node", l, t, w, h, &s);
            svg.line("tk-node-line", l + 8.0, t, l + 8.0, b, &line_s);
            svg.line("tk-node-line", r - 8.0, t, r - 8.0, b, &line_s);
        }
        Cylinder => {
            let ry = 7.0f32;
            // 側面＋底の弧
            let d = format!(
                "M {l},{ty} v {body} a {rx},{ry} 0 0 0 {w},0 v -{body}",
                l = fmt_num(l),
                ty = fmt_num(t + ry),
                body = fmt_num(h - 2.0 * ry),
                rx = fmt_num(w / 2.0),
                ry = fmt_num(ry),
                w = fmt_num(w),
            );
            svg.path("tk-node", &d, &s);
            // 上面の楕円
            svg.raw(&format!(
                r#"<ellipse class="tk-node" cx="{}" cy="{}" rx="{}" ry="{}"{s}/>"#,
                fmt_num(cx),
                fmt_num(t + ry),
                fmt_num(w / 2.0),
                fmt_num(ry),
            ));
        }
        Circle => svg.circle_with("tk-node", cx, cy, w / 2.0, &s),
        DoubleCircle => {
            svg.circle_with("tk-node", cx, cy, w / 2.0, &s);
            svg.circle_with("tk-node-line", cx, cy, w / 2.0 - 4.0, &line_s);
        }
        Asymmetric => {
            // 左辺が旗形に切れ込む
            svg.polygon_with(
                "tk-node",
                &[(l + 10.0, t), (r, t), (r, b), (l + 10.0, b), (l, cy)],
                &s,
            );
        }
        Diamond => {
            svg.polygon_with("tk-node", &[(cx, t), (r, cy), (cx, b), (l, cy)], &s);
        }
        Hexagon => {
            let sh = h / 2.0;
            svg.polygon_with(
                "tk-node",
                &[
                    (l + sh, t),
                    (r - sh, t),
                    (r, cy),
                    (r - sh, b),
                    (l + sh, b),
                    (l, cy),
                ],
                &s,
            );
        }
        LeanRight => {
            let sh = h * 0.45;
            svg.polygon_with("tk-node", &[(l + sh, t), (r, t), (r - sh, b), (l, b)], &s);
        }
        LeanLeft => {
            let sh = h * 0.45;
            svg.polygon_with("tk-node", &[(l, t), (r - sh, t), (r, b), (l + sh, b)], &s);
        }
        TrapezoidBottom => {
            let sh = h * 0.45;
            svg.polygon_with("tk-node", &[(l + sh, t), (r - sh, t), (r, b), (l, b)], &s);
        }
        TrapezoidTop => {
            let sh = h * 0.45;
            svg.polygon_with("tk-node", &[(l, t), (r, t), (r - sh, b), (l + sh, b)], &s);
        }
        // 以下は stateDiagram 専用形状で style は常に None（見た目不変）
        StateStart => svg.circle("tk-state-dot", cx, cy, w / 2.0),
        StateEnd => {
            svg.circle("tk-node", cx, cy, w / 2.0);
            svg.circle("tk-state-dot", cx, cy, w / 2.0 - 4.0);
        }
        ForkBar(_) => svg.rect("tk-state-dot", l, t, w, h, r#" rx="2""#),
        NoteBox => svg.rect("tk-notebox", l, t, w, h, r#" rx="2""#),
    }
}
