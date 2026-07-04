//! flowchart のレイアウト。
//!
//! subgraph は**再帰合成方式**: 各スコープ（ルート／subgraph）を独立に
//! layered レイアウトし、子 subgraph は「タイトル帯＋パディング付きの複合ノード」
//! として親のレイアウトに参加する。エッジは両端の LCA スコープで配線される。
//! 方向（TB/BT/LR/RL）はスコープごとにローカル座標で変換してから親へ返す。

use crate::Options;
use crate::common::geom;
use crate::common::layered::{self, LayeredConfig, LayeredEdge, LayeredNode, Size};
use crate::common::path::{offset_amount, offset_polyline};
use crate::common::text::max_width;
use crate::flowchart::model::{Direction, EdgeLine, EdgeTip, EndRef, FlowchartDiagram, NodeShape};

const NODE_PAD_X: f32 = 15.0;
const NODE_PAD_Y: f32 = 12.0;
const MIN_NODE_W: f32 = 40.0;
const MIN_NODE_H: f32 = 36.0;
const CLUSTER_PAD: f32 = 8.0;
const CLUSTER_TITLE_H: f32 = 24.0;
const LABEL_PAD: f32 = 4.0;
const SELF_LOOP_W: f32 = 40.0;

pub(crate) struct Layout {
    pub width: f32,
    pub height: f32,
    pub line_h: f32,
    pub title: Option<Vec<String>>,
    pub nodes: Vec<NodeBox>,
    /// 外側 → 内側の順（背景として先に描く）
    pub clusters: Vec<ClusterBox>,
    pub edges: Vec<EdgePath>,
}

pub(crate) struct NodeBox {
    pub shape: NodeShape,
    pub cx: f32,
    pub cy: f32,
    pub w: f32,
    pub h: f32,
    pub label: Vec<String>,
}

pub(crate) struct ClusterBox {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub title: Vec<String>,
    /// concurrency 領域（破線・タイトルなし）
    pub region: bool,
}

pub(crate) struct EdgePath {
    /// クリップ済みの経由点列（from → to）
    pub points: Vec<(f32, f32)>,
    pub line: EdgeLine,
    pub head: EdgeTip,
    pub tail: EdgeTip,
    pub label: Vec<String>,
    pub label_at: Option<(f32, f32)>,
    pub self_loop: bool,
}

/// (edge id, 経由点列, label_at)
type RouteInfo = (usize, Vec<(f32, f32)>, Option<(f32, f32)>);

/// スコープ内のローカルレイアウト結果（ローカル座標）
struct ScopeLayout {
    size: Size,
    /// (node id, cx, cy)
    nodes: Vec<(usize, f32, f32)>,
    /// (subgraph id, x, y, w, h)。自スコープ直下 → 子孫の順
    clusters: Vec<(usize, f32, f32, f32, f32)>,
    /// (edge id, 経由点列, label_at)
    routes: Vec<RouteInfo>,
}

