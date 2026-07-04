//! gantt の定規レイアウト（日付 → x スケール、行 = タスク、セクション帯、軸目盛り）

use crate::Options;
use crate::common::date::{civil_from_days, days_from_civil, format_axis, weekday};
use crate::common::text::text_width;
use crate::gantt::model::{GanttDiagram, TaskTags, TickInterval};

const BAR_H: f32 = 24.0;
const BAR_GAP: f32 = 6.0;
const MARGIN: f32 = 20.0;
const AXIS_H: f32 = 28.0;
const TITLE_H: f32 = 30.0;
const TARGET_CHART_W: f32 = 920.0;

pub(crate) struct Layout {
    pub width: f32,
    pub height: f32,
    pub line_h: f32,
    pub title: Option<String>,
    /// セクション帯 (y, h, 名前, 交互フラグ)
    pub sections: Vec<(f32, f32, String, bool)>,
    pub bars: Vec<Bar>,
    /// 除外日の網掛け (x, w)
    pub excluded: Vec<(f32, f32)>,
    /// 目盛り (x, ラベル)
    pub ticks: Vec<(f32, String)>,
    /// チャート本体（グリッド線・網掛けの縦範囲）
    pub chart_top: f32,
    pub chart_bottom: f32,
    pub chart_left: f32,
    pub chart_right: f32,
}

pub(crate) struct Bar {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub name: String,
    pub tags: TaskTags,
    /// タスク名をバーの中に置けるか（置けなければ右横）
    pub name_inside: bool,
}

pub(crate) fn layout(diagram: &GanttDiagram, options: &Options) -> Layout {
    let fs = options.font_size;
    let line_h = fs * 1.4;

    let min_start = diagram
        .sections
        .iter()
        .flat_map(|s| s.tasks.iter().map(|t| t.start))
        .min()
        .unwrap_or(0)
        - 1;
    let max_end = diagram
        .sections
        .iter()
        .flat_map(|s| s.tasks.iter().map(|t| t.end))
        .max()
        .unwrap_or(1)
        + 1;
    let span = (max_end - min_start).max(1);
    let px_per_day = (TARGET_CHART_W / span as f32).clamp(4.0, 38.0);

    let section_w = diagram
        .sections
        .iter()
        .map(|s| text_width(&s.name, fs))
        .fold(0.0, f32::max);
    let chart_left = MARGIN + section_w + 16.0;
    let x_of = |day: i64| chart_left + (day - min_start) as f32 * px_per_day;
    let chart_right = x_of(max_end);

    let chart_top = MARGIN
        + if diagram.title.is_some() {
            TITLE_H
        } else {
            0.0
        };

    // 行の配置とセクション帯
    let mut bars: Vec<Bar> = Vec::new();
    let mut sections: Vec<(f32, f32, String, bool)> = Vec::new();
    let mut y = chart_top;
    for (si, section) in diagram.sections.iter().enumerate() {
        let band_top = y;
        for task in &section.tasks {
            let x = x_of(task.start);
            let w = ((task.end - task.start) as f32 * px_per_day).max(2.0);
            let name_inside = text_width(&task.name, fs) + 12.0 <= w && !task.tags.milestone;
            bars.push(Bar {
                x,
                y: y + BAR_GAP / 2.0,
                w,
                h: BAR_H,
                name: task.name.clone(),
                tags: task.tags,
                name_inside,
            });
            y += BAR_H + BAR_GAP;
        }
        sections.push((band_top, y - band_top, section.name.clone(), si % 2 == 1));
    }
    let chart_bottom = y;

    // 除外日の網掛け
    let excluded: Vec<(f32, f32)> = diagram
        .excluded_days
        .iter()
        .filter(|&&z| z >= min_start && z < max_end)
        .map(|&z| (x_of(z), px_per_day))
        .collect();

    // 目盛り（tickInterval 指定 or スパンから自動）
    let tick = diagram.tick.unwrap_or(if span <= 21 {
        TickInterval::Day(1)
    } else if span <= 120 {
        TickInterval::Week(1)
    } else {
        TickInterval::Month(1)
    });
    let default_format = match tick {
        TickInterval::Day(_) | TickInterval::Week(_) => "%m-%d",
        TickInterval::Month(_) => "%Y-%m",
    };
    let format = diagram.axis_format.as_deref().unwrap_or(default_format);
    let mut ticks: Vec<(f32, String)> = Vec::new();
    let mut day = match tick {
        // 週目盛りは月曜、月目盛りは 1 日に揃える
        TickInterval::Week(_) => {
            let mut d = min_start;
            while weekday(d) != 1 {
                d += 1;
            }
            d
        }
        TickInterval::Month(_) => {
            let (y0, m0, _) = civil_from_days(min_start);
            let (y1, m1) = if m0 == 12 { (y0 + 1, 1) } else { (y0, m0 + 1) };
            days_from_civil(y1, m1, 1)
        }
        TickInterval::Day(_) => min_start + 1,
    };
    while day < max_end {
        ticks.push((x_of(day), format_axis(day, format)));
        day = match tick {
            TickInterval::Day(n) => day + i64::from(n),
            TickInterval::Week(n) => day + 7 * i64::from(n),
            TickInterval::Month(n) => {
                let (mut y0, mut m0, _) = civil_from_days(day);
                for _ in 0..n {
                    if m0 == 12 {
                        y0 += 1;
                        m0 = 1;
                    } else {
                        m0 += 1;
                    }
                }
                days_from_civil(y0, m0, 1)
            }
        };
    }

    // 右横に出すタスク名の張り出し
    let mut width = chart_right + MARGIN;
    for bar in &bars {
        if !bar.name_inside {
            width = width.max(bar.x + bar.w + 8.0 + text_width(&bar.name, fs) + MARGIN);
        }
    }

    Layout {
        width,
        height: chart_bottom + AXIS_H + MARGIN,
        line_h,
        title: diagram.title.clone(),
        sections,
        bars,
        excluded,
        ticks,
        chart_top,
        chart_bottom,
        chart_left,
        chart_right,
    }
}

