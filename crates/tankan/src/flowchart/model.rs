//! flowchart の AST

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum Direction {
    #[default]
    Tb,
    Bt,
    Lr,
    Rl,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NodeShape {
    /// `[text]`（裸 id も同じ）
    Rect,
    /// `(text)`
    Round,
    /// `([text])`
    Stadium,
    /// `[[text]]`
    Subroutine,
    /// `[(text)]`
    Cylinder,
    /// `((text))`
    Circle,
    /// `(((text)))`
    DoubleCircle,
    /// `>text]`
    Asymmetric,
    /// `{text}`
    Diamond,
    /// `{{text}}`
    Hexagon,
    /// `[/text/]`
    LeanRight,
    /// `[\text\]`
    LeanLeft,
    /// `[/text\]`（下辺が長い台形）
    TrapezoidBottom,
    /// `[\text/]`（上辺が長い台形）
    TrapezoidTop,
}

#[derive(Debug)]
pub(crate) struct Node {
    pub label: Vec<String>,
    pub shape: NodeShape,
    /// 所属 subgraph（直近の親）
    pub subgraph: Option<usize>,
}

#[derive(Debug)]
pub(crate) struct Subgraph {
    pub title: Vec<String>,
    pub parent: Option<usize>,
    pub direction: Option<Direction>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EndRef {
    Node(usize),
    Subgraph(usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EdgeLine {
    Solid,
    Dotted,
    Thick,
    /// `~~~`（描画しないが rank 制約は効く）
    Invisible,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EdgeTip {
    None,
    Arrow,
    Circle,
    Cross,
}

#[derive(Debug, Clone)]
pub(crate) struct Edge {
    pub from: EndRef,
    pub to: EndRef,
    pub line: EdgeLine,
    /// to 側の端点
    pub head: EdgeTip,
    /// from 側の端点（`<-->` / `o--o` 等の双方向用）
    pub tail: EdgeTip,
    pub minlen: u32,
    pub label: Vec<String>,
}

#[derive(Debug)]
pub(crate) struct FlowchartDiagram {
    pub direction: Direction,
    /// frontmatter title / accTitle（aria-label 用）
    pub title: Option<Vec<String>>,
    pub nodes: Vec<Node>,
    pub subgraphs: Vec<Subgraph>,
    pub edges: Vec<Edge>,
}
