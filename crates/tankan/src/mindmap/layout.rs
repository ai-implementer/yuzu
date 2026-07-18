//! mindmap のレイアウト（中央ルート左右振り分けの tidy tree）。
//!
//! - ルートの子を偶数 = 右・奇数 = 左に交互割当し、各サイドを再帰的に配置する
//!   （サブツリー高さを先に計算し、親を子スパンの垂直中央に置く）
//! - 左サイドは右向きに配置してから x をミラーする
//! - エッジは親と子の縁の中点を結ぶ水平 3 次ベジェ（d3 の linkHorizontal 相当）
//! - ブランチ色 = ルート直下の子の番号（パレット循環）を全子孫へ継承

use crate::Options;
use crate::common::text::{max_width, wrap_text};
use crate::mindmap::model::{MindmapDiagram, NodeShape};

const MARGIN: f32 = 16.0;
/// ノードテキストの折返し幅（px）
const WRAP_W: f32 = 180.0;
const PAD_X: f32 = 12.0;
const PAD_Y: f32 = 7.0;
/// 親子の水平間隔
const H_GAP: f32 = 36.0;
/// 兄弟サブツリーの垂直間隔
const V_GAP: f32 = 10.0;

pub(crate) struct Layout {
    pub width: f32,
    pub height: f32,
    pub line_h: f32,
    /// aria-label 用（frontmatter title 優先、無ければルートの 1 行目）
    pub label: String,
    pub nodes: Vec<PlacedNode>,
    /// (パス d, ブランチ番号)
    pub edges: Vec<(String, usize)>,
}

pub(crate) struct PlacedNode {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub lines: Vec<String>,
    pub shape: NodeShape,
    /// None = ルート
    pub branch: Option<usize>,
}

/// 配置作業用
struct Work {
    w: f32,
    h: f32,
    lines: Vec<String>,
    /// サブツリー全体の高さ（自身と子孫スパンの最大）
    span: f32,
    x: f32,
    cy: f32,
    branch: Option<usize>,
}

