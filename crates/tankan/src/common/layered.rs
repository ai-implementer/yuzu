//! Sugiyama 法サブセットの layered layout エンジン（flowchart / state / ER で共用）。
//!
//! パイプライン: ①閉路除去（DFS で back edge 反転）→ ②層割当（Kahn + longest-path +
//! source 引き締め）→ ③ダミーノード挿入（多層跨ぎエッジの分解、エッジラベルはラベル
//! 寸法のダミー）→ ④交差削減（barycenter を上下 4 往復、最良順序を保存）→ ⑤座標決定
//! （median 希望位置への priority 改善スイープ）。
//!
//! - 座標系は常に「rank 軸 = +y / order 軸 = +x」（= TB）。方向変換（LR 等）は
//!   呼び出し側の責務（投入前に w/h 交換、出力後に座標 swap）
//! - 自己ループ（from == to）は入れないこと（呼び出し側で個別ルーティング）
//! - すべて Vec 添字ベースで決定的（HashMap のイテレーション順に依存しない）

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Size {
    pub w: f32,
    pub h: f32,
}

pub(crate) struct LayeredNode {
    pub size: Size,
}

pub(crate) struct LayeredEdge {
    pub from: usize,
    pub to: usize,
    /// rank 差の最小値（既定 1。flowchart の `--->` は 2）
    pub minlen: u32,
    /// エッジラベルの寸法（中間 rank にこの寸法のダミーを置く）
    pub label: Option<Size>,
}

pub(crate) struct LayeredConfig {
    /// 同一 rank 内のノード間隔（dagre nodeSpacing 相当）
    pub node_sep: f32,
    /// rank 間の間隔（dagre rankSpacing 相当）
    pub rank_sep: f32,
    /// 図全体の余白（dagre diagramPadding 相当）
    pub margin: f32,
}

impl Default for LayeredConfig {
    fn default() -> Self {
        Self {
            node_sep: 50.0,
            rank_sep: 50.0,
            margin: 20.0,
        }
    }
}

pub(crate) struct EdgeRoute {
    /// from 中心 → ダミー列 → to 中心（**常に元エッジの from→to の向き**。
    /// 閉路除去で反転されたエッジは内部で向きを戻してある）
    pub points: Vec<(f32, f32)>,
    /// エッジラベルの中心（`label` 付きエッジのみ Some）
    pub label_at: Option<(f32, f32)>,
    /// 閉路除去で反転されたか（参考情報。現状は消費者なし）
    #[allow(dead_code)]
    pub reversed: bool,
}

pub(crate) struct LayeredResult {
    pub width: f32,
    pub height: f32,
    /// 実ノードの中心座標（入力順）
    pub node_pos: Vec<(f32, f32)>,
    /// 入力エッジ順のルート
    pub edge_routes: Vec<EdgeRoute>,
}

/// 内部の作業ノード（実ノード＋ダミー）
struct WorkNode {
    size: Size,
    rank: usize,
    /// true ならダミー（エッジ経由点）
    virtual_node: bool,
}

/// 内部の作業エッジ（単位セグメント。rank 差は常に 1）
#[derive(Clone, Copy)]
struct Segment {
    from: usize,
    to: usize,
}