pub(crate) fn layout(diagram: &FlowchartDiagram, options: &Options) -> Layout {
    let fs = options.font_size;
    let line_h = fs * 1.4;

    let node_sizes: Vec<Size> = diagram
        .nodes
        .iter()
        .map(|n| node_size(n.shape, &n.label, fs, line_h))
        .collect();

    // 各エッジの LCA スコープ（自己ループは個別処理）
    let mut self_loops: Vec<usize> = Vec::new();
    let mut edge_scope: Vec<Option<Option<usize>>> = vec![None; diagram.edges.len()];
    for (i, edge) in diagram.edges.iter().enumerate() {
        if edge.from == edge.to {
            self_loops.push(i);
        } else {
            edge_scope[i] = Some(lca_scope(diagram, edge.from, edge.to));
        }
    }

    let root = layout_scope(diagram, &node_sizes, &edge_scope, None, options);

    // 絶対座標（ルートのローカル座標がそのまま絶対）
    let mut abs_nodes = vec![(0.0f32, 0.0f32); diagram.nodes.len()];
    for &(id, cx, cy) in &root.nodes {
        abs_nodes[id] = (cx, cy);
    }
    let mut cluster_rect = vec![(0.0f32, 0.0f32, 0.0f32, 0.0f32); diagram.subgraphs.len()];
    let clusters_out: Vec<ClusterBox> = root
        .clusters
        .iter()
        .map(|&(sid, x, y, w, h)| {
            cluster_rect[sid] = (x, y, w, h);
            ClusterBox {
                x,
                y,
                w,
                h,
                title: diagram.subgraphs[sid].title.clone(),
                region: diagram.subgraphs[sid].region,
            }
        })
        .collect();

    // エッジ確定（端点差し替え → 多重オフセット → 形状クリップ）
    let mut edges_out: Vec<EdgePath> = Vec::new();
    let mut pair_count: Vec<((EndRef, EndRef), usize)> = Vec::new();
    for (edge_id, mut points, label_at) in root.routes {
        let edge = &diagram.edges[edge_id];
        if let EndRef::Node(n) = edge.from {
            points[0] = abs_nodes[n];
        }
        if let EndRef::Node(n) = edge.to {
            let last = points.len() - 1;
            points[last] = abs_nodes[n];
        }

        // 同一ペア間の多重エッジは法線方向へオフセット
        let key = (edge.from, edge.to);
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
        if k > 0 && points.len() >= 2 {
            offset_polyline(&mut points, offset_amount(k));
        }

        let points = clip_ends(
            diagram,
            &node_sizes,
            &abs_nodes,
            &cluster_rect,
            edge,
            points,
        );
        edges_out.push(EdgePath {
            points,
            line: edge.line,
            head: edge.head,
            tail: edge.tail,
            label: edge.label.clone(),
            label_at,
            self_loop: false,
        });
    }

    // 自己ループ（ノード右側の C 字。複数は拡径）
    let mut loop_count: Vec<(usize, usize)> = Vec::new();
    for &edge_id in &self_loops {
        let edge = &diagram.edges[edge_id];
        let EndRef::Node(n) = edge.from else { continue };
        let k = match loop_count.iter_mut().find(|(id, _)| *id == n) {
            Some((_, c)) => {
                *c += 1;
                *c - 1
            }
            None => {
                loop_count.push((n, 1));
                0
            }
        };
        let (cx, cy) = abs_nodes[n];
        let x = cx + node_sizes[n].w / 2.0;
        let ext = SELF_LOOP_W + k as f32 * 8.0;
        edges_out.push(EdgePath {
            points: vec![
                (x, cy - 8.0),
                (x + ext, cy - 8.0),
                (x + ext, cy + 8.0),
                (x, cy + 8.0),
            ],
            line: edge.line,
            head: edge.head,
            tail: edge.tail,
            label: edge.label.clone(),
            label_at: (!edge.label.is_empty()).then_some((x + ext + LABEL_PAD, cy)),
            self_loop: true,
        });
    }

    let nodes: Vec<NodeBox> = diagram
        .nodes
        .iter()
        .enumerate()
        .map(|(i, n)| NodeBox {
            shape: n.shape,
            cx: abs_nodes[i].0,
            cy: abs_nodes[i].1,
            w: node_sizes[i].w,
            h: node_sizes[i].h,
            label: n.label.clone(),
        })
        .collect();

    // 自己ループ・ラベルの張り出しを図の大きさへ反映
    let mut width = root.size.w;
    let mut height = root.size.h;
    for edge in &edges_out {
        for &(x, y) in &edge.points {
            width = width.max(x + 20.0);
            height = height.max(y + 20.0);
        }
        if let Some((lx, ly)) = edge.label_at {
            let lw = max_width(&edge.label, fs);
            width = width.max(lx + lw + 20.0);
            height = height.max(ly + 20.0);
        }
    }

    Layout {
        width,
        height,
        line_h,
        title: diagram.title.clone(),
        nodes,
        clusters: clusters_out,
        edges: edges_out,
    }
}

