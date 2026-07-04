//! flowchart の行指向パーサ（手書きスキャナ）。
//!
//! - `;` で文分割（引用符内は保護）、文ごとに先頭キーワードで dispatch
//! - チェーン文は「ノード群 → エッジ → ノード群 → …」を左から走査。
//!   `A & B --> C & D` はデカルト積に展開
//! - **エッジは最長一致**: ダッシュ/イコール列を貪欲に消費してから分類。
//!   単発の `-` はノード id の一部（`A-1` は id）。`o`/`x` は「直後にエッジ本体が
//!   続く」ときだけ端点（`A---oB` = 丸端点＋ノード B）
//! - スタイル系（style/classDef/class/linkStyle/click/`:::`）・`@{}`・markdown
//!   文字列・`fa:` は UnsupportedSyntax（フォールバック）

use std::collections::HashMap;

use crate::common::text::{decode_entities, split_br_lines};
use crate::error::Error;
use crate::flowchart::model::{
    Direction, Edge, EdgeLine, EdgeTip, EndRef, FlowchartDiagram, Node, NodeShape, Subgraph,
};
use crate::kind::trim_line;

pub(crate) fn parse(source: &str) -> Result<FlowchartDiagram, Error> {
    let mut p = Parser::default();

    let mut in_directive = false;
    let mut in_frontmatter = false;
    let mut seen_header = false;
    let mut first_content = true;

    for (idx, raw) in source.lines().enumerate() {
        let line_no = idx + 1;
        let line = trim_line(raw);
        if line.is_empty() {
            continue;
        }
        if in_directive {
            if line.ends_with("}%%") {
                in_directive = false;
            }
            continue;
        }
        if in_frontmatter {
            if line == "---" {
                in_frontmatter = false;
            } else if let Some(t) = line.strip_prefix("title:") {
                p.title = Some(split_text(t));
            }
            continue;
        }
        if first_content && line == "---" {
            in_frontmatter = true;
            first_content = false;
            continue;
        }
        first_content = false;
        if line.starts_with("%%{") {
            if !line.ends_with("}%%") {
                in_directive = true;
            }
            continue;
        }
        if line.starts_with("%%") {
            continue;
        }

        for statement in split_statements(line) {
            let statement = trim_line(&statement);
            if statement.is_empty() {
                continue;
            }
            if !seen_header {
                p.parse_header(statement, line_no)?;
                seen_header = true;
                continue;
            }
            p.statement(statement, line_no)?;
        }
    }

    if !p.subgraph_stack.is_empty() {
        return Err(Error::Parse {
            line: source.lines().count(),
            message: "閉じられていない subgraph があります（end 不足）".to_string(),
        });
    }

    p.resolve()
}

/// 未解決のエッジ（名前ベース。subgraph 宣言が後に来ても解決できるように遅延する）
struct PendingEdge {
    from: PendingEnd,
    to: PendingEnd,
    line: EdgeLine,
    head: EdgeTip,
    tail: EdgeTip,
    minlen: u32,
    label: Vec<String>,
}

enum PendingEnd {
    /// 形状付き等で即 intern 済み
    Resolved(EndRef),
    /// 裸 id（解決時に subgraph 名 → ノードの順で照合。ノード化時の所属先も保持）
    Name(String, Option<usize>),
}

#[derive(Default)]
struct Parser {
    title: Option<Vec<String>>,
    direction: Direction,
    nodes: Vec<Node>,
    node_index: HashMap<String, usize>,
    subgraphs: Vec<Subgraph>,
    subgraph_index: HashMap<String, usize>,
    subgraph_stack: Vec<usize>,
    pending_edges: Vec<PendingEdge>,
}

impl Parser {
    fn parse_header(&mut self, statement: &str, line_no: usize) -> Result<(), Error> {
        let mut it = statement.split_whitespace();
        let keyword = it.next().unwrap_or("");
        if keyword != "flowchart" && keyword != "graph" {
            return Err(Error::Parse {
                line: line_no,
                message: "flowchart/graph ヘッダがありません".to_string(),
            });
        }
        if let Some(dir) = it.next() {
            self.direction = parse_direction(dir, line_no)?;
        }
        Ok(())
    }

