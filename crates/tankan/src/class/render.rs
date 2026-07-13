//! classDiagram のレイアウト済みプリミティブ → SVG。
//! 関係マーカーは `<marker>` 4 種（`orient="auto-start-reverse"` で両端共用）

use std::fmt::Write;

use crate::Options;
use crate::class::layout::Layout;
use crate::class::model::Marker;
use crate::common::path::rounded_polyline;
use crate::common::svg::{SvgBuilder, fmt_num};
use crate::common::text::{escape_xml, max_width};

const PAD_X: f32 = 12.0;
const TITLE_PAD_Y: f32 = 6.0;
const COMPART_PAD_Y: f32 = 6.0;

pub(crate) fn to_svg(layout: &Layout, options: &Options) -> String {
    let p = &options.id_prefix;
    let t = &options.theme;

    let mut out = String::new();
    let _ = write!(
        out,
        concat!(
            r#"<svg class="tankan tankan-class" xmlns="http://www.w3.org/2000/svg" "#,
            r#"viewBox="0 0 {vw} {vh}" width="{w}" height="{h}" role="img" aria-label="{label}" "#,
            r#"font-family="{font}" font-size="{fs}">"#,
        ),
        vw = fmt_num(layout.width),
        vh = fmt_num(layout.height),
        w = fmt_num(layout.width * options.scale),
        h = fmt_num(layout.height * options.scale),
        label = escape_xml(
            &layout
                .title
                .as_ref()
                .map(|t| t.join(" "))
                .unwrap_or_else(|| "Class diagram".to_string())
        ),
        font = escape_xml(&options.font_family),
        fs = fmt_num(options.font_size),
    );
    out.push('\n');

    let _ = write!(
        out,
        "<style>\n\
         .tankan text {{ fill: {fg}; }}\n\
         .tankan .tk-class {{ fill: {bg}; stroke: {border}; }}\n\
         .tankan .tk-class-title {{ fill: {surface}; stroke: {border}; }}\n\
         .tankan .tk-class-name {{ font-weight: bold; }}\n\
         .tankan .tk-class-anno {{ fill: {muted}; font-style: italic; }}\n\
         .tankan .tk-class-line {{ stroke: {border}; }}\n\
         .tankan .tk-rel {{ stroke: {fg}; fill: none; }}\n\
         .tankan .tk-marker {{ fill: {bg}; stroke: {fg}; stroke-width: 1.2; }}\n\
         .tankan .tk-marker-fill {{ fill: {fg}; stroke: {fg}; stroke-width: 1.2; }}\n\
         .tankan .tk-marker-open {{ fill: none; stroke: {fg}; stroke-width: 1.2; }}\n\
         .tankan .tk-rel-label rect {{ fill: {bg}; stroke: none; }}\n\
         .tankan .tk-card {{ fill: {muted}; }}\n\
         </style>\n",
        fg = t.foreground,
        bg = t.background,
        border = t.border,
        muted = t.muted,
        surface = t.surface,
    );

    // 関係マーカー（線の進行方向 = クラスへ向かう向きで描く）
    let _ = write!(
        out,
        concat!(
            "<defs>\n",
            // 継承・実現: 白抜き三角（頂点が refX でクラスに触れる）
            r#"<marker id="{p}-cls-tri" viewBox="0 0 20 16" refX="18" refY="8" markerWidth="20" markerHeight="16" orient="auto-start-reverse"><path class="tk-marker" d="M2,2 L18,8 L2,14 Z"/></marker>"#,
            "\n",
            // コンポジション: 塗り菱形
            r#"<marker id="{p}-cls-diamond" viewBox="0 0 24 12" refX="22" refY="6" markerWidth="24" markerHeight="12" orient="auto-start-reverse"><path class="tk-marker-fill" d="M22,6 L12,2 L2,6 L12,10 Z"/></marker>"#,
            "\n",
            // 集約: 白抜き菱形
            r#"<marker id="{p}-cls-odiamond" viewBox="0 0 24 12" refX="22" refY="6" markerWidth="24" markerHeight="12" orient="auto-start-reverse"><path class="tk-marker" d="M22,6 L12,2 L2,6 L12,10 Z"/></marker>"#,
            "\n",
            // 関連・依存: 開き矢印
            r#"<marker id="{p}-cls-arrow" viewBox="0 0 16 12" refX="14" refY="6" markerWidth="14" markerHeight="12" orient="auto-start-reverse"><path class="tk-marker-open" d="M2,2 L14,6 L2,10"/></marker>"#,
            "\n</defs>\n",
        ),
        p = p,
    );

    let mut svg = SvgBuilder::new();
    let fs = options.font_size;
    let line_h = layout.line_h;

    // 関係
    for rel in &layout.relations {
        let dash = if rel.dashed {
            r#" stroke-dasharray="4,4""#
        } else {
            ""
        };
        let marker_start = marker_ref(p, rel.from_marker, "marker-start");
        let marker_end = marker_ref(p, rel.to_marker, "marker-end");
        let d = rounded_polyline(&rel.points, 6.0);
        svg.path("tk-rel", &d, &format!("{dash}{marker_start}{marker_end}"));

        if let Some((lx, ly)) = rel.label_at {
            if !rel.label.is_empty() {
                let lw = max_width(&rel.label, fs);
                let lh = rel.label.len() as f32 * line_h;
                svg.raw(r#"<g class="tk-rel-label">"#);
                svg.rect(
                    "",
                    lx - lw / 2.0 - 4.0,
                    ly - lh / 2.0 - 2.0,
                    lw + 8.0,
                    lh + 4.0,
                    "",
                );
                svg.text_lines(
                    "",
                    lx,
                    ly - lh / 2.0 + fs * 0.85,
                    line_h,
                    "middle",
                    &rel.label,
                );
                svg.raw("</g>");
            }
        }

        for (text, (x, y)) in [&rel.from_card, &rel.to_card].into_iter().flatten() {
            svg.text_lines(
                "tk-card",
                *x,
                *y,
                line_h,
                "middle",
                std::slice::from_ref(text),
            );
        }
    }

    // クラスボックス
    for c in &layout.classes {
        svg.rect("tk-class", c.x, c.y, c.w, c.h, "");
        // タイトル区画（下辺が名前区画の仕切り線を兼ねる）
        svg.rect("tk-class-title", c.x, c.y, c.w, c.title_h, "");

        let mut ty = c.y + TITLE_PAD_Y + fs * 0.85;
        if let Some(anno) = &c.annotation {
            svg.text_lines(
                "tk-class-anno",
                c.x + c.w / 2.0,
                ty,
                line_h,
                "middle",
                std::slice::from_ref(anno),
            );
            ty += line_h;
        }
        svg.text_lines(
            "tk-class-name",
            c.x + c.w / 2.0,
            ty,
            line_h,
            "middle",
            std::slice::from_ref(&c.name),
        );

        if c.has_body {
            // 属性区画とメソッド区画の仕切り線
            let mid = c.y + c.title_h + c.attr_h;
            svg.line("tk-class-line", c.x, mid, c.x + c.w, mid, "");

            let mut ay = c.y + c.title_h + COMPART_PAD_Y + fs * 0.85;
            for a in &c.attributes {
                svg.text_lines(
                    "tk-class-member",
                    c.x + PAD_X,
                    ay,
                    line_h,
                    "start",
                    std::slice::from_ref(a),
                );
                ay += line_h;
            }
            let mut my = c.y + c.title_h + c.attr_h + COMPART_PAD_Y + fs * 0.85;
            for m in &c.methods {
                svg.text_lines(
                    "tk-class-member",
                    c.x + PAD_X,
                    my,
                    line_h,
                    "start",
                    std::slice::from_ref(m),
                );
                my += line_h;
            }
        }
    }

    out.push_str(&svg.finish());
    out.push_str("</svg>");
    out
}

fn marker_ref(prefix: &str, marker: Marker, attr: &str) -> String {
    let id = match marker {
        Marker::None => return String::new(),
        Marker::Triangle => "cls-tri",
        Marker::DiamondFilled => "cls-diamond",
        Marker::DiamondHollow => "cls-odiamond",
        Marker::Arrow => "cls-arrow",
    };
    format!(r#" {attr}="url(#{prefix}-{id})""#)
}