/// スコープ直下のエンティティ
enum Entity {
    Node(usize),
    Cluster(usize, ScopeLayout),
}

/// スコープ（None = ルート）を再帰レイアウトする
fn layout_scope(
    diagram: &FlowchartDiagram,
    node_sizes: &[Size],
    edge_scope: &[Option<Option<usize>>],
    scope: Option<usize>,
    options: &Options,
) -> ScopeLayout {
    let fs = options.font_size;
    let line_h = fs * 1.4;

    let mut entities: Vec<Entity> = Vec::new();
    for (i, n) in diagram.nodes.iter().enumerate() {
        if n.subgraph == scope {
            entities.push(Entity::Node(i));
        }
    }
    for (i, s) in diagram.subgraphs.iter().enumerate() {
        if s.parent == scope {
            let inner = layout_scope(diagram, node_sizes, edge_scope, Some(i), options);
            entities.push(Entity::Cluster(i, inner));
        }
    }

    let direction = scope
        .and_then(|s| diagram.subgraphs[s].direction)
        .unwrap_or(diagram.direction);
    let swap = matches!(direction, Direction::Lr | Direction::Rl);
    let maybe_swap = |s: Size| {
        if swap { Size { w: s.h, h: s.w } } else { s }
    };

    let l_nodes: Vec<LayeredNode> = entities
        .iter()
        .map(|e| LayeredNode {
            size: maybe_swap(entity_size(e, diagram, node_sizes, fs)),
        })
        .collect();

    // このスコープで配線するエッジ（端点を直下エンティティへ写像）
    let mut l_edges: Vec<LayeredEdge> = Vec::new();
    let mut edge_ids: Vec<usize> = Vec::new();
    for (i, edge) in diagram.edges.iter().enumerate() {
        if edge_scope[i] != Some(scope) {
            continue;
        }
        let (Some(f), Some(t)) = (
            entity_of(diagram, &entities, scope, edge.from),
            entity_of(diagram, &entities, scope, edge.to),
        ) else {
            continue;
        };
        if f == t {
            continue;
        }
        let label = (!edge.label.is_empty()).then(|| {
            maybe_swap(Size {
                w: max_width(&edge.label, fs) + 2.0 * LABEL_PAD,
                h: edge.label.len() as f32 * line_h + 2.0 * LABEL_PAD,
            })
        });
        l_edges.push(LayeredEdge {
            from: f,
            to: t,
            minlen: edge.minlen,
            label,
        });
        edge_ids.push(i);
    }

    let result = layered::layout(&l_nodes, &l_edges, &LayeredConfig::default());

    // 方向変換（ローカル座標で完結）
    let (sw, sh) = if swap {
        (result.height, result.width)
    } else {
        (result.width, result.height)
    };
    let tf = move |p: (f32, f32)| -> (f32, f32) {
        let (x, y) = if swap { (p.1, p.0) } else { p };
        match direction {
            Direction::Tb | Direction::Lr => (x, y),
            Direction::Bt => (x, sh - y),
            Direction::Rl => (sw - x, y),
        }
    };

    let mut nodes_out: Vec<(usize, f32, f32)> = Vec::new();
    let mut clusters_out: Vec<(usize, f32, f32, f32, f32)> = Vec::new();
    let mut routes: Vec<RouteInfo> = Vec::new();

    for (ei, entity) in entities.iter().enumerate() {
        let center = tf(result.node_pos[ei]);
        match entity {
            Entity::Node(i) => nodes_out.push((*i, center.0, center.1)),
            Entity::Cluster(sid, inner) => {
                let size = entity_size(entity, diagram, node_sizes, fs);
                let x = center.0 - size.w / 2.0;
                let y = center.1 - size.h / 2.0;
                clusters_out.push((*sid, x, y, size.w, size.h));
                // 内部レイアウトをタイトル帯の下へ埋め込む
                let ox = x + (size.w - inner.size.w) / 2.0;
                let oy = y + cluster_title_h(diagram, *sid) + CLUSTER_PAD;
                for &(id, cx, cy) in &inner.nodes {
                    nodes_out.push((id, cx + ox, cy + oy));
                }
                for &(cid, cx, cy, cw, ch) in &inner.clusters {
                    clusters_out.push((cid, cx + ox, cy + oy, cw, ch));
                }
                for (eid, pts, label_at) in &inner.routes {
                    routes.push((
                        *eid,
                        pts.iter().map(|&(px, py)| (px + ox, py + oy)).collect(),
                        label_at.map(|(lx, ly)| (lx + ox, ly + oy)),
                    ));
                }
            }
        }
    }

    for (route, &eid) in result.edge_routes.iter().zip(&edge_ids) {
        routes.push((
            eid,
            route.points.iter().map(|&p| tf(p)).collect(),
            route.label_at.map(tf),
        ));
    }

    ScopeLayout {
        size: Size { w: sw, h: sh },
        nodes: nodes_out,
        clusters: clusters_out,
        routes,
    }
}

