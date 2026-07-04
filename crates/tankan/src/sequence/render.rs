//! レイアウト済みプリミティブ → SVG 文字列。
//!
//! - クラスは `tk-` 名前空間（mermaid.js の CSS と衝突させない）。
//!   **`mermaid` クラスは付けない**（mermaid.run() の再処理対象になるため）
//! - 色は `<style>` ブロック経由（SVG 属性内の var() は仕様上効かないため。
//!   インライン SVG ならページの CSS 変数がカスケードで届く）
//! - `<marker>` の id は文書グローバルなので `id_prefix` で一意化する

use std::fmt::Write;

use crate::Options;
use crate::common::svg::{SvgBuilder, fmt_num};
use crate::common::text::escape_xml;
use crate::sequence::layout::{Layout, TextAt};
use crate::sequence::model::{HeadKind, LineKind};

pub(crate) fn to_svg(layout: &Layout, options: &Options) -> String {
    let p = &options.id_prefix;
    let t = &options.theme;
    let (w, h) = (layout.width, layout.height);

    let mut out = String::new();
    let _ = write!(
        out,
        concat!(
            r#"<svg class="tankan tankan-sequence" xmlns="http://www.w3.org/2000/svg" "#,
            r#"viewBox="0 0 {vw} {vh}" width="{w}" height="{h}" role="img" aria-label="{label}" "#,
            r#"font-family="{font}" font-size="{fs}">"#,
        ),
        vw = fmt_num(w),
        vh = fmt_num(h),
        w = fmt_num(w * options.scale),
        h = fmt_num(h * options.scale),
        label = escape_xml(
            &layout
                .title
                .as_ref()
                .map(|t| t.lines.join(" "))
                .unwrap_or_else(|| "Sequence diagram".to_string())
        ),
        font = escape_xml(&options.font_family),
        fs = fmt_num(options.font_size),
    );
    out.push('\n');

    // テーマ（すべて CSS 値として埋め込む。var() 参照可）
    let _ = write!(
        out,
        "<style>\n\
         .tankan text {{ fill: {fg}; }}\n\
         .tankan .tk-actor rect {{ fill: {bg}; stroke: {border}; }}\n\
         .tankan .tk-actor-figure {{ stroke: {fg}; fill: none; }}\n\
         .tankan .tk-lifeline {{ stroke: {muted}; }}\n\
         .tankan .tk-msg {{ stroke: {fg}; fill: none; }}\n\
         .tankan .tk-head-fill {{ fill: {fg}; }}\n\
         .tankan .tk-head-line {{ stroke: {fg}; fill: none; stroke-width: 1.5; }}\n\
         .tankan .tk-activation {{ fill: {surface}; stroke: {border}; }}\n\
         .tankan .tk-note {{ fill: {surface}; stroke: {border}; }}\n\
         .tankan .tk-frame {{ stroke: {border}; fill: none; }}\n\
         .tankan .tk-frame-sep {{ stroke: {border}; stroke-dasharray: 4,4; }}\n\
         .tankan .tk-frame-labelbox {{ fill: {surface}; stroke: {border}; }}\n\
         .tankan .tk-frame-kind {{ font-weight: bold; }}\n\
         .tankan .tk-frame-label {{ fill: {muted}; }}\n\
         .tankan .tk-groupbox {{ stroke: {border}; fill: none; }}\n\
         .tankan .tk-groupbox-bg {{ stroke: none; }}\n\
         .tankan .tk-seqnum circle {{ fill: {accent}; }}\n\
         .tankan .tk-seqnum text {{ fill: {bg}; font-size: {numfs}px; }}\n\
         .tankan .tk-title {{ font-weight: bold; }}\n\
         </style>\n",
        fg = t.foreground,
        bg = t.background,
        border = t.border,
        muted = t.muted,
        surface = t.surface,
        accent = t.accent,
        numfs = fmt_num(options.font_size * 0.8),
    );

    // マーカー（矢じり）
    let _ = write!(
        out,
        concat!(
            "<defs>\n",
            r#"<marker id="{p}-head" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" orient="auto-start-reverse"><path class="tk-head-fill" d="M0,0 L10,5 L0,10 z"/></marker>"#,
            "\n",
            r#"<marker id="{p}-open" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="9" markerHeight="9" orient="auto-start-reverse"><path class="tk-head-line" d="M1,1 L9,5 L1,9"/></marker>"#,
            "\n",
            r#"<marker id="{p}-cross" viewBox="0 0 10 10" refX="5" refY="5" markerWidth="8" markerHeight="8" orient="auto"><path class="tk-head-line" d="M1,1 L9,9 M9,1 L1,9"/></marker>"#,
            "\n</defs>\n",
        ),
        p = p,
    );

    let mut svg = SvgBuilder::new();
    let fs = options.font_size;
    let line_h = layout.line_h;

    // rect ブロック背景（最背面）
    for r in &layout.rect_bgs {
        svg.raw(&format!(
            r#"<rect class="tk-rectblock" x="{}" y="{}" width="{}" height="{}" fill="{}" opacity="0.35"/>"#,
            fmt_num(r.x),
            fmt_num(r.y),
            fmt_num(r.w),
            fmt_num(r.h),
            escape_xml(&r.color),
        ));
    }

    // box（参加者グルーピング）
    for b in &layout.group_boxes {
        if let Some(color) = &b.color {
            svg.raw(&format!(
                r#"<rect class="tk-groupbox-bg" x="{}" y="{}" width="{}" height="{}" fill="{}" opacity="0.3"/>"#,
                fmt_num(b.x),
                fmt_num(b.y),
                fmt_num(b.w),
                fmt_num(b.h),
                escape_xml(color),
            ));
        }
        svg.rect("tk-groupbox", b.x, b.y, b.w, b.h, r#" rx="3""#);
        svg.text_lines(
            "tk-groupbox-label",
            b.x + b.w / 2.0,
            b.y + fs,
            line_h,
            "middle",
            &b.label,
        );
    }

    // ライフライン
    for &(x, y1, y2) in &layout.lifelines {
        svg.line("tk-lifeline", x, y1, x, y2, "");
    }

    // activation バー
    for bar in &layout.activations {
        svg.rect("tk-activation", bar.x, bar.y1, 10.0, bar.y2 - bar.y1, "");
    }

    // ブロックフレーム（loop/alt/...）
    for f in &layout.frames {
        svg.rect("tk-frame", f.x, f.y, f.w, f.h, "");
        // 五角形のキーワードラベル
        let (lw, lh) = (50.0_f32, 20.0_f32);
        svg.polygon(
            "tk-frame-labelbox",
            &[
                (f.x, f.y),
                (f.x + lw, f.y),
                (f.x + lw, f.y + lh - 6.0),
                (f.x + lw - 6.0, f.y + lh),
                (f.x, f.y + lh),
            ],
        );
        svg.text_lines(
            "tk-frame-kind",
            f.x + lw / 2.0 - 3.0,
            f.y + lh - 6.0,
            line_h,
            "middle",
            &[f.kind.to_string()],
        );
        if !f.label.is_empty() && !f.label[0].is_empty() {
            let label = brackets(&f.label);
            svg.text_lines(
                "tk-frame-label",
                f.x + lw + 8.0,
                f.y + lh - 6.0,
                line_h,
                "start",
                &label,
            );
        }
        for (y, label) in &f.separators {
            svg.line("tk-frame-sep", f.x, *y, f.x + f.w, *y, "");
            if !label.is_empty() && !label[0].is_empty() {
                svg.text_lines(
                    "tk-frame-label",
                    f.x + 8.0,
                    y + line_h,
                    line_h,
                    "start",
                    &brackets(label),
                );
            }
        }
    }

    // メッセージ
    for m in &layout.messages {
        let dash = match m.line {
            LineKind::Solid => "",
            LineKind::Dotted => r#" stroke-dasharray="3,3""#,
        };
        let marker_end = match m.head {
            HeadKind::None => String::new(),
            HeadKind::Arrow | HeadKind::BothArrow => format!(r#" marker-end="url(#{p}-head)""#),
            HeadKind::Cross => format!(r#" marker-end="url(#{p}-cross)""#),
            HeadKind::Open => format!(r#" marker-end="url(#{p}-open)""#),
        };
        let marker_start = match m.head {
            HeadKind::BothArrow => format!(r#" marker-start="url(#{p}-head)""#),
            _ => String::new(),
        };
        if m.self_msg {
            let d = format!(
                "M {},{} C {},{} {},{} {},{}",
                fmt_num(m.x1),
                fmt_num(m.y),
                fmt_num(m.x1 + 56.0),
                fmt_num(m.y - 8.0),
                fmt_num(m.x1 + 56.0),
                fmt_num(m.y + 28.0),
                fmt_num(m.x1),
                fmt_num(m.y + 20.0),
            );
            svg.path("tk-msg", &d, &format!("{dash}{marker_end}"));
        } else {
            svg.line(
                "tk-msg",
                m.x1,
                m.y,
                m.x2,
                m.y,
                &format!("{dash}{marker_start}{marker_end}"),
            );
        }
        if let Some(text) = &m.text {
            let anchor = if m.self_msg { "start" } else { "middle" };
            svg.text_lines("tk-msg-text", text.x, text.y, line_h, anchor, &text.lines);
        }
        if let Some(number) = m.number {
            let (cx, cy) = if m.self_msg {
                (m.x1 + 12.0, m.y - 2.0)
            } else if m.x1 <= m.x2 {
                (m.x1 + 12.0, m.y)
            } else {
                (m.x1 - 12.0, m.y)
            };
            svg.raw(&format!(
                concat!(
                    r#"<g class="tk-seqnum"><circle cx="{cx}" cy="{cy}" r="9"/>"#,
                    r#"<text x="{cx}" y="{ty}" text-anchor="middle">{n}</text></g>"#,
                ),
                cx = fmt_num(cx),
                cy = fmt_num(cy),
                ty = fmt_num(cy + 3.5),
                n = number,
            ));
        }
    }

    // Note
    for note in &layout.notes {
        svg.rect("tk-note", note.x, note.y, note.w, note.h, r#" rx="2""#);
        svg.text_lines(
            "tk-note-text",
            note.x + note.w / 2.0,
            note.y + 8.0 + fs * 0.8,
            line_h,
            "middle",
            &note.lines,
        );
    }

    // 参加者ボックス（上端＋下端ミラー）
    for actor in &layout.actors {
        for top in [layout.actor_top_y, layout.mirror_y] {
            svg.raw(r#"<g class="tk-actor">"#);
            svg.rect(
                "tk-actor-box",
                actor.cx - actor.w / 2.0,
                top,
                actor.w,
                layout.actor_h,
                r#" rx="3""#,
            );
            if actor.is_actor {
                // 簡易スティックフィギュア
                let (cx, cy) = (actor.cx - actor.w / 2.0 + 14.0, top + 12.0);
                svg.circle("tk-actor-figure", cx, cy, 4.0);
                svg.path(
                    "tk-actor-figure",
                    &format!(
                        "M {cx},{y1} v 8 m -5,-5 h 10 m -5,5 l -4,6 m 4,-6 l 4,6",
                        cx = fmt_num(cx),
                        y1 = fmt_num(cy + 4.0),
                    ),
                    "",
                );
            }
            let text_top =
                top + (layout.actor_h - actor.lines.len() as f32 * line_h) / 2.0 + fs - 2.0;
            svg.text_lines(
                "tk-actor-label",
                actor.cx,
                text_top,
                line_h,
                "middle",
                &actor.lines,
            );
            svg.raw("</g>");
        }
    }

    // タイトル
    if let Some(title) = &layout.title {
        emit_text(&mut svg, "tk-title", title, line_h, "middle");
    }

    out.push_str(&svg.finish());
    out.push_str("</svg>");
    out
}

fn emit_text(svg: &mut SvgBuilder, class: &str, text: &TextAt, line_h: f32, anchor: &str) {
    svg.text_lines(class, text.x, text.y, line_h, anchor, &text.lines);
}

/// ブロック条件ラベルの mermaid 風ブラケット表示（1 行目のみ [ ] で囲む）
fn brackets(label: &[String]) -> Vec<String> {
    label
        .iter()
        .enumerate()
        .map(|(i, l)| if i == 0 { format!("[{l}]") } else { l.clone() })
        .collect()
}
