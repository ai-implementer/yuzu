//! erDiagram のレイアウト済みプリミティブ → SVG。
//! クロウズフット記号は `<marker>` 4 種（orient="auto-start-reverse" で両端共用）

use std::fmt::Write;

use crate::Options;
use crate::common::path::rounded_polyline;
use crate::common::svg::{SvgBuilder, fmt_num};
use crate::common::text::{escape_xml, max_width};
use crate::er::layout::Layout;
use crate::er::model::Cardinality;

pub(crate) fn to_svg(layout: &Layout, options: &Options) -> String {
    let p = &options.id_prefix;
    let t = &options.theme;

    let mut out = String::new();
    let _ = write!(
        out,
        concat!(
            r#"<svg class="tankan tankan-er" xmlns="http://www.w3.org/2000/svg" "#,
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
                .unwrap_or_else(|| "ER diagram".to_string())
        ),
        font = escape_xml(&options.font_family),
        fs = fmt_num(options.font_size),
    );
    out.push('\n');

    let _ = write!(
        out,
        "<style>\n\
         .tankan text {{ fill: {fg}; }}\n\
         .tankan .tk-entity {{ fill: {bg}; stroke: {border}; }}\n\
         .tankan .tk-entity-title {{ fill: {surface}; stroke: {border}; }}\n\
         .tankan .tk-entity-title-text {{ font-weight: bold; }}\n\
         .tankan .tk-entity-line {{ stroke: {border}; }}\n\
         .tankan .tk-entity-key {{ fill: {accent}; }}\n\
         .tankan .tk-entity-comment {{ fill: {muted}; }}\n\
         .tankan .tk-rel {{ stroke: {fg}; fill: none; }}\n\
         .tankan .tk-card {{ stroke: {fg}; fill: none; stroke-width: 1.2; }}\n\
         .tankan .tk-card-zero {{ stroke: {fg}; fill: {bg}; stroke-width: 1.2; }}\n\
         .tankan .tk-rel-label rect {{ fill: {bg}; stroke: none; }}\n\
         </style>\n",
        fg = t.foreground,
        bg = t.background,
        border = t.border,
        muted = t.muted,
        surface = t.surface,
        accent = t.accent,
    );

    // クロウズフット marker（線の進行方向 = エンティティへ向かう向きで描く）
    let _ = write!(
        out,
        concat!(
            "<defs>\n",
            // ちょうど 1: 縦バー 2 本
            r#"<marker id="{p}-one" viewBox="0 0 12 12" refX="10" refY="6" markerWidth="14" markerHeight="14" orient="auto-start-reverse"><path class="tk-card" d="M4,2 V10 M7,2 V10"/></marker>"#,
            "\n",
            // 0 or 1: 円 + 縦バー
            r#"<marker id="{p}-zeroone" viewBox="0 0 12 12" refX="10" refY="6" markerWidth="14" markerHeight="14" orient="auto-start-reverse"><circle class="tk-card-zero" cx="3.5" cy="6" r="2.3"/><path class="tk-card" d="M8,2 V10"/></marker>"#,
            "\n",
            // 1 以上: 縦バー + 鳥足
            r#"<marker id="{p}-onemany" viewBox="0 0 12 12" refX="11" refY="6" markerWidth="14" markerHeight="14" orient="auto-start-reverse"><path class="tk-card" d="M3,2 V10 M5,6 L11,2 M5,6 L11,6 M5,6 L11,10"/></marker>"#,
            "\n",
            // 0 以上: 円 + 鳥足
            r#"<marker id="{p}-zeromany" viewBox="0 0 12 12" refX="11" refY="6" markerWidth="14" markerHeight="14" orient="auto-start-reverse"><circle class="tk-card-zero" cx="3" cy="6" r="2.3"/><path class="tk-card" d="M5.5,6 L11,2 M5.5,6 L11,6 M5.5,6 L11,10"/></marker>"#,
            "\n</defs>\n",
        ),
        p = p,
    );

    let mut svg = SvgBuilder::new();
    let fs = options.font_size;
    let line_h = layout.line_h;

    // リレーション
    for rel in &layout.relations {
        let dash = if rel.identifying {
            ""
        } else {
            r#" stroke-dasharray="4,4""#
        };
        let markers = format!(
            r#" marker-start="url(#{p}-{})" marker-end="url(#{p}-{})""#,
            card_name(rel.from_card),
            card_name(rel.to_card),
        );
        let d = rounded_polyline(&rel.points, 6.0);
        svg.path("tk-rel", &d, &format!("{dash}{markers}"));

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
    }

    // エンティティ（テーブル）
    for e in &layout.entities {
        svg.rect("tk-entity", e.x, e.y, e.w, e.h, "");
        svg.rect("tk-entity-title", e.x, e.y, e.w, e.title_h, "");
        svg.text_lines(
            "tk-entity-title-text",
            e.x + e.w / 2.0,
            e.y + e.title_h / 2.0 + fs * 0.35,
            line_h,
            "middle",
            std::slice::from_ref(&e.title),
        );
        // 行と列
        let col_x = |i: usize| e.x + e.col_w[..i].iter().sum::<f32>();
        for (ri, row) in e.rows.iter().enumerate() {
            let ry = e.y + e.title_h + ri as f32 * e.row_h;
            if ri > 0 {
                svg.line("tk-entity-line", e.x, ry, e.x + e.w, ry, "");
            }
            let baseline = ry + e.row_h / 2.0 + fs * 0.35;
            for (ci, cell) in row.iter().enumerate() {
                if cell.is_empty() {
                    continue;
                }
                let class = match ci {
                    2 => "tk-entity-key",
                    3 => "tk-entity-comment",
                    _ => "tk-entity-cell",
                };
                svg.text_lines(
                    class,
                    col_x(ci) + 10.0,
                    baseline,
                    line_h,
                    "start",
                    std::slice::from_ref(cell),
                );
            }
        }
        // 列区切り（コメント列が空なら 3 列分）
        let cols = if e.col_w[3] > 0.0 { 4 } else { 3 };
        for ci in 1..cols {
            let x = col_x(ci);
            if x > e.x && x < e.x + e.w && !e.rows.is_empty() {
                svg.line("tk-entity-line", x, e.y + e.title_h, x, e.y + e.h, "");
            }
        }
    }

    out.push_str(&svg.finish());
    out.push_str("</svg>");
    out
}

fn card_name(card: Cardinality) -> &'static str {
    match card {
        Cardinality::One => "one",
        Cardinality::ZeroOne => "zeroone",
        Cardinality::OneMany => "onemany",
        Cardinality::ZeroMany => "zeromany",
    }
}