/// エンティティの layered 投入サイズ（クラスタはタイトル帯＋パディング込み）
fn entity_size(entity: &Entity, diagram: &FlowchartDiagram, node_sizes: &[Size], fs: f32) -> Size {
    match entity {
        Entity::Node(i) => node_sizes[*i],
        Entity::Cluster(sid, inner) => {
            let title_w = max_width(&diagram.subgraphs[*sid].title, fs) + 2.0 * CLUSTER_PAD;
            Size {
                w: (inner.size.w + 2.0 * CLUSTER_PAD).max(title_w),
                h: inner.size.h + cluster_title_h(diagram, *sid) + 2.0 * CLUSTER_PAD,
            }
        }
    }
}

/// クラスタのタイトル帯の高さ（region はタイトルなしで薄く）
fn cluster_title_h(diagram: &FlowchartDiagram, sid: usize) -> f32 {
    if diagram.subgraphs[sid].region {
        6.0
    } else {
        CLUSTER_TITLE_H
    }
}

/// end を含む「scope 直下」のエンティティ添字
fn entity_of(
    diagram: &FlowchartDiagram,
    entities: &[Entity],
    scope: Option<usize>,
    end: EndRef,
) -> Option<usize> {
    let mut cur = end;
    loop {
        let parent = match cur {
            EndRef::Node(n) => diagram.nodes[n].subgraph,
            EndRef::Subgraph(s) => diagram.subgraphs[s].parent,
        };
        if parent == scope {
            return entities.iter().position(|e| match (e, cur) {
                (Entity::Node(i), EndRef::Node(n)) => *i == n,
                (Entity::Cluster(sid, _), EndRef::Subgraph(s)) => *sid == s,
                _ => false,
            });
        }
        cur = EndRef::Subgraph(parent?);
    }
}

/// end の所属スコープチェーン（内側 → 外側 → None）
fn scope_chain(diagram: &FlowchartDiagram, end: EndRef) -> Vec<Option<usize>> {
    let mut chain = Vec::new();
    let mut cur = match end {
        EndRef::Node(n) => diagram.nodes[n].subgraph,
        EndRef::Subgraph(s) => diagram.subgraphs[s].parent,
    };
    loop {
        chain.push(cur);
        match cur {
            Some(s) => cur = diagram.subgraphs[s].parent,
            None => break,
        }
    }
    chain
}

/// 両端の最内共通スコープ
fn lca_scope(diagram: &FlowchartDiagram, a: EndRef, b: EndRef) -> Option<usize> {
    let chain_a = scope_chain(diagram, a);
    let chain_b = scope_chain(diagram, b);
    for s in &chain_a {
        if chain_b.contains(s) {
            return *s;
        }
    }
    None
}