pub(crate) fn layout(
    nodes: &[LayeredNode],
    edges: &[LayeredEdge],
    cfg: &LayeredConfig,
) -> LayeredResult {
    let n_real = nodes.len();
    if n_real == 0 {
        return LayeredResult {
            width: cfg.margin * 2.0,
            height: cfg.margin * 2.0,
            node_pos: Vec::new(),
            edge_routes: Vec::new(),
        };
    }
    debug_assert!(
        edges.iter().all(|e| e.from != e.to),
        "自己ループは呼び出し側で除外すること"
    );

    // ---- ① 閉路除去 ----
    let reversed_flags = break_cycles(n_real, edges);
    // 作業用の向き（reversed なら from/to 交換）
    let dir: Vec<(usize, usize)> = edges
        .iter()
        .zip(&reversed_flags)
        .map(|(e, &rev)| if rev { (e.to, e.from) } else { (e.from, e.to) })
        .collect();

    // ---- ② 層割当 ----
    // エッジラベルがあれば rank を倍化して中間 rank を確保する（dagre 方式の簡略版）
    let factor: u32 = if edges.iter().any(|e| e.label.is_some()) {
        2
    } else {
        1
    };
    let minlens: Vec<u32> = edges.iter().map(|e| e.minlen.max(1) * factor).collect();
    let ranks_of = assign_ranks(n_real, &dir, &minlens);

    // ---- ③ ダミーノード挿入 ----
    let mut work: Vec<WorkNode> = nodes
        .iter()
        .enumerate()
        .map(|(i, n)| WorkNode {
            size: n.size,
            rank: ranks_of[i],
            virtual_node: false,
        })
        .collect();
    let mut segments: Vec<Segment> = Vec::new();
    // 元エッジ → 経由ノード列（from, ダミー…, to）とラベルノード
    let mut chains: Vec<Vec<usize>> = Vec::with_capacity(edges.len());
    let mut label_nodes: Vec<Option<usize>> = Vec::with_capacity(edges.len());

    for (i, edge) in edges.iter().enumerate() {
        let (from, to) = dir[i];
        let (r_from, r_to) = (ranks_of[from], ranks_of[to]);
        debug_assert!(r_to > r_from, "層割当後は必ず下向き");
        let span = r_to - r_from;
        // ラベルは中間 rank（偶数 span の中央）に置く
        let label_rank = edge.label.map(|_| r_from + span / 2);

        let mut chain = vec![from];
        let mut label_node = None;
        for rank in (r_from + 1)..r_to {
            let size = if Some(rank) == label_rank {
                edge.label.unwrap()
            } else {
                Size { w: 8.0, h: 1.0 }
            };
            let id = work.len();
            work.push(WorkNode {
                size,
                rank,
                virtual_node: true,
            });
            if Some(rank) == label_rank {
                label_node = Some(id);
            }
            chain.push(id);
        }
        chain.push(to);
        for pair in chain.windows(2) {
            segments.push(Segment {
                from: pair[0],
                to: pair[1],
            });
        }
        chains.push(chain);
        label_nodes.push(label_node);
    }

    // ---- ④ 交差削減 ----
    let max_rank = work.iter().map(|n| n.rank).max().unwrap_or(0);
    let mut orders = initial_orders(&work, &segments, max_rank);
    reduce_crossings(&mut orders, &work, &segments);

    // ---- ⑤ 座標決定 ----
    let xs = assign_x(&orders, &work, &segments, cfg);
    let ys = assign_y(&work, max_rank, cfg);

    // 平行移動（margin へ）
    let min_x = work
        .iter()
        .enumerate()
        .map(|(i, n)| xs[i] - n.size.w / 2.0)
        .fold(f32::INFINITY, f32::min);
    let max_x = work
        .iter()
        .enumerate()
        .map(|(i, n)| xs[i] + n.size.w / 2.0)
        .fold(f32::NEG_INFINITY, f32::max);
    let shift = cfg.margin - min_x;
    let width = (max_x - min_x) + cfg.margin * 2.0;
    let height = ys.1 + cfg.margin;

    let pos = |i: usize| (xs[i] + shift, ys.0[work[i].rank]);

    let node_pos: Vec<(f32, f32)> = (0..n_real).map(pos).collect();
    let edge_routes: Vec<EdgeRoute> = chains
        .iter()
        .enumerate()
        .map(|(i, chain)| {
            let mut points: Vec<(f32, f32)> = chain.iter().map(|&id| pos(id)).collect();
            // 反転エッジは元の from→to の向きへ戻す
            if reversed_flags[i] {
                points.reverse();
            }
            EdgeRoute {
                points,
                label_at: label_nodes[i].map(pos),
                reversed: reversed_flags[i],
            }
        })
        .collect();

    LayeredResult {
        width,
        height,
        node_pos,
        edge_routes,
    }
}

