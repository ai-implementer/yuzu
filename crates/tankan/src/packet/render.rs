//! packet のレイアウト済みプリミティブ → SVG。
//!
//! 本家 packet は全ブロック単色なのでパレット（CSS 変数循環）は持たず、
//! テーマの surface / border / foreground だけで描く（ダークモード追従は
//! `<style>` 内の Theme 文字列経由）。ラベルは折り返さずはみ出し許容
//! （rowHeight 固定 = 本家と同じ見た目を優先）。

use std::fmt::Write;

use crate::Options;
use crate::common::svg::{SvgBuilder, fmt_num};
use crate::common::text::escape_xml;
use crate::packet::layout::Layout;

pub(crate) fn to_svg(layout: &Layout, options: &Options) -> String {
    let t = &options.theme;
    let fs = options.font_size;

    let mut out = String::new();
    let _ = write!(
        out,
        concat!(
            r#"<svg class="tankan tankan-packet" xmlns="http://www.w3.org/2000/svg" "#,
            r#"viewBox="0 0 {vw} {vh}" width="{w}" height="{h}" role="img" aria-label="{label}" "#,
            r#"font-family="{font}" font-size="{fs}">"#,
        ),
        vw = fmt_num(layout.width),
        vh = fmt_num(layout.height),
        w = fmt_num(layout.width * options.scale),
        h = fmt_num(layout.height * options.scale),
        label = escape_xml(layout.title.as_deref().unwrap_or("Packet")),
        font = escape_xml(&options.font_family),
        fs = fmt_num(fs),
    );
    out.push('\n');

    let _ = write!(
        out,
        "<style>\n\
         .tankan text {{ fill: {fg}; }}\n\
         .tankan .tk-title {{ font-weight: bold; }}\n\
         .tankan .tk-pk-block {{ fill: {surface}; stroke: {border}; }}\n\
         .tankan .tk-pk-bit {{ fill: {muted}; font-size: {bit_fs}px; }}\n\
         </style>\n",
        fg = t.foreground,
        surface = t.surface,
        border = t.border,
        muted = t.muted,
        bit_fs = fmt_num(layout.bit_fs),
    );

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

    for block in &layout.blocks {
        svg.rect("tk-pk-block", block.x, block.y, block.w, block.h, "");
        if layout.show_bits {
            // ビット番号は各行上の番号帯へ（範囲は左右端・単一ビットは中央に 1 つ）
            let bit_y = block.y - 3.0;
            if block.bit_start == block.bit_end {
                svg.text_lines(
                    "tk-pk-bit",
                    block.x + block.w / 2.0,
                    bit_y,
                    layout.line_h,
                    "middle",
                    &[block.bit_start.to_string()],
                );
            } else {
                svg.text_lines(
                    "tk-pk-bit",
                    block.x,
                    bit_y,
                    layout.line_h,
                    "start",
                    &[block.bit_start.to_string()],
                );
                svg.text_lines(
                    "tk-pk-bit",
                    block.x + block.w,
                    bit_y,
                    layout.line_h,
                    "end",
                    &[block.bit_end.to_string()],
                );
            }
        }
        if !block.label.is_empty() {
            svg.text_lines(
                "tk-pk-label",
                block.x + block.w / 2.0,
                block.y + block.h / 2.0 + fs * 0.35,
                layout.line_h,
                "middle",
                std::slice::from_ref(&block.label),
            );
        }
    }

    out.push_str(&svg.finish());
    out.push_str("</svg>");
    out
}
