//! 形状境界と線分の交点（エッジ端をノード形状の縁でクリップするため）。
//!
//! いずれも「中心 `(cx, cy)` から目標点 `(tx, ty)` へ向かう半直線」と
//! 形状境界の交点を返す。目標点が中心と一致する場合は中心を返す。

/// 矩形（w × h）の境界との交点
pub(crate) fn clip_rect(cx: f32, cy: f32, w: f32, h: f32, tx: f32, ty: f32) -> (f32, f32) {
    let (dx, dy) = (tx - cx, ty - cy);
    if dx == 0.0 && dy == 0.0 {
        return (cx, cy);
    }
    let sx = if dx != 0.0 {
        (w / 2.0) / dx.abs()
    } else {
        f32::INFINITY
    };
    let sy = if dy != 0.0 {
        (h / 2.0) / dy.abs()
    } else {
        f32::INFINITY
    };
    let s = sx.min(sy);
    (cx + dx * s, cy + dy * s)
}

/// 円（半径 r）の境界との交点
pub(crate) fn clip_circle(cx: f32, cy: f32, r: f32, tx: f32, ty: f32) -> (f32, f32) {
    let (dx, dy) = (tx - cx, ty - cy);
    let len = (dx * dx + dy * dy).sqrt();
    if len == 0.0 {
        return (cx, cy);
    }
    (cx + dx / len * r, cy + dy / len * r)
}

/// ひし形（対角線 w × h）の境界との交点
pub(crate) fn clip_diamond(cx: f32, cy: f32, w: f32, h: f32, tx: f32, ty: f32) -> (f32, f32) {
    let (dx, dy) = (tx - cx, ty - cy);
    // |dx|/(w/2) + |dy|/(h/2) = 1 となるスケール
    let denom = dx.abs() / (w / 2.0) + dy.abs() / (h / 2.0);
    if denom == 0.0 {
        return (cx, cy);
    }
    let s = 1.0 / denom;
    (cx + dx * s, cy + dy * s)
}

/// 楕円（半径 rx × ry）の境界との交点
pub(crate) fn clip_ellipse(cx: f32, cy: f32, rx: f32, ry: f32, tx: f32, ty: f32) -> (f32, f32) {
    let (dx, dy) = (tx - cx, ty - cy);
    let denom = ((dx / rx).powi(2) + (dy / ry).powi(2)).sqrt();
    if denom == 0.0 {
        return (cx, cy);
    }
    (cx + dx / denom, cy + dy / denom)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 矩形クリップは辺上に乗る() {
        // 真右
        assert_eq!(clip_rect(0.0, 0.0, 100.0, 40.0, 200.0, 0.0), (50.0, 0.0));
        // 真下
        assert_eq!(clip_rect(0.0, 0.0, 100.0, 40.0, 0.0, 100.0), (0.0, 20.0));
        // 斜め（縦が先に当たる）
        let (x, y) = clip_rect(0.0, 0.0, 100.0, 40.0, 30.0, 30.0);
        assert_eq!(y, 20.0);
        assert_eq!(x, 20.0);
        // 目標 = 中心
        assert_eq!(clip_rect(5.0, 5.0, 10.0, 10.0, 5.0, 5.0), (5.0, 5.0));
    }

    #[test]
    fn 円クリップは半径上に乗る() {
        let (x, y) = clip_circle(0.0, 0.0, 10.0, 30.0, 40.0);
        assert!((x * x + y * y - 100.0).abs() < 0.001);
    }

    #[test]
    fn ひし形クリップは境界式を満たす() {
        let (x, y) = clip_diamond(0.0, 0.0, 100.0, 60.0, 80.0, 80.0);
        let on_boundary = x.abs() / 50.0 + y.abs() / 30.0;
        assert!((on_boundary - 1.0).abs() < 0.001, "{on_boundary}");
    }

    #[test]
    fn 楕円クリップは境界式を満たす() {
        let (x, y) = clip_ellipse(0.0, 0.0, 50.0, 30.0, 70.0, 10.0);
        let on_boundary = (x / 50.0).powi(2) + (y / 30.0).powi(2);
        assert!((on_boundary - 1.0).abs() < 0.001, "{on_boundary}");
    }
}