/// ① 添字順 DFS で back edge を検出し、反転フラグを返す
fn break_cycles(n: usize, edges: &[LayeredEdge]) -> Vec<bool> {
    let mut adj: Vec<Vec<usize>> = vec![Vec::new(); n];
    for (i, e) in edges.iter().enumerate() {
        adj[e.from].push(i);
    }
    let mut state = vec![0u8; n]; // 0=未訪問 1=探索中 2=完了
    let mut reversed = vec![false; edges.len()];

    fn dfs(
        u: usize,
        adj: &[Vec<usize>],
        edges: &[LayeredEdge],
        state: &mut [u8],
        reversed: &mut [bool],
    ) {
        state[u] = 1;
        for &ei in &adj[u] {
            let v = edges[ei].to;
            match state[v] {
                0 => dfs(v, adj, edges, state, reversed),
                1 => reversed[ei] = true, // back edge
                _ => {}
            }
        }
        state[u] = 2;
    }

    for u in 0..n {
        if state[u] == 0 {
            dfs(u, &adj, edges, &mut state, &mut reversed);
        }
    }
    reversed
}

/// ② Kahn 決定的トポ順 → longest-path → source 引き締め
fn assign_ranks(n: usize, dir: &[(usize, usize)], minlens: &[u32]) -> Vec<usize> {
    let mut indeg = vec![0usize; n];
    let mut out: Vec<Vec<usize>> = vec![Vec::new(); n];
    let mut inc: Vec<Vec<usize>> = vec![Vec::new(); n];
    for (i, &(f, t)) in dir.iter().enumerate() {
        indeg[t] += 1;
        out[f].push(i);
        inc[t].push(i);
    }

    let mut rank = vec![0usize; n];
    let mut queue: Vec<usize> = (0..n).filter(|&v| indeg[v] == 0).collect();
    let mut head = 0;
    while head < queue.len() {
        let u = queue[head];
        head += 1;
        for &ei in &out[u] {
            let v = dir[ei].1;
            rank[v] = rank[v].max(rank[u] + minlens[ei] as usize);
            indeg[v] -= 1;
            if indeg[v] == 0 {
                queue.push(v);
            }
        }
    }

    // source の引き締め（後続との差を最小 minlen に詰め、長エッジ化を防ぐ）
    for v in 0..n {
        if inc[v].is_empty() && !out[v].is_empty() {
            let tight = out[v]
                .iter()
                .map(|&ei| rank[dir[ei].1].saturating_sub(minlens[ei] as usize))
                .min()
                .unwrap_or(0);
            rank[v] = tight;
        }
    }
    rank
}

/// ④ 初期順序: source から DFS preorder で各 rank に到達順に積む
fn initial_orders(work: &[WorkNode], segments: &[Segment], max_rank: usize) -> Vec<Vec<usize>> {
    let n = work.len();
    let mut out: Vec<Vec<usize>> = vec![Vec::new(); n];
    let mut indeg = vec![0usize; n];
    for s in segments {
        out[s.from].push(s.to);
        indeg[s.to] += 1;
    }

    let mut orders: Vec<Vec<usize>> = vec![Vec::new(); max_rank + 1];
    let mut visited = vec![false; n];

    fn dfs(
        u: usize,
        out: &[Vec<usize>],
        work: &[WorkNode],
        visited: &mut [bool],
        orders: &mut [Vec<usize>],
    ) {
        if visited[u] {
            return;
        }
        visited[u] = true;
        orders[work[u].rank].push(u);
        for &v in &out[u] {
            dfs(v, out, work, visited, orders);
        }
    }

    // source（入次数 0）→ 残りの未訪問、いずれも添字順
    for (u, &d) in indeg.iter().enumerate() {
        if d == 0 {
            dfs(u, &out, work, &mut visited, &mut orders);
        }
    }
    for u in 0..n {
        dfs(u, &out, work, &mut visited, &mut orders);
    }
    orders
}