/// 形状に応じたノード寸法
fn node_size(shape: NodeShape, label: &[String], fs: f32, line_h: f32) -> Size {
    use NodeShape::*;
    let tw = max_width(label, fs);
    let th = label.len() as f32 * line_h;
    let base_w = (tw + 2.0 * NODE_PAD_X).max(MIN_NODE_W);
    let base_h = (th + 2.0 * NODE_PAD_Y).max(MIN_NODE_H);
    match shape {
        Rect | Round | Asymmetric => Size {
            w: base_w,
            h: base_h,
        },
        Subroutine => Size {
            w: base_w + 16.0,
            h: base_h,
        },
        Stadium => Size {
            w: base_w + base_h / 2.0,
            h: base_h,
        },
        Cylinder => Size {
            w: base_w.max(50.0),
            h: base_h + 14.0,
        },
        Circle | DoubleCircle => {
            let d = tw.max(th) + 30.0;
            Size { w: d, h: d }
        }
        Diamond => Size {
            w: (base_w * 1.6).max(60.0),
            h: (base_h * 1.6).max(50.0),
        },
        Hexagon => Size {
            w: base_w + base_h,
            h: base_h,
        },
        LeanRight | LeanLeft | TrapezoidBottom | TrapezoidTop => Size {
            w: base_w + base_h * 0.9,
            h: base_h,
        },
        StateStart => Size { w: 14.0, h: 14.0 },
        StateEnd => Size { w: 18.0, h: 18.0 },
        ForkBar(vertical) => {
            if vertical {
                Size { w: 8.0, h: 60.0 }
            } else {
                Size { w: 60.0, h: 8.0 }
            }
        }
        NoteBox => Size {
            w: tw + 16.0,
            h: th + 12.0,
        },
    }
}

/// エッジ両端を実体の形状境界でクリップする
fn clip_ends(
    diagram: &FlowchartDiagram,
    node_sizes: &[Size],
    abs_nodes: &[(f32, f32)],
    cluster_rect: &[(f32, f32, f32, f32)],
    edge: &crate::flowchart::model::Edge,
    mut points: Vec<(f32, f32)>,
) -> Vec<(f32, f32)> {
    if points.len() < 2 {
        return points;
    }
    let toward_start = points[1];
    let toward_end = points[points.len() - 2];
    points[0] = clip_end(
        diagram,
        node_sizes,
        abs_nodes,
        cluster_rect,
        edge.from,
        toward_start,
    );
    let last = points.len() - 1;
    points[last] = clip_end(
        diagram,
        node_sizes,
        abs_nodes,
        cluster_rect,
        edge.to,
        toward_end,
    );
    points
}

