//! gantt のレイアウト済みプリミティブ → SVG

use std::fmt::Write;

use crate::Options;
use crate::common::svg::{SvgBuilder, fmt_num};
use crate::common::text::escape_xml;
use crate::gantt::layout::Layout;

pub(crate) fn to_svg(layout: &Layout, options: &Options) -> String {
    let t = &options.theme;

    let mut out = String::new();
    let _ = write!(
        out,
        concat!(
            r#"<svg class="tankan tankan-gantt" xmlns="http://www.w3.org/2000/svg" "#,
            r#"viewBox="0 0 {vw} {vh}" width="{w}" height="{h}" role="img" aria-label="{label}" "#,
            r#"font-family="{font}" font-size="{fs}">"#,
        ),
        vw = fmt_num(layout.width),
        vh = fmt_num(layout.height),
        w = fmt_num(layout.width * options.scale),
        h = fmt_num(layout.height * options.scale),
        label = escape_xml(layout.title.as_deref().unwrap_or("Gantt chart")),
        font = escape_xml(&options.font_family),
        fs = fmt_num(options.font_size),
    );
    out.push('\n');

    let _ = write!(
        out,
        "<style>\n\
         .tankan text {{ fill: {fg}; }}\n\
         .tankan .tk-title {{ font-weight: bold; }}\n\
         .tankan .tk-section-band {{ fill: {surface}; opacity: 0.5; }}\n\
         .tankan .tk-section-label {{ fill: {muted}; }}\n\
         .tankan .tk-grid {{ stroke: {border}; }}\n\
         .tankan .tk-axis-label {{ fill: {muted}; }}\n\
         .tankan .tk-excluded {{ fill: {surface}; opacity: 0.7; }}\n\
         .tankan .tk-task {{ fill: {surface}; stroke: {border}; }}\n\
         .tankan .tk-task-active {{ fill: {bg}; stroke: {accent}; }}\n\
         .tankan .tk-task-done {{ fill: {border}; stroke: {border}; }}\n\
         .tankan .tk-task-crit {{ stroke: {accent}; stroke-width: 2; }}\n\
         .tankan .tk-milestone {{ fill: {accent}; stroke: {border}; }}\n\
         .tankan .tk-task-name-done {{ fill: {muted}; }}\n\
         </style>\n",
        fg = t.foreground,
        bg = t.background,
        border = t.border,
        muted = t.muted,
        surface = t.surface,
        accent = t.accent,
    );

    let mut svg = SvgBuilder::new();
    let fs = options.font_size;

    // セクション帯（交互）とラベル
    for (y, h, name, alternate) in &layout.sections {
        if *alternate {
            svg.rect("tk-section-band", 0.0, *y, layout.width, *h, "");
        }
        if !name.is_empty() {
            svg.text_lines(
                "tk-section-label",
                8.0,
                y + h / 2.0 + fs * 0.35,
                layout.line_h,
                "start",
                std::slice::from_ref(name),
            );
        }
    }

    // 除外日の網掛け
    for &(x, w) in &layout.excluded {
        svg.rect(
            "tk-excluded",
            x,
            layout.chart_top,
            w,
            layout.chart_bottom - layout.chart_top,
            "",
        );
    }

    // 軸線（チャート下端のベースライン）
    svg.line(
        "tk-grid",
        layout.chart_left,
        layout.chart_bottom,
        layout.chart_right,
        layout.chart_bottom,
        "",
    );

    // グリッドと軸ラベル
    for (x, label) in &layout.ticks {
        svg.line(
            "tk-grid",
            *x,
            layout.chart_top,
            *x,
            layout.chart_bottom + 4.0,
            "",
        );
        svg.text_lines(
            "tk-axis-label",
            *x,
            layout.chart_bottom + fs + 6.0,
            layout.line_h,
            "middle",
            std::slice::from_ref(label),
        );
    }

    // タスクバー
    for bar in &layout.bars {
        if bar.tags.milestone {
            let cx = bar.x + bar.w / 2.0;
            let cy = bar.y + bar.h / 2.0;
            let r = bar.h / 2.0;
            svg.polygon(
                "tk-milestone",
                &[(cx, cy - r), (cx + r, cy), (cx, cy + r), (cx - r, cy)],
            );
        } else {
            let mut class = String::from("tk-task");
            if bar.tags.done {
                class = "tk-task tk-task-done".to_string();
            } else if bar.tags.active {
                class = "tk-task tk-task-active".to_string();
            }
            if bar.tags.crit {
                class.push_str(" tk-task-crit");
            }
            svg.rect(&class, bar.x, bar.y, bar.w, bar.h, r#" rx="3""#);
        }

        let name_class = if bar.tags.done {
            "tk-task-name tk-task-name-done"
        } else {
            "tk-task-name"
        };
        let baseline = bar.y + bar.h / 2.0 + fs * 0.35;
        if bar.name_inside {
            svg.text_lines(
                name_class,
                bar.x + bar.w / 2.0,
                baseline,
                layout.line_h,
                "middle",
                std::slice::from_ref(&bar.name),
            );
        } else {
            svg.text_lines(
                name_class,
                bar.x + bar.w + 8.0,
                baseline,
                layout.line_h,
                "start",
                std::slice::from_ref(&bar.name),
            );
        }
    }

    // タイトル
    if let Some(title) = &layout.title {
        svg.text_lines(
            "tk-title",
            layout.width / 2.0,
            20.0 + fs * 0.35,
            layout.line_h,
            "middle",
            std::slice::from_ref(title),
        );
    }

    out.push_str(&svg.finish());
    out.push_str("</svg>");
    out
}