    fn statement(&mut self, statement: &str, line_no: usize) -> Result<(), Error> {
        // 未対応構文の検出（引用符の外にあるかは問わず安全側に倒す）
        for needle in ["@{", "\"`", ":::"] {
            if statement.contains(needle) {
                return Err(Error::UnsupportedSyntax {
                    line: line_no,
                    construct: needle.to_string(),
                });
            }
        }

        let keyword = statement.split_whitespace().next().unwrap_or("");
        let rest = trim_line(&statement[keyword.len().min(statement.len())..]);
        match keyword {
            "style" | "classDef" | "class" | "linkStyle" | "click" => {
                Err(Error::UnsupportedSyntax {
                    line: line_no,
                    construct: keyword.to_string(),
                })
            }
            "subgraph" => self.begin_subgraph(rest),
            "end" if rest.is_empty() => {
                if self.subgraph_stack.pop().is_none() {
                    return Err(Error::Parse {
                        line: line_no,
                        message: "対応する subgraph のない end です".to_string(),
                    });
                }
                Ok(())
            }
            "direction" => {
                let dir = parse_direction(rest, line_no)?;
                match self.subgraph_stack.last() {
                    Some(&id) => self.subgraphs[id].direction = Some(dir),
                    None => self.direction = dir,
                }
                Ok(())
            }
            k if k.starts_with("accTitle") => {
                if let Some(t) = statement.split_once(':').map(|(_, t)| t) {
                    self.title = Some(split_text(t));
                }
                Ok(())
            }
            k if k.starts_with("accDescr") => Ok(()),
            _ => self.chain(statement, line_no),
        }
    }

    fn begin_subgraph(&mut self, rest: &str) -> Result<(), Error> {
        // `id[タイトル]` / `id["タイトル"]` / `タイトル（空白可）`
        let (id, title) = match rest.split_once('[') {
            Some((id, t)) => {
                let t = t.strip_suffix(']').unwrap_or(t);
                let t = t.trim_matches('"');
                (trim_line(id).to_string(), t.to_string())
            }
            None => (rest.to_string(), rest.to_string()),
        };
        let sub_id = self.subgraphs.len();
        self.subgraphs.push(Subgraph {
            title: split_text(&title),
            parent: self.subgraph_stack.last().copied(),
            direction: None,
            region: false,
        });
        self.subgraph_index.entry(id).or_insert(sub_id);
        self.subgraph_stack.push(sub_id);
        Ok(())
    }

    /// チェーン文: ノード群 (エッジ ノード群)*
    fn chain(&mut self, statement: &str, line_no: usize) -> Result<(), Error> {
        let chars: Vec<char> = statement.chars().collect();
        let mut pos = 0usize;

        let mut group = self.node_group(&chars, &mut pos, line_no)?;
        loop {
            skip_ws(&chars, &mut pos);
            if pos >= chars.len() {
                break;
            }
            let edge = parse_edge(&chars, &mut pos, line_no)?;
            let next = self.node_group(&chars, &mut pos, line_no)?;
            for f in &group {
                for t in &next {
                    self.pending_edges.push(PendingEdge {
                        from: clone_end(f),
                        to: clone_end(t),
                        line: edge.line,
                        head: edge.head,
                        tail: edge.tail,
                        minlen: edge.minlen,
                        label: edge.label.clone(),
                    });
                }
            }
            group = next;
        }
        Ok(())
    }

    /// ノード群: node (`&` node)*
    fn node_group(
        &mut self,
        chars: &[char],
        pos: &mut usize,
        line_no: usize,
    ) -> Result<Vec<PendingEnd>, Error> {
        let mut group = vec![self.node(chars, pos, line_no)?];
        loop {
            skip_ws(chars, pos);
            if chars.get(*pos) == Some(&'&') {
                *pos += 1;
                group.push(self.node(chars, pos, line_no)?);
            } else {
                break;
            }
        }
        Ok(group)
    }

    /// 1 ノード: id ＋ 任意の形状ブラケット
    fn node(
        &mut self,
        chars: &[char],
        pos: &mut usize,
        line_no: usize,
    ) -> Result<PendingEnd, Error> {
        skip_ws(chars, pos);
        let start = *pos;
        while *pos < chars.len() {
            let c = chars[*pos];
            if c.is_whitespace() || c == '&' || c == '|' {
                break;
            }
            if is_shape_open(chars, *pos) || edge_lookahead(chars, *pos).is_some() {
                break;
            }
            *pos += 1;
        }
        let id: String = chars[start..*pos].iter().collect();
        if id.is_empty() {
            return Err(Error::Parse {
                line: line_no,
                message: "ノード id がありません".to_string(),
            });
        }
        if id == "end" {
            return Err(Error::Parse {
                line: line_no,
                message: "`end` はノード id に使えません（End 等に変えてください）".to_string(),
            });
        }

        // 形状ブラケット
        if is_shape_open(chars, *pos) {
            let (shape, text) = parse_shape(chars, pos, line_no)?;
            if text.contains("fa:") {
                return Err(Error::UnsupportedSyntax {
                    line: line_no,
                    construct: "fa:".to_string(),
                });
            }
            let node_id = self.intern_node(&id, Some(shape), Some(split_text(&text)));
            return Ok(PendingEnd::Resolved(EndRef::Node(node_id)));
        }

        // 裸 id は解決を遅延（後で宣言される subgraph かもしれない）
        Ok(PendingEnd::Name(id, self.subgraph_stack.last().copied()))
    }