/// ④ barycenter スイープ（上下 4 往復、最良順序を保存）
fn reduce_crossings(orders: &mut Vec<Vec<usize>>, work: &[WorkNode], segments: &[Segment]) {
    let n = work.len();
    let mut up_nb: Vec<Vec<usize>> = vec![Vec::new(); n]; // rank が 1 小さい隣接
    let mut down_nb: Vec<Vec<usize>> = vec![Vec::new(); n];
    for s in segments {
        down_nb[s.from].push(s.to);
        up_nb[s.to].push(s.from);
    }

    let mut best = orders.clone();
    let mut best_crossings = count_crossings(orders, segments, work);

    for _ in 0..4 {
        sweep(orders, &up_nb, false); // 下向き（上の rank を固定）
        sweep(orders, &down_nb, true); // 上向き
        let crossings = count_crossings(orders, segments, work);
        if crossings < best_crossings {
            best_crossings = crossings;
            best = orders.clone();
        }
    }
    *orders = best;
}

/// 1 方向のスイープ。`fixed_nb[v]` = 固定側の隣接ノード
fn sweep(orders: &mut [Vec<usize>], fixed_nb: &[Vec<usize>], upward: bool) {
    let mut pos = vec![0usize; fixed_nb.len()];
    let range: Vec<usize> = if upward {
        (0..orders.len().saturating_sub(1)).rev().collect()
    } else {
        (1..orders.len()).collect()
    };
    // pos を全 rank 分初期化
    for rank in orders.iter() {
        for (i, &v) in rank.iter().enumerate() {
            pos[v] = i;
        }
    }
    for r in range {
        let rank = &mut orders[r];
        let mut keyed: Vec<(f32, usize, usize)> = rank
            .iter()
            .enumerate()
            .map(|(i, &v)| {
                let nb = &fixed_nb[v];
                let bary = if nb.is_empty() {
                    i as f32
                } else {
                    nb.iter().map(|&u| pos[u] as f32).sum::<f32>() / nb.len() as f32
                };
                (bary, i, v)
            })
            .collect();
        // (barycenter, 元順位) のタプル比較で全順序化（f32 sort の不定性排除）
        keyed.sort_by(|a, b| {
            a.0.partial_cmp(&b.0)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(a.1.cmp(&b.1))
        });
        for (i, &(_, _, v)) in keyed.iter().enumerate() {
            rank[i] = v;
            pos[v] = i;
        }
    }
}

/// 隣接 rank ペアごとの転倒数の合計（O(E²) で十分な規模）
fn count_crossings(orders: &[Vec<usize>], segments: &[Segment], work: &[WorkNode]) -> usize {
    let n = work.len();
    let mut pos = vec![0usize; n];
    for rank in orders {
        for (i, &v) in rank.iter().enumerate() {
            pos[v] = i;
        }
    }
    let mut total = 0;
    for r in 0..orders.len().saturating_sub(1) {
        // rank r → r+1 のセグメント（順位ペア）
        let pairs: Vec<(usize, usize)> = segments
            .iter()
            .filter(|s| work[s.from].rank == r)
            .map(|s| (pos[s.from], pos[s.to]))
            .collect();
        for i in 0..pairs.len() {
            for j in (i + 1)..pairs.len() {
                let (a, b) = (pairs[i], pairs[j]);
                if (a.0 < b.0 && a.1 > b.1) || (a.0 > b.0 && a.1 < b.1) {
                    total += 1;
                }
            }
        }
    }
    total
}

