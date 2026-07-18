//! timeline のレイアウト。
//!
//! 全期間を等間隔カラムで横に並べ（幅は全テキストの折返し後実測の最大値で一律）、
//! 縦は タイトル → セクション帯 → 期間箱 → イベント箱の縦積み の順。
//! 色 index はセクション番号（暗黙セクションのみの文書は期間番号 = mermaid の見た目）

use crate::Options;
use crate::common::text::{max_width, wrap_text};
use crate::timeline::model::TimelineDiagram;

const MARGIN: f32 = 16.0;
/// カラム幅の下限
const COL_MIN: f32 = 80.0;
/// テキストの折返し幅（px）
const WRAP_W: f32 = 160.0;
const COL_GAP: f32 = 12.0;
const BOX_PAD_X: f32 = 10.0;
const BOX_PAD_Y: f32 = 6.0;
/// 期間箱とイベント箱・イベント箱同士の縦間隔
const V_GAP: f32 = 8.0;

pub(crate) struct Layout {
    pub width: f32,
    pub height: f32,
    pub line_h: f32,
    pub title: Option<String>,
    /// セクション帯（暗黙セクションのみの文書では空）
    pub bands: Vec<Band>,
    pub periods: Vec<BoxItem>,
    pub events: Vec<BoxItem>,
    /// 箱の隙間だけを結ぶ縦線（x, y1, y2）
    pub connectors: Vec<(f32, f32, f32)>,
}

pub(crate) struct Band {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub label: String,
    pub color: usize,
}

pub(crate) struct BoxItem {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub lines: Vec<String>,
    pub color: usize,
}

