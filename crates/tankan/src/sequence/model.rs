//! sequence 図の AST（パース結果）

/// 参加者。id は `SequenceDiagram::participants` の添字
#[derive(Debug)]
pub(crate) struct Participant {
    /// 表示名（`as` があればそちら）。`<br/>` で複数行
    pub display: Vec<String>,
    /// `actor` キーワードで宣言されたか（人型スタイル）
    pub is_actor: bool,
}

/// `box` によるグルーピング
#[derive(Debug)]
pub(crate) struct PBox {
    pub label: Vec<String>,
    /// CSS 色（`box Aqua グループ` の Aqua 等）
    pub color: Option<String>,
    /// メンバー参加者の添字
    pub members: Vec<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LineKind {
    Solid,
    Dotted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HeadKind {
    /// 矢じりなし（`->` / `-->`）
    None,
    /// 塗り三角（`->>` / `-->>`）
    Arrow,
    /// 両端塗り三角（`<<->>` / `<<-->>`）
    BothArrow,
    /// ×（`-x` / `--x`）
    Cross,
    /// 開き山形・非同期（`-)` / `--)`）
    Open,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NotePos {
    LeftOf,
    RightOf,
    Over,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum BlockKind {
    Loop,
    Alt,
    Opt,
    Par,
    Critical,
    Break,
    /// 背景色付き矩形（色は CSS 色文字列）
    Rect(String),
}

impl BlockKind {
    /// フレーム左上に表示するキーワードラベル
    pub fn label(&self) -> &'static str {
        match self {
            Self::Loop => "loop",
            Self::Alt => "alt",
            Self::Opt => "opt",
            Self::Par => "par",
            Self::Critical => "critical",
            Self::Break => "break",
            Self::Rect(_) => "",
        }
    }

    /// このブロック内で使える区切りキーワード
    pub fn separator_keyword(&self) -> Option<&'static str> {
        match self {
            Self::Alt => Some("else"),
            Self::Par => Some("and"),
            Self::Critical => Some("option"),
            _ => None,
        }
    }
}

/// 文書順のイベント列（ブロックは Begin/Separator/End で表現）
#[derive(Debug)]
pub(crate) enum Event {
    Message {
        from: usize,
        to: usize,
        line: LineKind,
        head: HeadKind,
        text: Vec<String>,
        /// `->>+` : 送信先を activate
        activate_to: bool,
        /// `->>-` : 送信元を deactivate
        deactivate_from: bool,
    },
    Note {
        pos: NotePos,
        a: usize,
        b: Option<usize>,
        text: Vec<String>,
    },
    Activate(usize),
    Deactivate(usize),
    BlockBegin {
        kind: BlockKind,
        label: Vec<String>,
    },
    BlockSeparator {
        label: Vec<String>,
    },
    BlockEnd,
    AutonumberOn {
        start: u32,
        step: u32,
    },
    AutonumberOff,
}

#[derive(Debug)]
pub(crate) struct SequenceDiagram {
    pub title: Option<Vec<String>>,
    pub participants: Vec<Participant>,
    pub boxes: Vec<PBox>,
    pub events: Vec<Event>,
}