    /// ノードを登録/更新して添字を返す（形状・ラベルは後勝ちで上書き）
    fn intern_node(
        &mut self,
        name: &str,
        shape: Option<NodeShape>,
        label: Option<Vec<String>>,
    ) -> usize {
        let id = match self.node_index.get(name) {
            Some(&id) => id,
            None => {
                let id = self.nodes.len();
                self.nodes.push(Node {
                    label: vec![name.to_string()],
                    shape: NodeShape::Rect,
                    subgraph: self.subgraph_stack.last().copied(),
                });
                self.node_index.insert(name.to_string(), id);
                id
            }
        };
        if let Some(shape) = shape {
            self.nodes[id].shape = shape;
        }
        if let Some(label) = label {
            if !label.is_empty() && !label[0].is_empty() {
                self.nodes[id].label = label;
            }
        }
        id
    }

    /// 裸 id を解決してエッジを確定する（subgraph 名を優先）
    fn resolve(mut self) -> Result<FlowchartDiagram, Error> {
        let mut edges = Vec::with_capacity(self.pending_edges.len());
        let pending = std::mem::take(&mut self.pending_edges);
        for pe in pending {
            let from = self.resolve_end(pe.from);
            let to = self.resolve_end(pe.to);
            edges.push(Edge {
                from,
                to,
                line: pe.line,
                head: pe.head,
                tail: pe.tail,
                minlen: pe.minlen,
                label: pe.label,
            });
        }
        Ok(FlowchartDiagram {
            direction: self.direction,
            title: self.title,
            nodes: self.nodes,
            subgraphs: self.subgraphs,
            edges,
        })
    }

    fn resolve_end(&mut self, end: PendingEnd) -> EndRef {
        match end {
            PendingEnd::Resolved(r) => r,
            PendingEnd::Name(name, ctx) => {
                if let Some(&sub) = self.subgraph_index.get(&name) {
                    return EndRef::Subgraph(sub);
                }
                if let Some(&id) = self.node_index.get(&name) {
                    return EndRef::Node(id);
                }
                // 文の解析時点の subgraph に所属させて新規登録
                let id = self.nodes.len();
                self.nodes.push(Node {
                    label: vec![name.clone()],
                    shape: NodeShape::Rect,
                    subgraph: ctx,
                });
                self.node_index.insert(name, id);
                EndRef::Node(id)
            }
        }
    }
}

fn clone_end(end: &PendingEnd) -> PendingEnd {
    match end {
        PendingEnd::Resolved(r) => PendingEnd::Resolved(*r),
        PendingEnd::Name(n, c) => PendingEnd::Name(n.clone(), *c),
    }
}

// ---- エッジのスキャン ----

struct ParsedEdge {
    line: EdgeLine,
    head: EdgeTip,
    tail: EdgeTip,
    minlen: u32,
    label: Vec<String>,
}

/// 位置 p からエッジが始まるか（始まるなら消費せず Some）。
/// ノード id の終端判定と parse_edge の両方で使う
fn edge_lookahead(chars: &[char], p: usize) -> Option<()> {
    let c = *chars.get(p)?;
    match c {
        '-' => {
            // `--` 以上、または `-.`（点線）
            match chars.get(p + 1) {
                Some('-') | Some('.') => Some(()),
                _ => None,
            }
        }
        '=' => (chars.get(p + 1) == Some(&'=')).then_some(()),
        '~' => (chars.get(p + 1) == Some(&'~') && chars.get(p + 2) == Some(&'~')).then_some(()),
        '<' | 'o' | 'x' => {
            // 端点の後にエッジ本体が続くときだけ
            let next = *chars.get(p + 1)?;
            (matches!(next, '-' | '=') && edge_lookahead(chars, p + 1).is_some()).then_some(())
        }
        _ => None,
    }
}