pub(crate) fn layout(diagram: &TimelineDiagram, options: &Options) -> Layout {
    let fs = options.font_size;
    let line_h = fs * 1.4;

    // 期間を通し番号で平坦化（色 index を伴う）
    let has_named_sections = diagram.sections.iter().any(|s| s.name.is_some());
    struct Flat {
        color: usize,
        label_lines: Vec<String>,
        event_lines: Vec<Vec<String>>,
    }
    let mut flats: Vec<Flat> = Vec::new();
    for (si, section) in diagram.sections.iter().enumerate() {
        for period in &section.periods {
            let color = if has_named_sections { si } else { flats.len() };
            flats.push(Flat {
                color,
                label_lines: wrap_text(&period.label, fs, WRAP_W),
                event_lines: period
                    .events
                    .iter()
                    .map(|e| wrap_text(e, fs, WRAP_W))
                    .collect(),
            });
        }
    }

    // カラム幅 = 全テキスト（折返し後）の最大幅 + パディング（全カラム一律）
    let text_w = flats
        .iter()
        .flat_map(|f| std::iter::once(&f.label_lines).chain(f.event_lines.iter()))
        .map(|lines| max_width(lines, fs))
        .fold(0.0, f32::max);
    let col_w = (text_w + 2.0 * BOX_PAD_X).max(COL_MIN);
    let pitch = col_w + COL_GAP;
    let col_x = |i: usize| MARGIN + i as f32 * pitch;

    let mut y = MARGIN;
    let title = diagram.title.clone();
    if title.is_some() {
        y += line_h + V_GAP;
    }

    // セクション帯（名前付きセクションがある場合のみ）
    let mut bands = Vec::new();
    if has_named_sections {
        let band_h = line_h + 8.0;
        let mut start = 0usize;
        for (si, section) in diagram.sections.iter().enumerate() {
            let n = section.periods.len();
            if n > 0 {
                bands.push(Band {
                    x: col_x(start),
                    y,
                    w: (n as f32) * pitch - COL_GAP,
                    h: band_h,
                    label: section.name.clone().unwrap_or_default(),
                    color: si,
                });
            }
            start += n;
        }
        y += band_h + V_GAP;
    }

    // 期間箱の行（高さは全期間で一律 = 最大行数に合わせる）
    let period_lines_max = flats.iter().map(|f| f.label_lines.len()).max().unwrap_or(1);
    let period_h = period_lines_max as f32 * line_h + 2.0 * BOX_PAD_Y;
    let period_y = y;
    let mut periods = Vec::new();
    let mut events = Vec::new();
    let mut connectors = Vec::new();
    let mut max_bottom = period_y + period_h;
    for (i, flat) in flats.iter().enumerate() {
        let x = col_x(i);
        periods.push(BoxItem {
            x,
            y: period_y,
            w: col_w,
            h: period_h,
            lines: flat.label_lines.clone(),
            color: flat.color,
        });
        // イベント箱を縦積みし、隙間を縦線で結ぶ
        let cx = x + col_w / 2.0;
        let mut ey = period_y + period_h;
        for lines in &flat.event_lines {
            let h = lines.len() as f32 * line_h + 2.0 * BOX_PAD_Y;
            connectors.push((cx, ey, ey + V_GAP));
            ey += V_GAP;
            events.push(BoxItem {
                x,
                y: ey,
                w: col_w,
                h,
                lines: lines.clone(),
                color: flat.color,
            });
            ey += h;
        }
        max_bottom = max_bottom.max(ey);
    }

    Layout {
        width: MARGIN * 2.0 + flats.len() as f32 * pitch - COL_GAP,
        height: max_bottom + MARGIN,
        line_h,
        title,
        bands,
        periods,
        events,
        connectors,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::timeline::parser::parse;

    fn lay(src: &str) -> Layout {
        layout(&parse(src).unwrap(), &Options::default())
    }

    #[test]
    fn カラムは等幅で等間隔に並ぶ() {
        let l = lay("timeline\n2020 : a\n2021 : b\n2022 : c\n");
        assert_eq!(l.periods.len(), 3);
        let w = l.periods[0].w;
        assert!(l.periods.iter().all(|p| p.w == w), "等幅");
        let pitch = l.periods[1].x - l.periods[0].x;
        assert!(
            (l.periods[2].x - l.periods[1].x - pitch).abs() < 0.01,
            "等間隔"
        );
    }

    #[test]
    fn セクション帯は自セクションの期間を覆う() {
        let l = lay("timeline\nsection A\n2020 : a\n2021 : b\nsection B\n2022 : c\n");
        assert_eq!(l.bands.len(), 2);
        // A の帯は期間 0..2 のカラム範囲
        let band_a = &l.bands[0];
        assert!((band_a.x - l.periods[0].x).abs() < 0.01);
        assert!(
            (band_a.x + band_a.w - (l.periods[1].x + l.periods[1].w)).abs() < 0.01,
            "帯の右端 = 2 本目の期間箱の右端"
        );
        // B の帯は期間 2 のカラムから始まる
        assert!((l.bands[1].x - l.periods[2].x).abs() < 0.01);
    }

    #[test]
    fn イベントは期間の下に縦積みされる() {
        let l = lay("timeline\n2020 : a : b : c\n");
        assert_eq!(l.events.len(), 3);
        assert!(l.events[0].y > l.periods[0].y + l.periods[0].h);
        assert!(l.events[1].y > l.events[0].y + l.events[0].h - 0.01);
        assert!(l.events[2].y > l.events[1].y + l.events[1].h - 0.01);
        // 接続線は箱の数だけ（期間→e1, e1→e2, e2→e3）
        assert_eq!(l.connectors.len(), 3);
    }

    #[test]
    fn 長文イベントは折り返されカラム幅が抑えられる() {
        let long = "とても長いイベントの説明文がここに入りますが折返しで幅は抑えられます";
        let l = lay(&format!("timeline\n2020 : {long}\n"));
        assert!(l.events[0].lines.len() > 1, "折返しが起きる");
        // WRAP_W + パディング + 若干の禁則はみ出しに収まる
        assert!(
            l.periods[0].w < WRAP_W + 2.0 * BOX_PAD_X + 20.0,
            "w={}",
            l.periods[0].w
        );
    }

    #[test]
    fn 暗黙セクションのみなら帯は無く色は期間番号() {
        let l = lay("timeline\n2020 : a\n2021 : b\n");
        assert!(l.bands.is_empty());
        assert_eq!(l.periods[0].color, 0);
        assert_eq!(l.periods[1].color, 1);
    }
}
