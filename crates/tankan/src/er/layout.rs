//! erDiagram のレイアウト（layered エンジン共用。方向は TB 固定）。
//! エンティティ = 列幅を実測したテーブル、リレーション = クロウズフット付きエッジ

use crate::Options;
use crate::common::geom;
use crate::common::layered::{self, LayeredConfig, LayeredEdge, LayeredNode, Size};
use crate::common::path::{offset_amount, offset_polyline};
use crate::common::style::Style;
use crate::common::text::{max_width, text_width};
use crate::er::model::{Cardinality, ErDiagram};

const TITLE_PAD_Y: f32 = 8.0;
const ROW_PAD_Y: f32 = 5.0;
const COL_PAD_X: f32 = 10.0;
const MIN_ENTITY_W: f32 = 100.0;
const LABEL_PAD: f32 = 4.0;
const SELF_LOOP_W: f32 = 50.0;

pub(crate) struct Layout {
    pub width: f32,
    pub height: f32,
    pub line_h: f32,
    pub title: Option<Vec<String>>,
    pub entities: Vec<EntityBox>,
    pub relations: Vec<RelationPath>,
}

pub(crate) struct EntityBox {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub title: String,
    pub title_h: f32,
    pub row_h: f32,
    /// (型, 名前, キー, コメント) の列テキスト
    pub rows: Vec<[String; 4]>,
    /// 列幅（型・名前・キー・コメント。コメント列は無ければ 0）
    pub col_w: [f32; 4],
    /// 解決済みインラインスタイル（無ければ None）
    pub style: Option<Style>,
}

pub(crate) struct RelationPath {
    pub points: Vec<(f32, f32)>,
    pub from_card: Cardinality,
    pub to_card: Cardinality,
    pub identifying: bool,
    pub label: Vec<String>,
    pub label_at: Option<(f32, f32)>,
}

pub(crate) fn layout(diagram: &ErDiagram, options: &Options) -> Layout {
    let fs = options.font_size;
    let line_h = fs * 1.4;
    let title_h = line_h + 2.0 * TITLE_PAD_Y;
    let row_h = line_h + 2.0 * ROW_PAD_Y;

    // エンティティのテーブル寸法
    let mut boxes: Vec<EntityBox> = diagram
        .entities
        .iter()
        .map(|e| {
            let rows: Vec<[String; 4]> = e
                .attributes
                .iter()
                .map(|a| {
                    [
                        a.type_name.clone(),
                        a.name.clone(),
                        a.keys.join(", "),
                        a.comment.clone().unwrap_or_default(),
                    ]
                })
                .collect();
            let mut col_w = [0.0f32; 4];
            for row in &rows {
                for (i, cell) in row.iter().enumerate() {
                    if !cell.is_empty() {
                        col_w[i] = col_w[i].max(text_width(cell, fs) + 2.0 * COL_PAD_X);
                    }
                }
            }
            let table_w: f32 = col_w.iter().sum();
            let w = table_w
                .max(text_width(&e.display, fs) + 2.0 * COL_PAD_X)
                .max(MIN_ENTITY_W);
            // 列幅の合計が w に届かないぶんは名前列へ足す
            if table_w > 0.0 && table_w < w {
                col_w[1] += w - table_w;
            }
            let h = title_h + rows.len() as f32 * row_h;
            EntityBox {
                x: 0.0,
                y: 0.0,
                w,
                h,
                title: e.display.clone(),
                title_h,
                row_h,
                rows,
                col_w,
                style: e.style.clone(),
            }
        })
        .collect();

    // layered へ（自己リレーションは個別処理）
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

    // リレーション（多重オフセット → 矩形クリップ）
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
        relations.push(RelationPath {
            points,
            from_card: r.from_card,
            to_card: r.to_card,
            identifying: r.identifying,
            label: r.label.clone(),
            label_at: route.label_at,
        });
    }

    // 自己リレーション（右側の C 字）
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
        relations.push(RelationPath {
            points: vec![
                (x, cy - 10.0),
                (x + ext, cy - 10.0),
                (x + ext, cy + 10.0),
                (x, cy + 10.0),
            ],
            from_card: r.from_card,
            to_card: r.to_card,
            identifying: r.identifying,
            label: r.label.clone(),
            label_at: (!r.label.is_empty()).then_some((x + ext + LABEL_PAD, cy)),
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
    }

    Layout {
        width,
        height,
        line_h,
        title: diagram.title.clone(),
        entities: boxes,
        relations,
    }
}

fn clip_ends(boxes: &[EntityBox], from: usize, to: usize, points: &mut [(f32, f32)]) {
    if points.len() < 2 {
        return;
    }
    let clip = |b: &EntityBox, toward: (f32, f32)| {
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
    use crate::er::parser::parse;

    fn lay(src: &str) -> super::Layout {
        layout(
            &parse(&format!("erDiagram\n{src}")).unwrap(),
            &Options::default(),
        )
    }

    #[test]
    fn 属性で高さと列幅が決まる() {
        let l =
            lay("A {\n  string name\n  int long_attribute_name FK \"コメント\"\n}\nB ||--o{ A : x");
        let a = &l.entities[0];
        assert_eq!(a.rows.len(), 2);
        assert!(a.h > a.title_h + a.row_h);
        assert!(a.col_w[3] > 0.0, "コメント列");
        // 列幅の合計 = テーブル幅
        let total: f32 = a.col_w.iter().sum();
        assert!((total - a.w).abs() < 0.5, "total={total} w={}", a.w);
    }

    #[test]
    fn リレーションはエンティティ境界でクリップされる() {
        let l = lay("A ||--o{ B : has");
        let r = &l.relations[0];
        let a = &l.entities[0];
        assert_eq!(r.points[0].1, a.y + a.h, "A の下辺から出る");
    }

    #[test]
    fn 自己リレーションは右に張り出す() {
        let l = lay("EMPLOYEE ||--o{ EMPLOYEE : manages");
        let e = &l.entities[0];
        assert!(l.relations[0].points.iter().all(|p| p.0 >= e.x + e.w));
        assert!(l.width > e.x + e.w + 40.0);
    }

    #[test]
    fn 多重リレーションは重ならない() {
        let l = lay("A ||--o{ B : x\nA ||--|| B : y");
        assert_ne!(l.relations[0].points[0], l.relations[1].points[0]);
    }
}