/// エッジを 1 本読む
fn parse_edge(chars: &[char], pos: &mut usize, line_no: usize) -> Result<ParsedEdge, Error> {
    if edge_lookahead(chars, *pos).is_none() {
        return Err(Error::Parse {
            line: line_no,
            message: "文として解釈できません（エッジが見つかりません）".to_string(),
        });
    }

    // 始端の端点
    let tail = match chars.get(*pos) {
        Some('<') => {
            *pos += 1;
            EdgeTip::Arrow
        }
        Some('o') if edge_lookahead(chars, *pos + 1).is_some() => {
            *pos += 1;
            EdgeTip::Circle
        }
        Some('x') if edge_lookahead(chars, *pos + 1).is_some() => {
            *pos += 1;
            EdgeTip::Cross
        }
        _ => EdgeTip::None,
    };

    // 本体
    let (line, opener_len, dots) = read_body(chars, pos, line_no)?;

    // 終端の端点
    let head = read_tip(chars, pos);

    // ラベルとエッジ長
    let mut label = Vec::new();
    let minlen;
    match line {
        EdgeLine::Invisible => {
            minlen = (opener_len as u32).saturating_sub(2).max(1);
        }
        EdgeLine::Dotted => {
            // 点線: `-.` ＋ dots ＋ `-`。dots 数がエッジ長。
            // 開きだけ（`-.` で本体が閉じていない）なら mid-text 形
            if dots == 0 {
                // `-. text .->` 形: 閉じトークンまでをラベルに
                let text = read_until_closing(chars, pos, line, line_no)?;
                label = split_text(&text);
                let (_, _, close_dots) = read_body_closing(chars, pos, line, line_no)?;
                minlen = close_dots.max(1);
            } else {
                minlen = dots;
            }
        }
        EdgeLine::Solid | EdgeLine::Thick => {
            let tipped = head != EdgeTip::None;
            if opener_len == 2 && !tipped {
                // `--`/`==` で止まっている → mid-text 形（`-- text -->`）
                let text = read_until_closing(chars, pos, line, line_no)?;
                label = split_text(&text);
                let (close_len, close_tipped, _) = read_body_closing(chars, pos, line, line_no)?;
                minlen = if close_tipped {
                    (close_len as u32).saturating_sub(1)
                } else {
                    (close_len as u32).saturating_sub(2)
                }
                .max(1);
            } else if tipped {
                minlen = (opener_len as u32).saturating_sub(1).max(1);
            } else {
                minlen = (opener_len as u32).saturating_sub(2).max(1);
            }
        }
    }

    // mid-text 形で閉じ側に端点があった場合を反映
    let head = if head == EdgeTip::None && !label.is_empty() {
        read_tip_prev(chars, *pos)
    } else {
        head
    };

    // `|text|` 形のラベル
    skip_ws(chars, pos);
    if chars.get(*pos) == Some(&'|') {
        *pos += 1;
        let start = *pos;
        while *pos < chars.len() && chars[*pos] != '|' {
            *pos += 1;
        }
        let text: String = chars[start..*pos].iter().collect();
        if chars.get(*pos) == Some(&'|') {
            *pos += 1;
        }
        label = split_text(trim_line(&text));
    }

    if line == EdgeLine::Invisible && (head != EdgeTip::None || tail != EdgeTip::None) {
        return Err(Error::Parse {
            line: line_no,
            message: "不可視リンク（~~~）に端点は付けられません".to_string(),
        });
    }
    if label.len() == 1 && label[0].is_empty() {
        label.clear();
    }
    if label.iter().any(|l| l.contains("fa:")) {
        return Err(Error::UnsupportedSyntax {
            line: line_no,
            construct: "fa:".to_string(),
        });
    }

    Ok(ParsedEdge {
        line,
        head,
        tail,
        minlen: minlen.max(1),
        label,
    })
}

