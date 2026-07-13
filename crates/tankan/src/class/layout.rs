//! classDiagram のレイアウト（layered エンジン共用。方向は TB 固定）。
//! クラス = 3 区画のボックス、関係 = マーカー付きエッジ

use crate::Options;
use crate::class::model::{Class, ClassDiagram, Marker};
use crate::common::geom;
use crate::common::layered::{self, LayeredConfig, LayeredEdge, LayeredNode, Size};
use crate::common::path::{offset_amount, offset_polyline};
use crate::common::text::{max_width, text_width};

const TITLE_PAD_Y: f32 = 6.0;
const COMPART_PAD_Y: f32 = 6.0;
const PAD_X: f32 = 12.0;
const MIN_CLASS_W: f32 = 90.0;
/// 空区画（メンバーなしの属性/メソッド区画）の高さ
const EMPTY_H: f32 = 12.0;
const LABEL_PAD: f32 = 4.0;
const SELF_LOOP_W: f32 = 46.0;

pub(crate) struct Layout {
    pub width: f32,
    pub height: f32,
    pub line_h: f32,
    pub title: Option<Vec<String>>,
    pub classes: Vec<ClassBox>,
    pub relations: Vec<RelationPath>,
}

pub(crate) struct ClassBox {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    /// クラス名（ジェネリクス変換済み）
    pub name: String,
    /// アノテーション（`«interface»` のように整形済み。ある場合のみ）
    pub annotation: Option<String>,
    pub attributes: Vec<String>,
    pub methods: Vec<String>,
    /// タイトル区画の高さ
    pub title_h: f32,
    /// 属性区画の高さ（本体を描かない空クラスは 0）
    pub attr_h: f32,
    /// 属性・メソッド区画（仕切り線）を描くか
    pub has_body: bool,
}

pub(crate) struct RelationPath {
    pub points: Vec<(f32, f32)>,
    pub from_marker: Marker,
    pub to_marker: Marker,
    pub dashed: bool,
    pub label: Vec<String>,
    pub label_at: Option<(f32, f32)>,
    /// 多重度テキストと描画位置
    pub from_card: Option<(String, (f32, f32))>,
    pub to_card: Option<(String, (f32, f32))>,
}

