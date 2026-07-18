//! timeline のレイアウト済みプリミティブ → SVG。
//!
//! 色はセクション（暗黙セクションのみの文書は期間）ごとにパレットを循環し、
//! クラス（`tk-tl-c0`〜）＋ CSS 変数（`--tankan-timeline-1`〜、未定義時は既定
//! パレット）でページ側のテーマから上書きできる。帯・期間箱・イベント箱の
//! 濃淡は fill-opacity で付け、テキストは常に foreground（ダークモード追従）

use std::fmt::Write;

use crate::Options;
use crate::common::svg::{SvgBuilder, fmt_num};
use crate::common::text::escape_xml;
use crate::timeline::layout::Layout;

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
            r#"<svg class="tankan tankan-timeline" xmlns="http://www.w3.org/2000/svg" "#,
            r#"viewBox="0 0 {vw} {vh}" width="{w}" height="{h}" role="img" aria-label="{label}" "#,
            r#"font-family="{font}" font-size="{fs}">"#,
        ),
        vw = fmt_num(layout.width),
        vh = fmt_num(layout.height),
        w = fmt_num(layout.width * options.scale),
        h = fmt_num(layout.height * options.scale),
        label = escape_xml(layout.title.as_deref().unwrap_or("Timeline")),
        font = escape_xml(&options.font_family),
        fs = fmt_num(fs),
    );
    out.push('\n');

    // パレットはクラス単位で CSS 変数化（SVG 属性に直接色を書かない）
    let _ = write!(
        out,
        "<style>\n\
         .tankan text {{ fill: {fg}; }}\n\
         .tankan .tk-title {{ font-weight: bold; }}\n\
         .tankan .tk-tl-band {{ fill-opacity: 0.3; }}\n\
         .tankan .tk-tl-period {{ fill-opacity: 0.55; stroke: {border}; }}\n\
         .tankan .tk-tl-event {{ fill-opacity: 0.18; stroke: {border}; }}\n\
         .tankan .tk-tl-line {{ stroke: {border}; }}\n",
        fg = t.foreground,
        border = t.border,
    );
    for (i, color) in PALETTE.iter().enumerate() {
        let _ = writeln!(
            out,
            ".tankan .tk-tl-c{i} {{ fill: var(--tankan-timeline-{n}, {color}); }}",
            n = i + 1,
        );
    }
    out.push_str("</style>\n");

    let mut svg = SvgBuilder::new();

    if let Some(title) = &layout.title {
        svg.text_lines(
            "tk-title",
            layout.width / 2.0,
            16.0 + fs * 0.85,
            layout.line_h,
            "middle",
            std::slice::from_ref(title),
        );
    }

    // セクション帯とラベル
    for band in &layout.bands {
        svg.rect(
            &format!("tk-tl-band tk-tl-c{}", band.color % PALETTE.len()),
            band.x,
            band.y,
            band.w,
            band.h,
            r#" rx="4""#,
        );
        svg.text_lines(
            "tk-tl-section",
            band.x + band.w / 2.0,
            band.y + band.h / 2.0 + fs * 0.35,
            layout.line_h,
            "middle",
            std::slice::from_ref(&band.label),
        );
    }

    // 接続線（箱の隙間のみ）→ 期間箱 → イベント箱の順に描く
    for &(x, y1, y2) in &layout.connectors {
        svg.line("tk-tl-line", x, y1, x, y2, "");
    }
    let draw_box = |svg: &mut SvgBuilder, kind: &str, item: &crate::timeline::layout::BoxItem| {
        svg.rect(
            &format!("{kind} tk-tl-c{}", item.color % PALETTE.len()),
            item.x,
            item.y,
            item.w,
            item.h,
            r#" rx="4""#,
        );
        let text_top = item.y + item.h / 2.0
            - (item.lines.len() as f32 - 1.0) * layout.line_h / 2.0
            + fs * 0.35;
        svg.text_lines(
            "tk-tl-text",
            item.x + item.w / 2.0,
            text_top,
            layout.line_h,
            "middle",
            &item.lines,
        );
    };
    for item in &layout.periods {
        draw_box(&mut svg, "tk-tl-period", item);
    }
    for item in &layout.events {
        draw_box(&mut svg, "tk-tl-event", item);
    }

    out.push_str(&svg.finish());
    out.push_str("</svg>");
    out
}