fn clip_end(
    diagram: &FlowchartDiagram,
    node_sizes: &[Size],
    abs_nodes: &[(f32, f32)],
    cluster_rect: &[(f32, f32, f32, f32)],
    end: EndRef,
    toward: (f32, f32),
) -> (f32, f32) {
    use NodeShape::*;
    match end {
        EndRef::Subgraph(s) => {
            let (x, y, w, h) = cluster_rect[s];
            geom::clip_rect(x + w / 2.0, y + h / 2.0, w, h, toward.0, toward.1)
        }
        EndRef::Node(n) => {
            let (cx, cy) = abs_nodes[n];
            let Size { w, h } = node_sizes[n];
            match diagram.nodes[n].shape {
                Circle | DoubleCircle | StateStart | StateEnd => {
                    geom::clip_circle(cx, cy, w / 2.0, toward.0, toward.1)
                }
                Diamond => geom::clip_diamond(cx, cy, w, h, toward.0, toward.1),
                Stadium => geom::clip_ellipse(cx, cy, w / 2.0, h / 2.0, toward.0, toward.1),
                _ => geom::clip_rect(cx, cy, w, h, toward.0, toward.1),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::layout;
    use crate::Options;
    use crate::flowchart::parser::parse;

    fn lay(src: &str) -> super::Layout {
        layout(&parse(src).unwrap(), &Options::default())
    }

    #[test]
    fn td_は上から下へ流れる() {
        let l = lay("flowchart TD\nA --> B --> C");
        assert!(l.nodes[0].cy < l.nodes[1].cy);
        assert!(l.nodes[1].cy < l.nodes[2].cy);
    }

    #[test]
    fn lr_は左から右へ流れる() {
        let l = lay("flowchart LR\nA --> B --> C");
        assert!(l.nodes[0].cx < l.nodes[1].cx);
        assert!(l.nodes[1].cx < l.nodes[2].cx);
        // 同一 y（一直線）
        assert_eq!(l.nodes[0].cy, l.nodes[1].cy);
    }

    #[test]
    fn bt_と_rl_は反転する() {
        let bt = lay("flowchart BT\nA --> B");
        assert!(bt.nodes[0].cy > bt.nodes[1].cy, "BT は下から上");
        let rl = lay("flowchart RL\nA --> B");
        assert!(rl.nodes[0].cx > rl.nodes[1].cx, "RL は右から左");
    }

    #[test]
    fn エッジ端はノード境界でクリップされる() {
        let l = lay("flowchart TD\nA --> B");
        let e = &l.edges[0];
        let a = &l.nodes[0];
        // 始点は A の下辺
        assert_eq!(e.points[0].1, a.cy + a.h / 2.0);
    }

    #[test]
    fn subgraph_はノードを包含する() {
        let l = lay("flowchart TD\nsubgraph G[グループ]\n  a --> b\nend\nx --> a");
        assert_eq!(l.clusters.len(), 1);
        let c = &l.clusters[0];
        for node in &l.nodes {
            if node.label == ["a"] || node.label == ["b"] {
                assert!(
                    node.cx - node.w / 2.0 >= c.x && node.cx + node.w / 2.0 <= c.x + c.w,
                    "ノードがクラスタ x 範囲内"
                );
                assert!(
                    node.cy - node.h / 2.0 >= c.y && node.cy + node.h / 2.0 <= c.y + c.h,
                    "ノードがクラスタ y 範囲内"
                );
            }
        }
    }

    #[test]
    fn ネスト_subgraph_は親が子を包含する() {
        let l = lay("flowchart TD\nsubgraph outer\nsubgraph inner\n  a\nend\nend");
        assert_eq!(l.clusters.len(), 2);
        let (o, i) = (&l.clusters[0], &l.clusters[1]);
        assert!(o.x < i.x && o.y < i.y);
        assert!(o.x + o.w > i.x + i.w && o.y + o.h > i.y + i.h);
    }

    #[test]
    fn 自己ループは右側に張り出す() {
        let l = lay("flowchart TD\nA --> A");
        let e = &l.edges[0];
        assert!(e.self_loop);
        let a = &l.nodes[0];
        assert!(e.points.iter().all(|p| p.0 >= a.cx));
        assert!(l.width > a.cx + a.w / 2.0 + 30.0);
    }

    #[test]
    fn 多重エッジは重ならない() {
        let l = lay("flowchart TD\nA --> B\nA --> B");
        assert_ne!(l.edges[0].points[0], l.edges[1].points[0]);
    }

    #[test]
    fn ラベル付きエッジは_label_at_を持つ() {
        let l = lay("flowchart TD\nA -->|はい| B");
        assert!(l.edges[0].label_at.is_some());
        assert_eq!(l.edges[0].label, ["はい"]);
    }

    #[test]
    fn 全ノードが図の範囲内() {
        let l = lay(
            "flowchart LR\nA[開始] --> B{判定}\nB -->|Yes| C((成功))\nB -->|No| D[/入出力/]\nC --> E\nD --> E",
        );
        for n in &l.nodes {
            assert!(n.cx - n.w / 2.0 >= 0.0, "{:?} left", n.label);
            assert!(n.cx + n.w / 2.0 <= l.width, "{:?} right", n.label);
            assert!(
                n.cy - n.h / 2.0 >= 0.0 && n.cy + n.h / 2.0 <= l.height,
                "{:?} y",
                n.label
            );
        }
    }
}
