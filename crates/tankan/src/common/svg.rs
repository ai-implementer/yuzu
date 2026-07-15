//! SVG 文字列の組み立て。
//!
//! 座標は 0.5px 単位に丸め、固定小数 1 桁で出力する
//! （決定的な出力＝スナップショットテストの安定性のため）。

use std::fmt::Write;

use crate::common::text::escape_xml;

/// 数値を 0.5px 単位に丸めて "12.5" / "12" 形式で整形する（-0 は 0 に正規化）
pub(crate) fn fmt_num(value: f32) -> String {
    let rounded = (value * 2.0).round() / 2.0;
    let rounded = if rounded == 0.0 { 0.0 } else { rounded }; // -0.0 対策
    if rounded.fract() == 0.0 {
        format!("{}", rounded as i64)
    } else {
        format!("{rounded:.1}")
    }
}

/// インデント付きで SVG 要素を書き溜めるビルダ
pub(crate) struct SvgBuilder {
    out: String,
}

impl SvgBuilder {
    pub fn new() -> Self {
        Self { out: String::new() }
    }

    pub fn raw(&mut self, s: &str) {
        self.out.push_str(s);
        self.out.push('\n');
    }

    pub fn line(&mut self, class: &str, x1: f32, y1: f32, x2: f32, y2: f32, extra: &str) {
        let _ = writeln!(
            self.out,
            r#"<line class="{class}" x1="{}" y1="{}" x2="{}" y2="{}"{extra}/>"#,
            fmt_num(x1),
            fmt_num(y1),
            fmt_num(x2),
            fmt_num(y2),
        );
    }

    pub fn rect(&mut self, class: &str, x: f32, y: f32, w: f32, h: f32, extra: &str) {
        let _ = writeln!(
            self.out,
            r#"<rect class="{class}" x="{}" y="{}" width="{}" height="{}"{extra}/>"#,
            fmt_num(x),
            fmt_num(y),
            fmt_num(w),
            fmt_num(h),
        );
    }

    pub fn circle(&mut self, class: &str, cx: f32, cy: f32, r: f32) {
        self.circle_with(class, cx, cy, r, "");
    }

    pub fn circle_with(&mut self, class: &str, cx: f32, cy: f32, r: f32, extra: &str) {
        let _ = writeln!(
            self.out,
            r#"<circle class="{class}" cx="{}" cy="{}" r="{}"{extra}/>"#,
            fmt_num(cx),
            fmt_num(cy),
            fmt_num(r),
        );
    }

    pub fn path(&mut self, class: &str, d: &str, extra: &str) {
        let _ = writeln!(self.out, r#"<path class="{class}" d="{d}"{extra}/>"#);
    }

    pub fn polygon(&mut self, class: &str, points: &[(f32, f32)]) {
        self.polygon_with(class, points, "");
    }

    pub fn polygon_with(&mut self, class: &str, points: &[(f32, f32)], extra: &str) {
        let pts: Vec<String> = points
            .iter()
            .map(|&(x, y)| format!("{},{}", fmt_num(x), fmt_num(y)))
            .collect();
        let _ = writeln!(
            self.out,
            r#"<polygon class="{class}" points="{}"{extra}/>"#,
            pts.join(" ")
        );
    }

    /// 複数行テキスト。`anchor` は text-anchor 値。y は 1 行目のベースライン
    pub fn text_lines(
        &mut self,
        class: &str,
        x: f32,
        y: f32,
        line_height: f32,
        anchor: &str,
        lines: &[String],
    ) {
        self.text_lines_with(class, x, y, line_height, anchor, lines, "");
    }

    /// `text_lines` に `<text>` 要素へ付ける追加属性 `extra` を加えた変種
    #[allow(clippy::too_many_arguments)]
    pub fn text_lines_with(
        &mut self,
        class: &str,
        x: f32,
        y: f32,
        line_height: f32,
        anchor: &str,
        lines: &[String],
        extra: &str,
    ) {
        for (i, line) in lines.iter().enumerate() {
            let _ = writeln!(
                self.out,
                r#"<text class="{class}" x="{}" y="{}" text-anchor="{anchor}"{extra}>{}</text>"#,
                fmt_num(x),
                fmt_num(y + i as f32 * line_height),
                escape_xml(line),
            );
        }
    }

    pub fn finish(self) -> String {
        self.out
    }
}

#[cfg(test)]
mod tests {
    use super::fmt_num;

    #[test]
    fn 数値整形は決定的() {
        assert_eq!(fmt_num(12.0), "12");
        assert_eq!(fmt_num(12.25), "12.5");
        assert_eq!(fmt_num(12.24), "12");
        assert_eq!(fmt_num(-0.1), "0");
        assert_eq!(fmt_num(1.75), "2");
        assert_eq!(fmt_num(1.5), "1.5");
    }
}