pub(crate) fn layout(diagram: &MindmapDiagram, options: &Options) -> Layout {
    let fs = options.font_size;
    let line_h = fs * 1.4;
    let nodes = &diagram.nodes;

    // 1. 各ノードの寸法（形状ごとの補正込み）
    let mut work: Vec<Work> = nodes
        .iter()
        .map(|n| {
            let lines = wrap_text(&n.text, fs, WRAP_W);
            let mut w = max_width(&lines, fs) + 2.0 * PAD_X;
            let mut h = lines.len() as f32 * line_h + 2.0 * PAD_Y;
            match n.shape {
                NodeShape::Circle | NodeShape::Bang => {
                    // 円はテキスト対角を覆う直径（min でも幅・高さの大きい方）
                    let d = (w * w * 0.6 + h * h).sqrt().max(w.max(h));
                    w = d;
                    h = d;
                }
                NodeShape::Cloud => {
                    // 楕円は四隅が欠けるぶん広げる
                    w *= 1.25;
                    h *= 1.45;
                }
                NodeShape::Hexagon => {
                    // 左右の頂点ぶん張り出す
                    w += h * 0.6;
                }
                _ => {}
            }
            Work {
                w,
                h,
                lines,
                span: 0.0,
                x: 0.0,
                cy: 0.0,
                branch: None,
            }
        })
        .collect();

    // 2. サブツリー高さ（post-order。再帰は木の深さぶんで浅い）
    fn calc_span(idx: usize, nodes: &[crate::mindmap::model::Node], work: &mut [Work]) {
        let children = nodes[idx].children.clone();
        let mut sum = 0.0;
        for &c in &children {
            calc_span(c, nodes, work);
            sum += work[c].span;
        }
        if !children.is_empty() {
            sum += (children.len() - 1) as f32 * V_GAP;
        }
        work[idx].span = work[idx].h.max(sum);
    }
    calc_span(0, nodes, &mut work);

    // 3. ルートの子を左右へ交互割当（偶数 = 右・奇数 = 左）し、ブランチ番号を継承
    let root_children = nodes[0].children.clone();
    let right: Vec<usize> = root_children.iter().copied().step_by(2).collect();
    let left: Vec<usize> = root_children.iter().copied().skip(1).step_by(2).collect();
    fn assign_branch(
        idx: usize,
        branch: usize,
        nodes: &[crate::mindmap::model::Node],
        work: &mut [Work],
    ) {
        work[idx].branch = Some(branch);
        for &c in nodes[idx].children.clone().iter() {
            assign_branch(c, branch, nodes, work);
        }
    }
    for (bi, &c) in root_children.iter().enumerate() {
        assign_branch(c, bi, nodes, &mut work);
    }

    // 4. 配置。まず右向きに置く関数（x = 左端、y_top = サブツリー上端）
    fn place(
        idx: usize,
        x: f32,
        y_top: f32,
        nodes: &[crate::mindmap::model::Node],
        work: &mut [Work],
    ) {
        let span = work[idx].span;
        work[idx].x = x;
        work[idx].cy = y_top + span / 2.0;
        let child_x = x + work[idx].w + H_GAP;
        let children = nodes[idx].children.clone();
        let children_span: f32 = children.iter().map(|&c| work[c].span).sum::<f32>()
            + children.len().saturating_sub(1) as f32 * V_GAP;
        let mut cursor = y_top + (span - children_span) / 2.0;
        for &c in &children {
            place(c, child_x, cursor, nodes, work);
            cursor += work[c].span + V_GAP;
        }
    }

    let side_span = |side: &[usize], work: &[Work]| -> f32 {
        side.iter().map(|&c| work[c].span).sum::<f32>()
            + side.len().saturating_sub(1) as f32 * V_GAP
    };
    let span_r = side_span(&right, &work);
    let span_l = side_span(&left, &work);
    let root_cy = span_r.max(span_l).max(work[0].h) / 2.0;

    // ルート（x=0 起点。あとで全体を平行移動する）
    work[0].x = 0.0;
    work[0].cy = root_cy;
    let root_w = work[0].w;

    // 右サイド
    let mut cursor = root_cy - span_r / 2.0;
    for &c in &right {
        place(c, root_w + H_GAP, cursor, nodes, &mut work);
        cursor += work[c].span + V_GAP;
    }
    // 左サイド: いったん右向きに置いてから x をミラー（x' = -(x + w) - H_GAP + root 左端）
    let mut cursor = root_cy - span_l / 2.0;
    let mut left_all: Vec<usize> = Vec::new();
    fn collect_desc(idx: usize, nodes: &[crate::mindmap::model::Node], out: &mut Vec<usize>) {
        out.push(idx);
        for &c in &nodes[idx].children {
            collect_desc(c, nodes, out);
        }
    }
    for &c in &left {
        place(c, root_w + H_GAP, cursor, nodes, &mut work);
        cursor += work[c].span + V_GAP;
        collect_desc(c, nodes, &mut left_all);
    }
    for &i in &left_all {
        // 軸 x = root_w / 2 で鏡映（右向き配置の x = root_w + H_GAP が
        // ミラー後に「右端 = root 左端 - H_GAP」となる）
        work[i].x = root_w - (work[i].x + work[i].w);
    }

    // 5. 全体を MARGIN へ平行移動
    let min_x = work.iter().map(|n| n.x).fold(f32::INFINITY, f32::min);
    let min_y = work
        .iter()
        .map(|n| n.cy - n.h / 2.0)
        .fold(f32::INFINITY, f32::min);
    let dx = MARGIN - min_x;
    let dy = MARGIN - min_y;
    for n in &mut work {
        n.x += dx;
        n.cy += dy;
    }
    let max_x = work.iter().map(|n| n.x + n.w).fold(0.0, f32::max);
    let max_y = work.iter().map(|n| n.cy + n.h / 2.0).fold(0.0, f32::max);

    // 6. エッジ（親子の縁の中点を結ぶ水平ベジェ。左右はミラー後の座標で判定）
    let mut edges = Vec::new();
    for (pi, node) in nodes.iter().enumerate() {
        for &ci in &node.children {
            let (p, c) = (&work[pi], &work[ci]);
            let (x1, x2) = if c.x >= p.x {
                (p.x + p.w, c.x) // 右向き: 親の右縁 → 子の左縁
            } else {
                (p.x, c.x + c.w) // 左向き: 親の左縁 → 子の右縁
            };
            let (y1, y2) = (p.cy, c.cy);
            let mx = (x1 + x2) / 2.0;
            let d = format!(
                "M {x1} {y1} C {mx} {y1} {mx} {y2} {x2} {y2}",
                x1 = crate::common::svg::fmt_num(x1),
                y1 = crate::common::svg::fmt_num(y1),
                mx = crate::common::svg::fmt_num(mx),
                x2 = crate::common::svg::fmt_num(x2),
                y2 = crate::common::svg::fmt_num(y2),
            );
            edges.push((d, work[ci].branch.unwrap_or(0)));
        }
    }

    let label = diagram
        .title
        .clone()
        .or_else(|| work[0].lines.first().cloned())
        .unwrap_or_else(|| "Mindmap".to_string());

    Layout {
        width: max_x + MARGIN,
        height: max_y + MARGIN,
        line_h,
        label,
        nodes: work
            .into_iter()
            .zip(nodes)
            .map(|(w, n)| PlacedNode {
                x: w.x,
                y: w.cy - w.h / 2.0,
                w: w.w,
                h: w.h,
                lines: w.lines,
                shape: n.shape,
                branch: w.branch,
            })
            .collect(),
        edges,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mindmap::parser::parse;

    fn lay(src: &str) -> Layout {
        layout(&parse(src).unwrap(), &Options::default())
    }

    #[test]
    fn ルートの子は左右に振り分けられる() {
        let l = lay("mindmap\nroot\n  A\n  B\n  C\n  D\n");
        let root = &l.nodes[0];
        // 偶数番目（A, C）は右、奇数番目（B, D）は左
        assert!(l.nodes[1].x > root.x, "A は右");
        assert!(l.nodes[3].x > root.x, "C は右");
        assert!(l.nodes[2].x + l.nodes[2].w <= root.x, "B は左");
        assert!(l.nodes[4].x + l.nodes[4].w <= root.x, "D は左");
    }

    #[test]
    fn 親は子スパンの垂直中央に来る() {
        let l = lay("mindmap\nroot\n  A\n    A1\n    A2\n    A3\n");
        let a = &l.nodes[1];
        let (a1, a3) = (&l.nodes[2], &l.nodes[4]);
        let mid = (a1.y + a1.h / 2.0 + a3.y + a3.h / 2.0) / 2.0;
        assert!((a.y + a.h / 2.0 - mid).abs() < 0.5, "A が A1..A3 の中央");
    }

    #[test]
    fn 同一サイドの兄弟サブツリーは重ならない() {
        // A(0 番目) と C(2 番目) はともに右サイド → 縦に分離している
        let l = lay("mindmap\nroot\n  A\n    A1\n    A2\n  B\n  C\n    C1\n");
        let a_bottom = l.nodes[3].y + l.nodes[3].h; // A2
        let c_top = l.nodes[5].y; // C
        assert!(
            c_top >= a_bottom - 0.01,
            "a_bottom={a_bottom} c_top={c_top}"
        );
    }

    #[test]
    fn 座標は全て_margin_以上() {
        let l = lay("mindmap\nroot((中心))\n  右\n  左\n    左の子\n");
        for n in &l.nodes {
            assert!(n.x >= MARGIN - 0.01 && n.y >= MARGIN - 0.01);
        }
        assert!(l.width > 0.0 && l.height > 0.0);
    }

    #[test]
    fn ブランチ番号はルート直下の番号を子孫へ継承する() {
        let l = lay("mindmap\nroot\n  A\n    A1\n  B\n");
        assert_eq!(l.nodes[0].branch, None, "ルートは無色");
        assert_eq!(l.nodes[1].branch, Some(0));
        assert_eq!(l.nodes[2].branch, Some(0), "A1 は A のブランチを継承");
        assert_eq!(l.nodes[3].branch, Some(1));
    }

    #[test]
    fn エッジは親子の数だけ生成される() {
        let l = lay("mindmap\nroot\n  A\n    A1\n  B\n");
        assert_eq!(l.edges.len(), 3);
        assert!(l.edges.iter().all(|(d, _)| d.starts_with("M ")));
    }
}