pub(crate) fn layout(diagram: &ClassDiagram, options: &Options) -> Layout {
    let fs = options.font_size;
    let line_h = fs * 1.4;

    let mut boxes: Vec<ClassBox> = diagram
        .classes
        .iter()
        .map(|c| build_box(c, fs, line_h))
        .collect();

    // layered へ（自己関係は個別処理）
    let l_nodes: Vec<LayeredNode> = boxes
        .iter()
        .map(|b| LayeredNode {
            size: Size { w: b.w, h: b.h },
        })
        .collect();
    let mut l_edges: Vec<LayeredEdge> = Vec::new();
    let mut edge_ids: Vec<usize> = Vec::new();
    let mut self_loops: Vec<usize> = Vec::new();
    for (i, r) in diagram.relations.iter().enumerate() {
        if r.from == r.to {
            self_loops.push(i);
            continue;
        }
        let label = (!r.label.is_empty()).then(|| Size {
            w: max_width(&r.label, fs) + 2.0 * LABEL_PAD,
            h: r.label.len() as f32 * line_h + 2.0 * LABEL_PAD,
        });
        l_edges.push(LayeredEdge {
            from: r.from,
            to: r.to,
            minlen: 1,
            label,
        });
        edge_ids.push(i);
    }
    let result = layered::layout(&l_nodes, &l_edges, &LayeredConfig::default());

    for (i, b) in boxes.iter_mut().enumerate() {
        b.x = result.node_pos[i].0 - b.w / 2.0;
        b.y = result.node_pos[i].1 - b.h / 2.0;
    }

    // 関係（多重オフセット → 矩形クリップ）
    let mut relations: Vec<RelationPath> = Vec::new();
    let mut pair_count: Vec<((usize, usize), usize)> = Vec::new();
    for (route, &rid) in result.edge_routes.iter().zip(&edge_ids) {
        let r = &diagram.relations[rid];
        let mut points = route.points.clone();
        let key = (r.from, r.to);
        let k = match pair_count.iter_mut().find(|(p, _)| *p == key) {
            Some((_, c)) => {
                *c += 1;
                *c - 1
            }
            None => {
                pair_count.push((key, 1));
                0
            }
        };
        if k > 0 {
            offset_polyline(&mut points, offset_amount(k));
        }
        clip_ends(&boxes, r.from, r.to, &mut points);
        let from_card = r.from_card.clone().map(|t| (t, card_pos(&points, true)));
        let to_card = r.to_card.clone().map(|t| (t, card_pos(&points, false)));
        relations.push(RelationPath {
            points,
            from_marker: r.from_marker,
            to_marker: r.to_marker,
            dashed: r.dashed,
            label: r.label.clone(),
            label_at: route.label_at,
            from_card,
            to_card,
        });
    }

    // 自己関係（右側の C 字）
    let mut loop_count: Vec<(usize, usize)> = Vec::new();
    for &rid in &self_loops {
        let r = &diagram.relations[rid];
        let k = match loop_count.iter_mut().find(|(id, _)| *id == r.from) {
            Some((_, c)) => {
                *c += 1;
                *c - 1
            }
            None => {
                loop_count.push((r.from, 1));
                0
            }
        };
        let b = &boxes[r.from];
        let x = b.x + b.w;
        let cy = b.y + b.h / 2.0;
        let ext = SELF_LOOP_W + k as f32 * 10.0;
        let points = vec![
            (x, cy - 10.0),
            (x + ext, cy - 10.0),
            (x + ext, cy + 10.0),
            (x, cy + 10.0),
        ];
        let from_card = r.from_card.clone().map(|t| (t, card_pos(&points, true)));
        let to_card = r.to_card.clone().map(|t| (t, card_pos(&points, false)));
        relations.push(RelationPath {
            points,
            from_marker: r.from_marker,
            to_marker: r.to_marker,
            dashed: r.dashed,
            label: r.label.clone(),
            label_at: (!r.label.is_empty()).then_some((x + ext + LABEL_PAD, cy)),
            from_card,
            to_card,
        });
    }

    // 張り出し反映
    let mut width = result.width;
    let mut height = result.height;
    for rel in &relations {
        for &(x, y) in &rel.points {
            width = width.max(x + 20.0);
            height = height.max(y + 20.0);
        }
        if let Some((lx, _)) = rel.label_at {
            width = width.max(lx + max_width(&rel.label, fs) + 20.0);
        }
        for (t, (x, y)) in [&rel.from_card, &rel.to_card].into_iter().flatten() {
            width = width.max(x + text_width(t, fs) / 2.0 + 12.0);
            height = height.max(y + 20.0);
        }
    }

    Layout {
        width,
        height,
        line_h,
        title: diagram.title.clone(),
        classes: boxes,
        relations,
    }
}

fn build_box(c: &Class, fs: f32, line_h: f32) -> ClassBox {
    let annotation = c.annotation.as_ref().map(|a| format!("\u{ab}{a}\u{bb}"));
    let title_lines = if annotation.is_some() { 2 } else { 1 };
    let title_h = title_lines as f32 * line_h + 2.0 * TITLE_PAD_Y;

    let has_body = !c.attributes.is_empty() || !c.methods.is_empty();
    let compart_h = |lines: usize| {
        if !has_body {
            0.0
        } else if lines == 0 {
            EMPTY_H
        } else {
            lines as f32 * line_h + 2.0 * COMPART_PAD_Y
        }
    };
    let attr_h = compart_h(c.attributes.len());
    let method_h = compart_h(c.methods.len());

    let mut w = MIN_CLASS_W;
    if let Some(a) = &annotation {
        w = w.max(text_width(a, fs) + 2.0 * PAD_X);
    }
    w = w.max(text_width(&c.display, fs) + 2.0 * PAD_X);
    for m in c.attributes.iter().chain(&c.methods) {
        w = w.max(text_width(m, fs) + 2.0 * PAD_X);
    }

    ClassBox {
        x: 0.0,
        y: 0.0,
        w,
        h: title_h + attr_h + method_h,
        name: c.display.clone(),
        annotation,
        attributes: c.attributes.clone(),
        methods: c.methods.clone(),
        title_h,
        attr_h,
        has_body,
    }
}