/// 本体（ダッシュ/イコール/チルダ列）を貪欲に読む。
/// 戻り値: (線種, 連続長, 点線のドット数)
fn read_body(
    chars: &[char],
    pos: &mut usize,
    line_no: usize,
) -> Result<(EdgeLine, usize, u32), Error> {
    match chars.get(*pos) {
        Some('~') => {
            let mut n = 0;
            while chars.get(*pos) == Some(&'~') {
                *pos += 1;
                n += 1;
            }
            if n < 3 {
                return Err(Error::Parse {
                    line: line_no,
                    message: "不可視リンクは ~~~ 以上です".to_string(),
                });
            }
            Ok((EdgeLine::Invisible, n, 0))
        }
        Some('=') => {
            let mut n = 0;
            while chars.get(*pos) == Some(&'=') {
                *pos += 1;
                n += 1;
            }
            Ok((EdgeLine::Thick, n, 0))
        }
        Some('-') => {
            let mut dashes = 0;
            while chars.get(*pos) == Some(&'-') {
                *pos += 1;
                dashes += 1;
            }
            // 点線: `-` の後に `.`
            if dashes == 1 || chars.get(*pos) == Some(&'.') {
                let mut dots = 0u32;
                while chars.get(*pos) == Some(&'.') {
                    *pos += 1;
                    dots += 1;
                }
                if dots == 0 {
                    return Err(Error::Parse {
                        line: line_no,
                        message: "エッジとして解釈できません".to_string(),
                    });
                }
                // 閉じ側の `-`（`-.-` の最後）。無ければ mid-text 形（dots=0 で通知）
                if chars.get(*pos) == Some(&'-') {
                    while chars.get(*pos) == Some(&'-') {
                        *pos += 1;
                    }
                    Ok((EdgeLine::Dotted, 0, dots))
                } else {
                    Ok((EdgeLine::Dotted, 0, 0))
                }
            } else {
                Ok((EdgeLine::Solid, dashes, 0))
            }
        }
        _ => Err(Error::Parse {
            line: line_no,
            message: "エッジとして解釈できません".to_string(),
        }),
    }
}

/// mid-text 形の閉じトークンまで（テキスト部分）を読む
fn read_until_closing(
    chars: &[char],
    pos: &mut usize,
    line: EdgeLine,
    line_no: usize,
) -> Result<String, Error> {
    let start = *pos;
    while *pos < chars.len() {
        let closing = match line {
            EdgeLine::Solid => chars[*pos] == '-' && chars.get(*pos + 1) == Some(&'-'),
            EdgeLine::Thick => chars[*pos] == '=' && chars.get(*pos + 1) == Some(&'='),
            EdgeLine::Dotted => chars[*pos] == '.',
            EdgeLine::Invisible => false,
        };
        if closing {
            let text: String = chars[start..*pos].iter().collect();
            return Ok(trim_line(&text).to_string());
        }
        *pos += 1;
    }
    Err(Error::Parse {
        line: line_no,
        message: "エッジラベルが閉じられていません（`-- text -->` の形にしてください）".to_string(),
    })
}

/// mid-text 形の閉じトークン本体を読む。戻り値: (連続長, 端点があったか, 点線ドット数)
fn read_body_closing(
    chars: &[char],
    pos: &mut usize,
    line: EdgeLine,
    line_no: usize,
) -> Result<(usize, bool, u32), Error> {
    let mut n = 0usize;
    let mut dots = 0u32;
    match line {
        EdgeLine::Solid => {
            while chars.get(*pos) == Some(&'-') {
                *pos += 1;
                n += 1;
            }
        }
        EdgeLine::Thick => {
            while chars.get(*pos) == Some(&'=') {
                *pos += 1;
                n += 1;
            }
        }
        EdgeLine::Dotted => {
            while chars.get(*pos) == Some(&'.') {
                *pos += 1;
                dots += 1;
            }
            while chars.get(*pos) == Some(&'-') {
                *pos += 1;
                n += 1;
            }
        }
        EdgeLine::Invisible => {}
    }
    if n == 0 && dots == 0 {
        return Err(Error::Parse {
            line: line_no,
            message: "エッジラベルの閉じトークンがありません".to_string(),
        });
    }
    let tipped = read_tip(chars, pos) != EdgeTip::None;
    Ok((n, tipped, dots))
}

fn read_tip(chars: &[char], pos: &mut usize) -> EdgeTip {
    match chars.get(*pos) {
        Some('>') => {
            *pos += 1;
            EdgeTip::Arrow
        }
        Some('o') => {
            *pos += 1;
            EdgeTip::Circle
        }
        Some('x') => {
            *pos += 1;
            EdgeTip::Cross
        }
        _ => EdgeTip::None,
    }
}