#[cfg(test)]
mod tests {
    use super::layout;
    use crate::Options;
    use crate::gantt::parser::parse;

    fn lay(src: &str) -> super::Layout {
        layout(
            &parse(&format!("gantt\ndateFormat YYYY-MM-DD\n{src}")).unwrap(),
            &Options::default(),
        )
    }

    #[test]
    fn バーの幅は日数に比例する() {
        let l = lay("A : 2024-01-01, 2d\nB : 2024-01-01, 4d");
        assert!((l.bars[1].w - l.bars[0].w * 2.0).abs() < 0.5);
        // 同じ開始日なら同じ x
        assert_eq!(l.bars[0].x, l.bars[1].x);
    }

    #[test]
    fn セクション帯が行を覆う() {
        let l = lay("section S1\nA : 2024-01-01, 1d\nB : 1d\nsection S2\nC : 1d");
        assert_eq!(l.sections.len(), 2);
        let (y0, h0, ..) = l.sections[0];
        assert!(l.bars[0].y >= y0 && l.bars[1].y + l.bars[1].h <= y0 + h0 + 0.5);
        assert!(l.sections[1].3, "2 番目のセクションは交互フラグ");
    }

    #[test]
    fn 目盛りは期間内に生成される() {
        let l = lay("A : 2024-01-01, 10d");
        assert!(!l.ticks.is_empty());
        for (x, label) in &l.ticks {
            assert!(*x >= l.chart_left && *x <= l.chart_right);
            assert!(label.contains('-'), "{label}");
        }
    }

    #[test]
    fn 長い名前はバーの右横に出て幅に反映される() {
        let l = lay("とてもとてもとても長い名前のタスク : 2024-01-01, 1d");
        assert!(!l.bars[0].name_inside);
        assert!(l.width > l.bars[0].x + l.bars[0].w + 50.0);
    }
}
