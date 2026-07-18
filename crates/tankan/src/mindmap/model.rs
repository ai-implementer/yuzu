//! mindmap の AST（Vec アリーナ + 添字。nodes[0] が必ずルート）

#[derive(Debug, Default)]
pub(crate) struct MindmapDiagram {
    /// nodes[0] = ルート
    pub nodes: Vec<Node>,
    /// frontmatter の title（aria-label 専用。描画はしない）
    pub title: Option<String>,
}

#[derive(Debug)]
pub(crate) struct Node {
    pub text: String,
    pub shape: NodeShape,
    pub children: Vec<usize>,
}

/// mermaid mindmap のノード形状。Bang / Cloud は近似形状で描く
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NodeShape {
    /// 無印（スタジアム風）
    Default,
    /// `[テキスト]`
    Square,
    /// `(テキスト)`
    Rounded,
    /// `((テキスト))`
    Circle,
    /// `))テキスト((`（円 + 破線枠の近似）
    Bang,
    /// `)テキスト(`（楕円の近似）
    Cloud,
    /// `{{テキスト}}`
    Hexagon,
}
