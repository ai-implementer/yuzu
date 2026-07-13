//! pie チャートの SVG レンダリング。
//!
//! 12 時位置から時計回りに扇形を描き、右側に凡例を置く。
//! 扇形の塗りはクラス（`tk-pie-c0`〜）＋ CSS 変数
//! （`--tankan-pie-1`〜、未定義時は既定パレット）で、ページ側のテーマから上書きできる。

use std::f32::consts::PI;
use std::fmt::Write;

use crate::Options;
use crate::common::svg::{SvgBuilder, fmt_num};
use crate::common::text::{escape_xml, text_width};
use crate::pie::model::PieChart;

/// 既定パレット（カテゴリカル 8 色。9 個目以降は循環）
const PALETTE: [&str; 8] = [
    "#5778a4", "#e49444", "#d1615d", "#85b6b2", "#6a9f58", "#e7ca60", "#a87c9f", "#967662",
];

const RADIUS: f32 = 90.0;
const PAD: f32 = 16.0;
const LEGEND_GAP: f32 = 28.0;
const SWATCH: f32 = 12.0;

pub(crate) fn to_svg(chart: &PieChart, options: &Options) -> String {
    let t = &options.theme;
    let fs = options.font_size;
    let line_h = fs * 1.6;
    let total = chart.total();

    // 凡例テキスト（showData なら値を付記）と幅
    let legend_texts: Vec<String> = chart
        .slices
        .iter()
        .map(|s| {
            if chart.show_data {
                format!("{} [{}]", s.label, s.raw_value)
            } else {
                s.label.clone()
            }
        })
        .collect();
    let legend_text_w = legend_texts
        .iter()
        .map(|l| text_width(l, fs))
        .fold(0.0, f32::max);
    let legend_w = SWATCH + 8.0 + legend_text_w;
    let legend_h = chart.slices.len() as f32 * line_h;

    let title_h = if chart.title.is_some() { fs * 2.0 } else { 0.0 };
    let content_h = (2.0 * RADIUS).max(legend_h);
    let width = PAD + 2.0 * RADIUS + LEGEND_GAP + legend_w + PAD;
    let height = PAD + title_h + content_h + PAD;

    let cx = PAD + RADIUS;
    let cy = PAD + title_h + content_h / 2.0;

    let mut out = String::new();
    let _ = write!(
        out,
        concat!(
            r#"<svg class="tankan tankan-pie" xmlns="http://www.w3.org/2000/svg" "#,
            r#"viewBox="0 0 {vw} {vh}" width="{w}" height="{h}" role="img" aria-label="{label}" "#,
            r#"font-family="{font}" font-size="{fs}">"#,
        ),
        vw = fmt_num(width),
        vh = fmt_num(height),
        w = fmt_num(width * options.scale),
        h = fmt_num(height * options.scale),
        label = escape_xml(chart.title.as_deref().unwrap_or("Pie chart")),
        font = escape_xml(&options.font_family),
        fs = fmt_num(fs),
    );
    out.push('\n');

    // パレットはクラス単位で CSS 変数化（SVG 属性に直接色を書かない）
    let _ = write!(
        out,
        "<style>\n\
         .tankan text {{ fill: {fg}; }}\n\
         .tankan .tk-pie-title {{ font-weight: bold; }}\n\
         .tankan .tk-pie-slice {{ stroke: {bg}; stroke-width: 2; }}\n\
         .tankan .tk-pie-pct {{ fill: {bg}; font-weight: bold; }}\n",
        fg = t.foreground,
        bg = t.background,
    );
    for (i, color) in PALETTE.iter().enumerate() {
        let _ = writeln!(
            out,
            ".tankan .tk-pie-c{i} {{ fill: var(--tankan-pie-{n}, {color}); }}",
            n = i + 1,
        );
    }
    out.push_str("</style>\n");

    let mut svg = SvgBuilder::new();

    if let Some(title) = &chart.title {
        svg.text_lines(
            "tk-pie-title",
            width / 2.0,
            PAD + fs,
            line_h,
            "middle",
            std::slice::from_ref(title),
        );
    }

    // 扇形（12 時位置から時計回り）。SVG の y 軸は下向きなので
    // 角度 = -90° 起点で増加方向がそのまま時計回りになる
    let point = |angle: f32| (cx + RADIUS * angle.cos(), cy + RADIUS * angle.sin());
    let mut angle = -PI / 2.0;
    for (i, slice) in chart.slices.iter().enumerate() {
        let class = format!("tk-pie-slice tk-pie-c{}", i % PALETTE.len());
        let fraction = slice.value / total;
        if fraction <= 0.0 {
            continue; // 値 0 は扇形なし（凡例には出す）
        }
        if fraction >= 0.999_99 {
            svg.circle(&class, cx, cy, RADIUS);
        } else {
            let sweep = fraction * 2.0 * PI;
            let (x1, y1) = point(angle);
            let (x2, y2) = point(angle + sweep);
            let large_arc = i32::from(fraction > 0.5);
            svg.path(
                &class,
                &format!(
                    "M{},{} L{},{} A{r},{r} 0 {large_arc} 1 {},{} Z",
                    fmt_num(cx),
                    fmt_num(cy),
                    fmt_num(x1),
                    fmt_num(y1),
                    fmt_num(x2),
                    fmt_num(y2),
                    r = fmt_num(RADIUS),
                ),
                "",
            );
        }

        // 扇形の中にパーセンテージ（整数丸め）
        let mid = angle + fraction * PI;
        let (px, py) = (
            cx + 0.62 * RADIUS * mid.cos(),
            cy + 0.62 * RADIUS * mid.sin(),
        );
        let pct = format!("{}%", (fraction * 100.0).round() as i64);
        svg.text_lines("tk-pie-pct", px, py + fs * 0.35, line_h, "middle", &[pct]);

        angle += fraction * 2.0 * PI;
    }

    // 凡例（右側・縦積み）
    let legend_x = PAD + 2.0 * RADIUS + LEGEND_GAP;
    let legend_y = PAD + title_h + (content_h - legend_h) / 2.0;
    for (i, text) in legend_texts.iter().enumerate() {
        let row_y = legend_y + i as f32 * line_h;
        svg.rect(
            &format!("tk-pie-c{}", i % PALETTE.len()),
            legend_x,
            row_y + (line_h - SWATCH) / 2.0,
            SWATCH,
            SWATCH,
            "",
        );
        svg.text_lines(
            "tk-pie-legend",
            legend_x + SWATCH + 8.0,
            row_y + line_h / 2.0 + fs * 0.35,
            line_h,
            "start",
            std::slice::from_ref(text),
        );
    }

    out.push_str(&svg.finish());
    out.push_str("</svg>\n");
    out
}
