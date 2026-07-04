//! ポリライン → 角丸 SVG パス文字列（エッジ描画の共通部品）。
//!
//! 曲がり角を 2 次ベジェ（Q）で丸める。dagre/mermaid の basis 曲線の
//! 簡易代替として十分な見た目で、決定的・依存なし。

use crate::common::svg::fmt_num;

/// 経由点列を角丸ポリラインのパス `d` にする（`radius` は角の丸み）
pub(crate) fn rounded_polyline(points: &[(f32, f32)], radius: f32) -> String {
    if points.is_empty() {
        return String::new();
    }
    let mut d = format!("M {},{}", fmt_num(points[0].0), fmt_num(points[0].1));
    if points.len() == 1 {
        return d;
    }

    for i in 1..points.len() - 1 {
        let (px, py) = points[i - 1];
        let (cx, cy) = points[i];
        let (nx, ny) = points[i + 1];

        // 角の前後を radius ぶん短縮（セグメント半分を上限）
        let r_in = radius.min(dist(px, py, cx, cy) / 2.0);
        let r_out = radius.min(dist(cx, cy, nx, ny) / 2.0);
        let (inx, iny) = toward(cx, cy, px, py, r_in);
        let (outx, outy) = toward(cx, cy, nx, ny, r_out);

        d.push_str(&format!(" L {},{}", fmt_num(inx), fmt_num(iny)));
        d.push_str(&format!(
            " Q {},{} {},{}",
            fmt_num(cx),
            fmt_num(cy),
            fmt_num(outx),
            fmt_num(outy)
        ));
    }
    let last = points[points.len() - 1];
    d.push_str(&format!(" L {},{}", fmt_num(last.0), fmt_num(last.1)));
    d
}

/// k 本目の多重エッジのオフセット量（+7, -7, +14, -14, …）
pub(crate) fn offset_amount(k: usize) -> f32 {
    const STEP: f32 = 7.0;
    let amount = k.div_ceil(2) as f32 * STEP;
    if k % 2 == 1 { amount } else { -amount }
}

/// ポリライン全体を始点→終点方向の法線側へ平行移動する（多重エッジの分離用）
pub(crate) fn offset_polyline(points: &mut [(f32, f32)], amount: f32) {
    if points.len() < 2 {
        return;
    }
    let (x1, y1) = points[0];
    let (x2, y2) = points[points.len() - 1];
    let (dx, dy) = (x2 - x1, y2 - y1);
    let len = (dx * dx + dy * dy).sqrt();
    if len == 0.0 {
        return;
    }
    let (nx, ny) = (-dy / len * amount, dx / len * amount);
    for p in points.iter_mut() {
        p.0 += nx;
        p.1 += ny;
    }
}

fn dist(x1: f32, y1: f32, x2: f32, y2: f32) -> f32 {
    ((x2 - x1).powi(2) + (y2 - y1).powi(2)).sqrt()
}

/// `(fx, fy)` から `(tx, ty)` 方向へ `len` 進んだ点
fn toward(fx: f32, fy: f32, tx: f32, ty: f32, len: f32) -> (f32, f32) {
    let d = dist(fx, fy, tx, ty);
    if d == 0.0 {
        return (fx, fy);
    }
    (fx + (tx - fx) / d * len, fy + (ty - fy) / d * len)
}

#[cfg(test)]
mod tests {
    use super::rounded_polyline;

    #[test]
    fn 直線は_m_l_のみ() {
        let d = rounded_polyline(&[(0.0, 0.0), (100.0, 0.0)], 6.0);
        assert_eq!(d, "M 0,0 L 100,0");
    }

    #[test]
    fn 折れ点は_q_で丸まる() {
        let d = rounded_polyline(&[(0.0, 0.0), (50.0, 0.0), (50.0, 50.0)], 6.0);
        assert!(d.contains(" Q 50,0 "), "{d}");
        assert!(d.starts_with("M 0,0 L 44,0"), "{d}");
        assert!(d.ends_with("L 50,50"), "{d}");
    }

    #[test]
    fn 短いセグメントでも破綻しない() {
        let d = rounded_polyline(&[(0.0, 0.0), (4.0, 0.0), (4.0, 4.0)], 6.0);
        assert!(d.starts_with("M 0,0"), "{d}");
        assert!(!d.contains("NaN"), "{d}");
    }
}