/// 直前に読み終わった位置の 1 つ前の文字から端点を復元する（mid-text 閉じ用）
fn read_tip_prev(chars: &[char], pos: usize) -> EdgeTip {
    match pos.checked_sub(1).and_then(|p| chars.get(p)) {
        Some('>') => EdgeTip::Arrow,
        Some('o') => EdgeTip::Circle,
        Some('x') => EdgeTip::Cross,
        _ => EdgeTip::None,
    }
}

// ---- 形状のスキャン ----

fn is_shape_open(chars: &[char], p: usize) -> bool {
    matches!(chars.get(p), Some('[') | Some('(') | Some('{') | Some('>'))
}

/// 形状ブラケットを読み、(形状, 内側テキスト) を返す
fn parse_shape(
    chars: &[char],
    pos: &mut usize,
    line_no: usize,
) -> Result<(NodeShape, String), Error> {
    use NodeShape::*;
    // (開きトークン, 閉じ候補と対応形状)。開きは最長一致順
    let openers: &[(&str, &[(&str, NodeShape)])] = &[
        ("(((", &[(")))", DoubleCircle)]),
        ("([", &[("])", Stadium)]),
        ("[[", &[("]]", Subroutine)]),
        ("[(", &[(")]", Cylinder)]),
        ("((", &[("))", Circle)]),
        ("{{", &[("}}", Hexagon)]),
        ("[/", &[("/]", LeanRight), ("\\]", TrapezoidBottom)]),
        ("[\\", &[("\\]", LeanLeft), ("/]", TrapezoidTop)]),
        ("[", &[("]", Rect)]),
        ("(", &[(")", Round)]),
        ("{", &[("}", Diamond)]),
        (">", &[("]", Asymmetric)]),
    ];

    for (open, closers) in openers {
        if !starts_with(chars, *pos, open) {
            continue;
        }
        *pos += open.chars().count();

        // 引用符付きテキスト: `A["(x)"]`（閉じ引用符まで素通し）
        let text = if chars.get(*pos) == Some(&'"') {
            *pos += 1;
            let start = *pos;
            while *pos < chars.len() && chars[*pos] != '"' {
                *pos += 1;
            }
            let text: String = chars[start..*pos].iter().collect();
            if chars.get(*pos) == Some(&'"') {
                *pos += 1;
            }
            text
        } else {
            // 最初に現れた閉じ候補まで
            let start = *pos;
            let mut end = None;
            'scan: while *pos < chars.len() {
                for (close, _) in *closers {
                    if starts_with(chars, *pos, close) {
                        end = Some(*pos);
                        break 'scan;
                    }
                }
                *pos += 1;
            }
            let Some(end) = end else {
                return Err(Error::Parse {
                    line: line_no,
                    message: format!("ノード形状 `{open}` が閉じられていません"),
                });
            };
            chars[start..end].iter().collect()
        };

        // 閉じトークンで形状を確定
        for (close, shape) in *closers {
            if starts_with(chars, *pos, close) {
                *pos += close.chars().count();
                return Ok((*shape, trim_line(&text).to_string()));
            }
        }
        return Err(Error::Parse {
            line: line_no,
            message: format!("ノード形状 `{open}` の閉じトークンが不正です"),
        });
    }
    Err(Error::Parse {
        line: line_no,
        message: "ノード形状として解釈できません".to_string(),
    })
}

// ---- ユーティリティ ----

fn skip_ws(chars: &[char], pos: &mut usize) {
    while chars
        .get(*pos)
        .is_some_and(|c| c.is_whitespace() || *c == '\u{3000}')
    {
        *pos += 1;
    }
}

fn starts_with(chars: &[char], p: usize, needle: &str) -> bool {
    needle
        .chars()
        .enumerate()
        .all(|(i, c)| chars.get(p + i) == Some(&c))
}

fn split_text(text: &str) -> Vec<String> {
    split_br_lines(&decode_entities(trim_line(text)))
}

/// `;` で文に分割（引用符内は保護）
fn split_statements(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut in_quote = false;
    for c in line.chars() {
        match c {
            '"' => {
                in_quote = !in_quote;
                current.push(c);
            }
            ';' if !in_quote => {
                out.push(std::mem::take(&mut current));
            }
            _ => current.push(c),
        }
    }
    out.push(current);
    out
}