/// 多重度テキストの描画位置（端点から線に沿って少し内側・脇へずらす）
fn card_pos(points: &[(f32, f32)], from: bool) -> (f32, f32) {
    let n = points.len();
    if n < 2 {
        return points.first().copied().unwrap_or((0.0, 0.0));
    }
    let (p0, p1) = if from {
        (points[0], points[1])
    } else {
        (points[n - 1], points[n - 2])
    };
    let (dx, dy) = (p1.0 - p0.0, p1.1 - p0.1);
    let len = (dx * dx + dy * dy).sqrt();
    if len == 0.0 {
        return p0;
    }
    const ALONG: f32 = 16.0;
    const SIDE: f32 = 8.0;
    let (ux, uy) = (dx / len, dy / len);
    let (px, py) = (-uy, ux); // 法線
    (p0.0 + ux * ALONG + px * SIDE, p0.1 + uy * ALONG + py * SIDE)
}

fn clip_ends(boxes: &[ClassBox], from: usize, to: usize, points: &mut [(f32, f32)]) {
    if points.len() < 2 {
        return;
    }
    let clip = |b: &ClassBox, toward: (f32, f32)| {
        geom::clip_rect(
            b.x + b.w / 2.0,
            b.y + b.h / 2.0,
            b.w,
            b.h,
            toward.0,
            toward.1,
        )
    };
    let toward_start = points[1];
    let toward_end = points[points.len() - 2];
    points[0] = clip(&boxes[from], toward_start);
    let last = points.len() - 1;
    points[last] = clip(&boxes[to], toward_end);
}

#[cfg(test)]
mod tests {
    use super::layout;
    use crate::Options;
    use crate::class::parser::parse;

    fn lay(src: &str) -> super::Layout {
        layout(
            &parse(&format!("classDiagram\n{src}")).unwrap(),
            &Options::default(),
        )
    }

    #[test]
    fn メンバー数で高さが決まる() {
        let l = lay("class A {\n  +int x\n  +int y\n  +run() void\n}\nB <|-- A");
        let a = &l.classes[0];
        assert!(a.has_body);
        // タイトル＋属性2行＋メソッド1行のぶん高さがある
        assert!(a.h > a.title_h + a.attr_h);
        assert!(a.attr_h > 0.0);
    }

    #[test]
    fn 空クラスは名前だけの高さ() {
        let l = lay("class Empty");
        let e = &l.classes[0];
        assert!(!e.has_body);
        assert_eq!(e.attr_h, 0.0);
        assert!((e.h - e.title_h).abs() < 0.01);
    }

    #[test]
    fn 関係はクラス境界でクリップされる() {
        let l = lay("A <|-- B");
        let r = &l.relations[0];
        let a = &l.classes[0];
        // A は上（from）で、線は A の下辺から出る
        assert!((r.points[0].1 - (a.y + a.h)).abs() < 0.5 || r.points[0].1 <= a.y + a.h + 0.5);
    }

    #[test]
    fn 多重度は位置を持つ() {
        let l = lay("A \"1\" --> \"*\" B");
        let r = &l.relations[0];
        assert!(r.from_card.is_some());
        assert!(r.to_card.is_some());
    }

    #[test]
    fn 自己関係は右に張り出す() {
        let l = lay("Node --> Node : next");
        let n = &l.classes[0];
        assert!(l.relations[0].points.iter().all(|p| p.0 >= n.x + n.w));
        assert!(l.width > n.x + n.w + 40.0);
    }
}