/// ⑤ x 座標: 左詰め初期化 → median 希望位置への priority 改善スイープ
fn assign_x(
    orders: &[Vec<usize>],
    work: &[WorkNode],
    segments: &[Segment],
    cfg: &LayeredConfig,
) -> Vec<f32> {
    let n = work.len();
    let mut up_nb: Vec<Vec<usize>> = vec![Vec::new(); n];
    let mut down_nb: Vec<Vec<usize>> = vec![Vec::new(); n];
    for s in segments {
        down_nb[s.from].push(s.to);
        up_nb[s.to].push(s.from);
    }
    // 優先度: ダミー > 実ノード（次数が大きいほど高い）
    let priority: Vec<usize> = (0..n)
        .map(|v| {
            if work[v].virtual_node {
                usize::MAX
            } else {
                up_nb[v].len() + down_nb[v].len()
            }
        })
        .collect();

    // 左詰め初期化
    let mut xs = vec![0.0f32; n];
    for rank in orders {
        let mut cursor = 0.0f32;
        for &v in rank {
            xs[v] = cursor + work[v].size.w / 2.0;
            cursor += work[v].size.w + cfg.node_sep;
        }
    }

    // 改善スイープ（下→上を 3 往復 = 6 回）
    for pass in 0..6 {
        let upward = pass % 2 == 1;
        let fixed_nb = if upward { &down_nb } else { &up_nb };
        let range: Vec<usize> = if upward {
            (0..orders.len()).rev().collect()
        } else {
            (0..orders.len()).collect()
        };
        for r in range {
            let rank = &orders[r];
            // 優先度降順に処理（同値は rank 内順位）
            let mut idx: Vec<usize> = (0..rank.len()).collect();
            idx.sort_by(|&a, &b| priority[rank[b]].cmp(&priority[rank[a]]).then(a.cmp(&b)));
            for &i in &idx {
                let v = rank[i];
                let nb = &fixed_nb[v];
                if nb.is_empty() {
                    continue;
                }
                let mut med: Vec<f32> = nb.iter().map(|&u| xs[u]).collect();
                med.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
                let desired = med[med.len() / 2];
                shift_to(v, i, desired, rank, &priority, work, &mut xs, cfg.node_sep);
            }
        }
    }
    xs
}

/// ノード v を desired へ寄せる。自分より優先度の低い隣は最小間隔を保って押しのけ、
/// 優先度が同等以上の隣には阻まれる
#[allow(clippy::too_many_arguments)]
fn shift_to(
    v: usize,
    i: usize,
    desired: f32,
    rank: &[usize],
    priority: &[usize],
    work: &[WorkNode],
    xs: &mut [f32],
    sep: f32,
) {
    let gap = |a: usize, b: usize| (work[a].size.w + work[b].size.w) / 2.0 + sep;
    if desired > xs[v] {
        // 右へ: 高優先度の隣までの余地を計算
        let mut limit = f32::INFINITY;
        let mut acc = 0.0f32;
        let mut prev = v;
        for &u in &rank[i + 1..] {
            acc += gap(prev, u);
            if priority[u] >= priority[v] {
                limit = xs[u] - acc;
                break;
            }
            prev = u;
        }
        let target = desired.min(limit);
        if target > xs[v] {
            xs[v] = target;
            // 低優先度の隣を右へ押しのける
            let mut prev = v;
            for &u in &rank[i + 1..] {
                let min_x = xs[prev] + gap(prev, u);
                if xs[u] < min_x {
                    xs[u] = min_x;
                } else {
                    break;
                }
                prev = u;
            }
        }
    } else if desired < xs[v] {
        let mut limit = f32::NEG_INFINITY;
        let mut acc = 0.0f32;
        let mut prev = v;
        for &u in rank[..i].iter().rev() {
            acc += gap(prev, u);
            if priority[u] >= priority[v] {
                limit = xs[u] + acc;
                break;
            }
            prev = u;
        }
        let target = desired.max(limit);
        if target < xs[v] {
            xs[v] = target;
            let mut prev = v;
            for &u in rank[..i].iter().rev() {
                let max_x = xs[prev] - gap(prev, u);
                if xs[u] > max_x {
                    xs[u] = max_x;
                } else {
                    break;
                }
                prev = u;
            }
        }
    }
}