fn parse_direction(token: &str, line_no: usize) -> Result<Direction, Error> {
    match token {
        "TB" | "TD" => Ok(Direction::Tb),
        "BT" => Ok(Direction::Bt),
        "LR" => Ok(Direction::Lr),
        "RL" => Ok(Direction::Rl),
        other => Err(Error::Parse {
            line: line_no,
            message: format!("不明な方向指定: {other}"),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::parse;
    use crate::error::Error;
    use crate::flowchart::model::{Direction, EdgeLine, EdgeTip, EndRef, NodeShape};

    fn flow(body: &str) -> String {
        format!("flowchart TD\n{body}")
    }

    #[test]
    fn 全ノード形状をパースできる() {
        use NodeShape::*;
        let cases = [
            ("a[矩形]", Rect),
            ("b(角丸)", Round),
            ("c([スタジアム])", Stadium),
            ("d[[サブルーチン]]", Subroutine),
            ("e[(データベース)]", Cylinder),
            ("f((円))", Circle),
            ("g(((二重円)))", DoubleCircle),
            ("h>非対称]", Asymmetric),
            ("i{ひし形}", Diamond),
            ("j{{六角形}}", Hexagon),
            ("k[/平行四辺形/]", LeanRight),
            ("l[\\逆平行四辺形\\]", LeanLeft),
            ("m[/台形\\]", TrapezoidBottom),
            ("n[\\逆台形/]", TrapezoidTop),
        ];
        for (src, shape) in cases {
            let d = parse(&flow(src)).unwrap_or_else(|e| panic!("{src}: {e}"));
            assert_eq!(d.nodes[0].shape, shape, "{src}");
        }
    }

    #[test]
    fn 全エッジ種をパースできる() {
        let cases = [
            ("A --> B", EdgeLine::Solid, EdgeTip::Arrow, EdgeTip::None, 1),
            ("A --- B", EdgeLine::Solid, EdgeTip::None, EdgeTip::None, 1),
            (
                "A ---> B",
                EdgeLine::Solid,
                EdgeTip::Arrow,
                EdgeTip::None,
                2,
            ),
            ("A ---- B", EdgeLine::Solid, EdgeTip::None, EdgeTip::None, 2),
            (
                "A -.-> B",
                EdgeLine::Dotted,
                EdgeTip::Arrow,
                EdgeTip::None,
                1,
            ),
            ("A -.- B", EdgeLine::Dotted, EdgeTip::None, EdgeTip::None, 1),
            (
                "A -..-> B",
                EdgeLine::Dotted,
                EdgeTip::Arrow,
                EdgeTip::None,
                2,
            ),
            ("A ==> B", EdgeLine::Thick, EdgeTip::Arrow, EdgeTip::None, 1),
            ("A === B", EdgeLine::Thick, EdgeTip::None, EdgeTip::None, 1),
            (
                "A ~~~ B",
                EdgeLine::Invisible,
                EdgeTip::None,
                EdgeTip::None,
                1,
            ),
            (
                "A --o B",
                EdgeLine::Solid,
                EdgeTip::Circle,
                EdgeTip::None,
                1,
            ),
            ("A --x B", EdgeLine::Solid, EdgeTip::Cross, EdgeTip::None, 1),
            (
                "A <--> B",
                EdgeLine::Solid,
                EdgeTip::Arrow,
                EdgeTip::Arrow,
                1,
            ),
            (
                "A o--o B",
                EdgeLine::Solid,
                EdgeTip::Circle,
                EdgeTip::Circle,
                1,
            ),
            (
                "A x--x B",
                EdgeLine::Solid,
                EdgeTip::Cross,
                EdgeTip::Cross,
                1,
            ),
            (
                "A <==> B",
                EdgeLine::Thick,
                EdgeTip::Arrow,
                EdgeTip::Arrow,
                1,
            ),
        ];
        for (src, line, head, tail, minlen) in cases {
            let d = parse(&flow(src)).unwrap_or_else(|e| panic!("{src}: {e}"));
            let e = &d.edges[0];
            assert_eq!(
                (e.line, e.head, e.tail, e.minlen),
                (line, head, tail, minlen),
                "{src}"
            );
        }
    }

    #[test]
    fn ラベルの_2_形式() {
        let d = parse(&flow(
            "A -->|はい| B\nC -- いいえ --> D\nE -. 点線 .-> F\nG == 太線 ==> H",
        ))
        .unwrap();
        assert_eq!(d.edges[0].label, ["はい"]);
        assert_eq!(d.edges[1].label, ["いいえ"]);
        assert_eq!(d.edges[1].head, EdgeTip::Arrow);
        assert_eq!(d.edges[2].label, ["点線"]);
        assert_eq!(d.edges[2].line, EdgeLine::Dotted);
        assert_eq!(d.edges[3].label, ["太線"]);
        assert_eq!(d.edges[3].line, EdgeLine::Thick);
    }

    #[test]
    fn チェーンとアンパサンド() {
        let d = parse(&flow("A --> B --> C")).unwrap();
        assert_eq!(d.edges.len(), 2);
        let d = parse(&flow("A & B --> C & D")).unwrap();
        assert_eq!(d.edges.len(), 4, "デカルト積");
        assert_eq!(d.nodes.len(), 4);
    }

    #[test]
    fn ノード_id_の単発ハイフンと_ox_始まり() {
        // 単発の `-` は id の一部
        let d = parse(&flow("A-1 --> B-2")).unwrap();
        assert_eq!(d.nodes.len(), 2);
        assert_eq!(d.nodes[0].label, ["A-1"]);
        // `o`/`x` で始まるノード（スペース区切り）
        let d = parse(&flow("x2 --> ok")).unwrap();
        assert_eq!(d.nodes[0].label, ["x2"]);
        assert_eq!(d.nodes[1].label, ["ok"]);
        // `A---oB` は丸端点＋ノード B
        let d = parse(&flow("A---oB")).unwrap();
        assert_eq!(d.edges[0].head, EdgeTip::Circle);
        assert_eq!(d.nodes[1].label, ["B"]);
    }

    #[test]
    fn 引用符テキストとセミコロン分割() {
        let d = parse(&flow("A[\"括弧 (と) ; 記号\"] --> B; B --> C")).unwrap();
        assert_eq!(d.nodes[0].label, ["括弧 (と) ; 記号"]);
        assert_eq!(d.edges.len(), 2);
        // graph エイリアス＋ヘッダ後セミコロン
        let d = parse("graph LR;\n  A-->B;").unwrap();
        assert_eq!(d.direction, Direction::Lr);
        assert_eq!(d.edges.len(), 1);
    }

    #[test]
    fn subgraph_の宣言と参照() {
        let d = parse(&flow(
            "subgraph one[グループ1]\n  a1 --> a2\nend\nsubgraph two\n  b1\nend\none --> two\na1 --> b1",
        ))
        .unwrap();
        assert_eq!(d.subgraphs.len(), 2);
        assert_eq!(d.subgraphs[0].title, ["グループ1"]);
        // subgraph 内のノードは所属を持つ
        let a1 = d.nodes.iter().position(|n| n.label == ["a1"]).unwrap();
        assert_eq!(d.nodes[a1].subgraph, Some(0));
        // subgraph 名へのエッジは Subgraph 参照
        let e = d
            .edges
            .iter()
            .find(|e| e.from == EndRef::Subgraph(0))
            .expect("one --> two");
        assert_eq!(e.to, EndRef::Subgraph(1));
    }

    #[test]
    fn ネスト_subgraph_と内部_direction() {
        let d = parse(&flow(
            "subgraph outer\n  direction LR\n  subgraph inner\n    x --> y\n  end\nend",
        ))
        .unwrap();
        assert_eq!(d.subgraphs[0].direction, Some(Direction::Lr));
        assert_eq!(d.subgraphs[1].parent, Some(0));
    }

    #[test]
    fn スタイル系は_unsupported() {
        for src in [
            "style A fill:#f9f",
            "classDef green fill:#9f6",
            "class A green",
            "linkStyle 0 stroke:#f00",
            "click A callback",
            "A:::green --> B",
            "A@{ shape: rounded } --> B",
        ] {
            let err = parse(&flow(src)).unwrap_err();
            assert!(err.is_unsupported(), "{src}: {err}");
        }
    }

    #[test]
    fn 構文エラーの検出() {
        assert!(matches!(
            parse(&flow("A[未閉鎖 --> B")).unwrap_err(),
            Error::Parse { .. }
        ));
        assert!(matches!(
            parse(&flow("end --> B")).unwrap_err(),
            Error::Parse { .. }
        ));
        assert!(matches!(
            parse(&flow("A -- ラベル未閉鎖 B")).unwrap_err(),
            Error::Parse { .. }
        ));
        assert!(matches!(
            parse("A-->B").unwrap_err(), // ヘッダなし
            Error::Parse { .. }
        ));
    }

    #[test]
    fn ノードの再定義は後勝ち() {
        let d = parse(&flow("A --> B\nA{判定}")).unwrap();
        assert_eq!(d.nodes[0].shape, NodeShape::Diamond);
        assert_eq!(d.nodes[0].label, ["判定"]);
    }
}