/// ⑤ y 座標: rank ごとの中心 y と全体の下端
fn assign_y(work: &[WorkNode], max_rank: usize, cfg: &LayeredConfig) -> (Vec<f32>, f32) {
    let mut heights = vec![0.0f32; max_rank + 1];
    for node in work {
        heights[node.rank] = heights[node.rank].max(node.size.h);
    }
    let mut centers = vec![0.0f32; max_rank + 1];
    let mut cursor = cfg.margin;
    for r in 0..=max_rank {
        centers[r] = cursor + heights[r] / 2.0;
        cursor += heights[r] + cfg.rank_sep;
    }
    (centers, cursor - cfg.rank_sep)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(w: f32, h: f32) -> LayeredNode {
        LayeredNode {
            size: Size { w, h },
        }
    }

    fn edge(from: usize, to: usize) -> LayeredEdge {
        LayeredEdge {
            from,
            to,
            minlen: 1,
            label: None,
        }
    }

    fn simple_nodes(n: usize) -> Vec<LayeredNode> {
        (0..n).map(|_| node(100.0, 40.0)).collect()
    }

    #[test]
    fn 直列チェーンは_rank_が単調に下がる() {
        let nodes = simple_nodes(3);
        let edges = [edge(0, 1), edge(1, 2)];
        let r = layout(&nodes, &edges, &LayeredConfig::default());
        assert!(r.node_pos[0].1 < r.node_pos[1].1);
        assert!(r.node_pos[1].1 < r.node_pos[2].1);
        // 同一 x（一直線）
        assert_eq!(r.node_pos[0].0, r.node_pos[1].0);
    }

    #[test]
    fn minlen_で_rank_が離れる() {
        let nodes = simple_nodes(2);
        let short = layout(&nodes, &[edge(0, 1)], &LayeredConfig::default());
        let long = layout(
            &nodes,
            &[LayeredEdge {
                from: 0,
                to: 1,
                minlen: 3,
                label: None,
            }],
            &LayeredConfig::default(),
        );
        assert!(long.node_pos[1].1 > short.node_pos[1].1);
    }

    #[test]
    fn 閉路があっても完了し全エッジのルートが返る() {
        let nodes = simple_nodes(3);
        let edges = [edge(0, 1), edge(1, 2), edge(2, 0)];
        let r = layout(&nodes, &edges, &LayeredConfig::default());
        assert_eq!(r.edge_routes.len(), 3);
        assert_eq!(r.edge_routes.iter().filter(|e| e.reversed).count(), 1);
        // 反転エッジも points は元の from→to（= ノード 2 → ノード 0）
        let back = &r.edge_routes[2];
        assert_eq!(back.points.first().copied(), Some(r.node_pos[2]));
        assert_eq!(back.points.last().copied(), Some(r.node_pos[0]));
    }

    #[test]
    fn 多層跨ぎエッジは中間点を経由する() {
        // 0→1→2→3 と 0→3（3 rank 跨ぎ）
        let nodes = simple_nodes(4);
        let edges = [edge(0, 1), edge(1, 2), edge(2, 3), edge(0, 3)];
        let r = layout(&nodes, &edges, &LayeredConfig::default());
        assert_eq!(r.edge_routes[3].points.len(), 4, "from + ダミー2 + to");
    }

    #[test]
    fn source_の引き締め() {
        // 4 は 3 にだけ繋がる source。rank 0 に張り付かず rank[3]-1 に来る
        let nodes = simple_nodes(5);
        let edges = [edge(0, 1), edge(1, 2), edge(2, 3), edge(4, 3)];
        let r = layout(&nodes, &edges, &LayeredConfig::default());
        // 4 の y は 2 と同じ rank（= 3 の 1 つ上）
        assert_eq!(r.node_pos[4].1, r.node_pos[2].1);
    }

    #[test]
    fn 同一_rank_のノードは最小間隔を保つ() {
        // 0 → {1,2,3}（同一 rank に 3 ノード）
        let nodes = simple_nodes(4);
        let edges = [edge(0, 1), edge(0, 2), edge(0, 3)];
        let cfg = LayeredConfig::default();
        let r = layout(&nodes, &edges, &cfg);
        let mut xs = [r.node_pos[1].0, r.node_pos[2].0, r.node_pos[3].0];
        xs.sort_by(|a, b| a.partial_cmp(b).unwrap());
        for pair in xs.windows(2) {
            assert!(
                pair[1] - pair[0] >= 100.0 + cfg.node_sep - 0.01,
                "間隔不足: {xs:?}"
            );
        }
    }

    #[test]
    fn 交差が解消される() {
        // 二部グラフ: 0→5, 1→4, 2→3 (初期順序のままだと交差だらけ)
        let nodes = simple_nodes(6);
        let edges = [edge(0, 5), edge(1, 4), edge(2, 3)];
        let r = layout(&nodes, &edges, &LayeredConfig::default());
        // 下段の x 順が上段の x 順と同型なら交差 0
        let top = [r.node_pos[0].0, r.node_pos[1].0, r.node_pos[2].0];
        let bottom = [r.node_pos[5].0, r.node_pos[4].0, r.node_pos[3].0];
        let mut top_sorted: Vec<usize> = (0..3).collect();
        top_sorted.sort_by(|&a, &b| top[a].partial_cmp(&top[b]).unwrap());
        let mut bottom_sorted: Vec<usize> = (0..3).collect();
        bottom_sorted.sort_by(|&a, &b| bottom[a].partial_cmp(&bottom[b]).unwrap());
        assert_eq!(top_sorted, bottom_sorted, "端点の順序が同型 = 交差なし");
    }

    #[test]
    fn ラベル付きエッジは_label_at_を返す() {
        let nodes = simple_nodes(2);
        let edges = [LayeredEdge {
            from: 0,
            to: 1,
            minlen: 1,
            label: Some(Size { w: 60.0, h: 20.0 }),
        }];
        let r = layout(&nodes, &edges, &LayeredConfig::default());
        let at = r.edge_routes[0].label_at.expect("ラベル位置");
        assert!(
            at.1 > r.node_pos[0].1 && at.1 < r.node_pos[1].1,
            "中間 rank"
        );
    }

    #[test]
    fn 決定性_同一入力で完全一致() {
        let nodes = simple_nodes(7);
        let edges = [
            edge(0, 1),
            edge(0, 2),
            edge(1, 3),
            edge(2, 3),
            edge(3, 4),
            edge(4, 5),
            edge(5, 3), // 閉路
            edge(2, 6),
        ];
        let a = layout(&nodes, &edges, &LayeredConfig::default());
        let b = layout(&nodes, &edges, &LayeredConfig::default());
        assert_eq!(a.node_pos, b.node_pos);
        assert_eq!(a.width, b.width);
        for (x, y) in a.edge_routes.iter().zip(&b.edge_routes) {
            assert_eq!(x.points, y.points);
        }
    }

    #[test]
    fn 全ノードが図の境界内に収まる() {
        let nodes = simple_nodes(6);
        let edges = [
            edge(0, 1),
            edge(0, 2),
            edge(1, 3),
            edge(2, 3),
            edge(3, 4),
            edge(0, 5),
        ];
        let cfg = LayeredConfig::default();
        let r = layout(&nodes, &edges, &cfg);
        for (i, &(x, y)) in r.node_pos.iter().enumerate() {
            assert!(x - nodes[i].size.w / 2.0 >= 0.0, "node {i} left");
            assert!(x + nodes[i].size.w / 2.0 <= r.width, "node {i} right");
            assert!(y >= 0.0 && y <= r.height, "node {i} y");
        }
    }
}
