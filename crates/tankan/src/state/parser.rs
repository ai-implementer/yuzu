//! stateDiagram-v2 の行指向パーサ（flowchart モデルへの変換器）。
//!
//! - **事前スキャン**で composite state（`state X {`）の名前を先登録する。
//!   これにより宣言前の遷移 `A --> X` も最初からクラスタ参照になり、
//!   孤児ノードが生まれない
//! - `[*]` は**スコープ（ルート／composite／region）ごと**に、かつ
//!   遷移元（Start）と遷移先（End）で**別ノード**として登録する
//!   （同一視すると開始と終了が繋がって壊れるため）
//! - `--` 単独行は concurrency 領域の区切り。最初の `--` を見た時点で
//!   それまでの直下要素を第 1 領域（region クラスタ）へ移し替える
//! - fork/join バーの向きは所属スコープの実効 direction から後決めする

use std::collections::HashMap;

use crate::common::style::StyleCollector;
use crate::common::text::{decode_entities, split_br_lines};
use crate::error::Error;
use crate::flowchart::model::{
    Direction, Edge, EdgeLine, EdgeTip, EndRef, FlowchartDiagram, Node, NodeShape, Subgraph,
};
use crate::kind::trim_line;

pub(crate) fn parse(source: &str) -> Result<FlowchartDiagram, Error> {
    let mut p = StateParser::default();
    p.prescan_composites(source);

    let mut in_directive = false;
    let mut in_frontmatter = false;
    let mut seen_header = false;
    let mut first_content = true;
    let mut note_block: Option<(usize, Vec<String>)> = None; // (対象ノード, 行バッファ)

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
                p.diagram.title = Some(split_text(t));
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

        // note ブロックモード
        if note_block.is_some() {
            if line.eq_ignore_ascii_case("end note") {
                let (target, lines) = note_block.take().expect("Some 確認済み");
                p.attach_note(target, lines);
            } else if let Some((_, lines)) = &mut note_block {
                lines.push(decode_entities(line));
            }
            continue;
        }

        if !seen_header {
            if line == "stateDiagram-v2" || line == "stateDiagram" {
                seen_header = true;
                continue;
            }
            return Err(Error::Parse {
                line: line_no,
                message: "stateDiagram ヘッダがありません".to_string(),
            });
        }

        if let Some(block) = p.statement(line, line_no)? {
            note_block = Some(block);
        }
    }

    if !p.scope_stack.is_empty() {
        return Err(Error::Parse {
            line: source.lines().count(),
            message: "閉じられていない composite state があります（} 不足）".to_string(),
        });
    }
    if note_block.is_some() {
        return Err(Error::Parse {
            line: source.lines().count(),
            message: "閉じられていない note があります（end note 不足）".to_string(),
        });
    }

    Ok(p.finish())
}

/// composite のスコープ管理
struct ScopeCtx {
    /// composite 本体のクラスタ id
    subgraph: usize,
    /// concurrency 領域に入っている場合はその region クラスタ id
    current_region: Option<usize>,
}

#[derive(Default)]
struct StateParser {
    diagram: FlowchartDiagram,
    node_index: HashMap<String, usize>,
    /// composite state 名 → クラスタ id（事前スキャンで登録）
    composite_index: HashMap<String, usize>,
    /// (実効スコープ, is_start) → [*] ノード
    star_index: HashMap<(Option<usize>, bool), usize>,
    scope_stack: Vec<ScopeCtx>,
    /// classDef / class / `:::` / style を蓄積し、finish で各状態ノードへ配る
    styles: StyleCollector,
}

impl StateParser {
    /// composite 宣言（`state X {` / `state "説明" as X {`）を先に集める
    fn prescan_composites(&mut self, source: &str) {
        for raw in source.lines() {
            let line = trim_line(raw);
            let Some(rest) = line.strip_prefix("state ") else {
                continue;
            };
            let Some(rest) = trim_line(rest).strip_suffix('{') else {
                continue;
            };
            let rest = trim_line(rest);
            let name = match rest.strip_prefix('"') {
                Some(r) => r
                    .split_once('"')
                    .and_then(|(_, after)| trim_line(after).strip_prefix("as "))
                    .map(trim_line),
                None => Some(rest),
            };
            let Some(name) = name else { continue };
            if name.is_empty() || self.composite_index.contains_key(name) {
                continue;
            }
            let id = self.diagram.subgraphs.len();
            self.diagram.subgraphs.push(Subgraph {
                title: vec![name.to_string()],
                parent: None, // 宣言時に確定
                direction: None,
                region: false,
            });
            self.composite_index.insert(name.to_string(), id);
        }
    }

    /// 現在ノードを所属させるスコープ（region があれば region）
    fn effective_scope(&self) -> Option<usize> {
        self.scope_stack
            .last()
            .map(|ctx| ctx.current_region.unwrap_or(ctx.subgraph))
    }

    /// 戻り値: note ブロック開始なら Some((対象, 空バッファ))
    fn statement(
        &mut self,
        line: &str,
        line_no: usize,
    ) -> Result<Option<(usize, Vec<String>)>, Error> {
        let keyword = line.split_whitespace().next().unwrap_or("");

        if line == "}" {
            if self.scope_stack.pop().is_none() {
                return Err(Error::Parse {
                    line: line_no,
                    message: "対応する composite のない } です".to_string(),
                });
            }
            return Ok(None);
        }
        if line == "--" {
            self.begin_region(line_no)?;
            return Ok(None);
        }

        match keyword {
            "classDef" => {
                self.styles.class_def(trim_line(&line["classDef".len()..]));
                Ok(None)
            }
            "class" => {
                self.styles.apply_class(trim_line(&line["class".len()..]));
                Ok(None)
            }
            "style" => {
                self.styles.apply_style(trim_line(&line["style".len()..]));
                Ok(None)
            }
            "direction" => {
                let dir = parse_direction(trim_line(&line["direction".len()..]), line_no)?;
                match self.scope_stack.last() {
                    Some(ctx) => self.diagram.subgraphs[ctx.subgraph].direction = Some(dir),
                    None => self.diagram.direction = dir,
                }
                Ok(None)
            }
            "state" => {
                self.state_decl(trim_line(&line["state".len()..]), line_no)?;
                Ok(None)
            }
            "note" => self.note_stmt(trim_line(&line["note".len()..]), line_no),
            _ if line.contains("-->") => {
                self.transition(line, line_no)?;
                Ok(None)
            }
            // `Ident:::class`（遷移でも説明でもない、インラインクラス付きの状態宣言）。
            // 説明の `:` より先に判定して `:::` の誤分割を防ぐ
            _ if line.contains(":::") => {
                self.inline_decl(line, line_no)?;
                Ok(None)
            }
            _ if line.contains(':') => {
                // `s1 : 説明`（複数回で行追加。composite はタイトルへ）
                let (name, desc) = line.split_once(':').expect("contains(':') 確認済み");
                let name = trim_line(name);
                let desc = split_text(desc);
                if let Some(&sub) = self.composite_index.get(name) {
                    self.diagram.subgraphs[sub].title.extend(desc);
                    return Ok(None);
                }
                let id = self.intern(name, line_no)?;
                let node = &mut self.diagram.nodes[id];
                if node.label == [name] {
                    node.label = desc;
                } else {
                    node.label.extend(desc);
                }
                Ok(None)
            }
            _ if !line.contains(' ') => {
                // 裸の状態宣言
                if !self.composite_index.contains_key(line) {
                    self.intern(line, line_no)?;
                }
                Ok(None)
            }
            _ => Err(Error::Parse {
                line: line_no,
                message: "文として解釈できません".to_string(),
            }),
        }
    }

    /// `state ...` 宣言:
    /// `state "説明" as s1 [{]` / `state s1 <<choice|fork|join>>` / `state s1 [{]`
    fn state_decl(&mut self, rest: &str, line_no: usize) -> Result<(), Error> {
        let (rest, opens_block) = match rest.strip_suffix('{') {
            Some(r) => (trim_line(r), true),
            None => (rest, false),
        };

        let (name, display): (&str, Option<Vec<String>>) = if let Some(r) = rest.strip_prefix('"') {
            let Some((desc, after)) = r.split_once('"') else {
                return Err(Error::Parse {
                    line: line_no,
                    message: "説明の引用符が閉じられていません".to_string(),
                });
            };
            let Some(name) = trim_line(after).strip_prefix("as ") else {
                return Err(Error::Parse {
                    line: line_no,
                    message: "`state \"説明\" as 名前` の形にしてください".to_string(),
                });
            };
            (trim_line(name), Some(split_text(desc)))
        } else {
            (rest, None)
        };

        // `<<choice>>` / `<<fork>>` / `<<join>>`
        let (name, special) = match name.split_once("<<") {
            Some((n, k)) => {
                let kind = k.strip_suffix(">>").map(trim_line);
                let shape = match kind {
                    Some("choice") => NodeShape::Diamond,
                    // 向きは finish() で実効 direction から決める（仮に横）
                    Some("fork") | Some("join") => NodeShape::ForkBar(false),
                    _ => {
                        return Err(Error::UnsupportedSyntax {
                            line: line_no,
                            construct: format!("<<{}>>", kind.unwrap_or(k)),
                        });
                    }
                };
                (trim_line(n), Some(shape))
            }
            None => (name, None),
        };

        if opens_block {
            // 事前スキャンで登録済みのクラスタに親・タイトルを確定してスコープへ
            let sub = *self
                .composite_index
                .get(name)
                .expect("事前スキャンで登録済み");
            self.diagram.subgraphs[sub].parent = self.effective_scope();
            if let Some(display) = display {
                self.diagram.subgraphs[sub].title = display;
            }
            self.scope_stack.push(ScopeCtx {
                subgraph: sub,
                current_region: None,
            });
            return Ok(());
        }

        let id = self.intern(name, line_no)?;
        if let Some(display) = display {
            self.diagram.nodes[id].label = display;
        }
        if let Some(shape) = special {
            self.diagram.nodes[id].shape = shape;
            self.diagram.nodes[id].label = Vec::new(); // choice/fork はラベルなし
        }
        Ok(())
    }

    fn begin_region(&mut self, line_no: usize) -> Result<(), Error> {
        let Some(top_subgraph) = self.scope_stack.last().map(|c| c.subgraph) else {
            return Err(Error::Parse {
                line: line_no,
                message: "`--` は composite state の中でのみ使えます".to_string(),
            });
        };
        // 最初の区切りなら、これまでの直下要素を第 1 領域へ移し替える
        let has_region = self
            .scope_stack
            .last()
            .is_some_and(|c| c.current_region.is_some());
        if !has_region {
            let first = self.new_region(top_subgraph);
            for node in &mut self.diagram.nodes {
                if node.subgraph == Some(top_subgraph) {
                    node.subgraph = Some(first);
                }
            }
            for sid in 0..self.diagram.subgraphs.len() {
                if sid != first
                    && self.diagram.subgraphs[sid].parent == Some(top_subgraph)
                    && !self.diagram.subgraphs[sid].region
                {
                    self.diagram.subgraphs[sid].parent = Some(first);
                }
            }
        }
        let next = self.new_region(top_subgraph);
        if let Some(ctx) = self.scope_stack.last_mut() {
            ctx.current_region = Some(next);
        }
        Ok(())
    }

    fn new_region(&mut self, parent: usize) -> usize {
        let id = self.diagram.subgraphs.len();
        self.diagram.subgraphs.push(Subgraph {
            title: Vec::new(),
            parent: Some(parent),
            direction: None,
            region: true,
        });
        id
    }

    /// `note right of X : text`（1 行形）/ `note left of X`（ブロック開始）
    fn note_stmt(
        &mut self,
        rest: &str,
        line_no: usize,
    ) -> Result<Option<(usize, Vec<String>)>, Error> {
        let lower = rest.to_ascii_lowercase();
        let after = if let Some(a) = lower.strip_prefix("right of ") {
            &rest[rest.len() - a.len()..]
        } else if let Some(a) = lower.strip_prefix("left of ") {
            &rest[rest.len() - a.len()..]
        } else {
            return Err(Error::Parse {
                line: line_no,
                message: "note の位置指定（left of / right of）が不正です".to_string(),
            });
        };
        match after.split_once(':') {
            Some((name, text)) => {
                let target = self.intern(trim_line(name), line_no)?;
                self.attach_note(target, vec![decode_entities(trim_line(text))]);
                Ok(None)
            }
            None => {
                let target = self.intern(trim_line(after), line_no)?;
                Ok(Some((target, Vec::new())))
            }
        }
    }

    /// note をノード＋点線リンクとして付ける
    /// （mermaid の隣接配置の近似。注記として同じ図に読める）
    fn attach_note(&mut self, target: usize, lines: Vec<String>) {
        let id = self.diagram.nodes.len();
        self.diagram.nodes.push(Node {
            label: if lines.is_empty() {
                vec![String::new()]
            } else {
                lines
            },
            shape: NodeShape::NoteBox,
            subgraph: self.diagram.nodes[target].subgraph,
            style: None,
        });
        self.diagram.edges.push(Edge {
            from: EndRef::Node(target),
            to: EndRef::Node(id),
            line: EdgeLine::Dotted,
            head: EdgeTip::None,
            tail: EdgeTip::None,
            minlen: 1,
            label: Vec::new(),
        });
    }

    /// `A --> B : label`（両端に `A:::cls` インラインクラス可）
    fn transition(&mut self, line: &str, line_no: usize) -> Result<(), Error> {
        // 先に `-->` で分割する（`:::` の `:` を説明の `:` と誤分割しないため）
        let Some((from, rhs)) = line.split_once("-->") else {
            return Err(Error::Parse {
                line: line_no,
                message: "遷移として解釈できません".to_string(),
            });
        };
        let (to, label) = match split_label(rhs) {
            (to, Some(text)) => (to, split_text(text)),
            (to, None) => (to, Vec::new()),
        };
        let from = self.intern_end(trim_line(from), true, line_no)?;
        let to = self.intern_end(trim_line(to), false, line_no)?;
        self.diagram.edges.push(Edge {
            from,
            to,
            line: EdgeLine::Solid,
            head: EdgeTip::Arrow,
            tail: EdgeTip::None,
            minlen: 1,
            label,
        });
        Ok(())
    }

    /// `Ident:::class` の裸宣言（任意で ` : 説明` 付き）。`:::` は状態名直後の
    /// 3 連コロンのみ対象。遷移との併記は transition 側で扱う
    fn inline_decl(&mut self, line: &str, line_no: usize) -> Result<(), Error> {
        let (token, rest) = match line.find(char::is_whitespace) {
            Some(i) => (&line[..i], trim_line(&line[i..])),
            None => (line, ""),
        };
        // 素の状態名（`:::` を剥がした側）。説明適用時の既存ラベル判定に使う
        let clean = token.split_once(":::").map_or(token, |(b, _)| b.trim());
        let id = self.intern(token, line_no)?;
        if let Some(desc) = rest.strip_prefix(':') {
            let desc = split_text(desc);
            let node = &mut self.diagram.nodes[id];
            if node.label == [clean] {
                node.label = desc;
            } else {
                node.label.extend(desc);
            }
        } else if !rest.is_empty() {
            return Err(Error::Parse {
                line: line_no,
                message: "文として解釈できません".to_string(),
            });
        }
        Ok(())
    }

    /// `name:::class` の末尾 `:::class` を剥がしてクラス登録し、素の名前を返す。
    /// `:::` が無ければそのまま返す（冪等）
    fn take_inline<'a>(&mut self, name: &'a str) -> &'a str {
        let Some((base, cls)) = name.split_once(":::") else {
            return name;
        };
        let base = base.trim();
        let cls = cls.trim();
        if !base.is_empty() && !cls.is_empty() {
            self.styles.add_inline(base, cls);
        }
        base
    }

    /// 遷移の端点（`[*]` はスコープ×方向ごとに別ノード、composite はクラスタ参照）
    fn intern_end(&mut self, name: &str, is_source: bool, line_no: usize) -> Result<EndRef, Error> {
        let name = self.take_inline(name);
        if name == "[*]" {
            let key = (self.effective_scope(), is_source);
            if let Some(&id) = self.star_index.get(&key) {
                return Ok(EndRef::Node(id));
            }
            let id = self.diagram.nodes.len();
            self.diagram.nodes.push(Node {
                label: Vec::new(),
                shape: if is_source {
                    NodeShape::StateStart
                } else {
                    NodeShape::StateEnd
                },
                subgraph: self.effective_scope(),
                style: None,
            });
            self.star_index.insert(key, id);
            return Ok(EndRef::Node(id));
        }
        if let Some(&sub) = self.composite_index.get(name) {
            return Ok(EndRef::Subgraph(sub));
        }
        Ok(EndRef::Node(self.intern(name, line_no)?))
    }

    fn intern(&mut self, name: &str, line_no: usize) -> Result<usize, Error> {
        let name = self.take_inline(name);
        if name.is_empty() || name.contains([' ', '{', '}']) {
            return Err(Error::Parse {
                line: line_no,
                message: format!("状態名として解釈できません: `{name}`"),
            });
        }
        if let Some(&id) = self.node_index.get(name) {
            return Ok(id);
        }
        let id = self.diagram.nodes.len();
        self.diagram.nodes.push(Node {
            label: vec![name.to_string()],
            shape: NodeShape::Round,
            subgraph: self.effective_scope(),
            style: None,
        });
        self.node_index.insert(name.to_string(), id);
        Ok(id)
    }

    /// 後処理: fork/join バーの向きを実効 direction から決める＋インラインスタイル配布
    fn finish(mut self) -> FlowchartDiagram {
        for i in 0..self.diagram.nodes.len() {
            if let NodeShape::ForkBar(_) = self.diagram.nodes[i].shape {
                let dir = self.effective_direction(self.diagram.nodes[i].subgraph);
                let vertical = matches!(dir, Direction::Lr | Direction::Rl);
                self.diagram.nodes[i].shape = NodeShape::ForkBar(vertical);
            }
        }
        // classDef / class / `:::` / style を各状態ノードへ配る。
        // [*]（Start/End）・note・composite クラスタは node_index に無いので対象外
        if !self.styles.is_empty() {
            let mut names = vec![String::new(); self.diagram.nodes.len()];
            for (name, &idx) in &self.node_index {
                names[idx] = name.clone();
            }
            for (node, name) in self.diagram.nodes.iter_mut().zip(&names) {
                if name.is_empty() {
                    continue;
                }
                if let Some(style) = self.styles.resolve(name) {
                    node.style = Some(style);
                }
            }
        }
        self.diagram
    }

    fn effective_direction(&self, scope: Option<usize>) -> Direction {
        let mut cur = scope;
        while let Some(s) = cur {
            if let Some(dir) = self.diagram.subgraphs[s].direction {
                return dir;
            }
            cur = self.diagram.subgraphs[s].parent;
        }
        self.diagram.direction
    }
}

fn split_text(text: &str) -> Vec<String> {
    split_br_lines(&decode_entities(trim_line(text)))
}

/// 遷移右辺 `TO[:::cls][ : label]` を (TO トークン, ラベル文字列) に分ける。
/// `:::` インラインクラスの `:` はラベル区切り（単独 `:`）と誤認しない
fn split_label(s: &str) -> (&str, Option<&str>) {
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b':' {
            if s[i..].starts_with(":::") {
                i += 3; // `:::` インラインクラスは読み飛ばす
                continue;
            }
            return (&s[..i], Some(&s[i + 1..]));
        }
        i += 1;
    }
    (s, None)
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
    use crate::flowchart::model::{EndRef, FlowchartDiagram, NodeShape};

    fn st(body: &str) -> String {
        format!("stateDiagram-v2\n{body}")
    }

    #[test]
    fn 基本の遷移と開始終了() {
        let d = parse(&st(
            "[*] --> Still\nStill --> [*]\nStill --> Moving\nMoving --> Still",
        ))
        .unwrap();
        // [*] は Start と End で別ノード
        let starts = d
            .nodes
            .iter()
            .filter(|n| n.shape == NodeShape::StateStart)
            .count();
        let ends = d
            .nodes
            .iter()
            .filter(|n| n.shape == NodeShape::StateEnd)
            .count();
        assert_eq!((starts, ends), (1, 1));
        assert_eq!(d.edges.len(), 4);
        // 状態は角丸
        assert!(
            d.nodes
                .iter()
                .any(|n| n.label == ["Still"] && n.shape == NodeShape::Round)
        );
    }

    #[test]
    fn 説明付き状態とラベル付き遷移() {
        let d = parse(&st(
            "state \"これは説明\" as s2\ns2 : 追記行\ns1 --> s2 : 条件A",
        ))
        .unwrap();
        let s2 = d
            .nodes
            .iter()
            .find(|n| n.label.contains(&"これは説明".to_string()))
            .unwrap();
        assert_eq!(s2.label, ["これは説明", "追記行"]);
        assert_eq!(d.edges[0].label, ["条件A"]);
    }

    #[test]
    fn composite_はクラスタになり宣言前の遷移も繋がる() {
        let d = parse(&st(
            "[*] --> First\nFirst --> Second\nstate First {\n  [*] --> fir\n  fir --> [*]\n}\nstate Second {\n  inner2\n}",
        ))
        .unwrap();
        assert_eq!(d.subgraphs.len(), 2);
        // First --> Second はクラスタ間エッジ
        let e = d
            .edges
            .iter()
            .find(|e| matches!((e.from, e.to), (EndRef::Subgraph(0), EndRef::Subgraph(1))))
            .expect("composite 間の遷移");
        assert_eq!(e.label.len(), 0);
        // composite 内の [*] はルートの [*] と別
        let starts = d
            .nodes
            .iter()
            .filter(|n| n.shape == NodeShape::StateStart)
            .count();
        assert_eq!(starts, 2, "ルートと First 内");
    }

    #[test]
    fn choice_fork_join() {
        let d = parse(&st(
            "state if_state <<choice>>\nstate fork_state <<fork>>\nstate join_state <<join>>\n[*] --> if_state",
        ))
        .unwrap();
        assert!(d.nodes.iter().any(|n| n.shape == NodeShape::Diamond));
        assert_eq!(
            d.nodes
                .iter()
                .filter(|n| matches!(n.shape, NodeShape::ForkBar(false)))
                .count(),
            2
        );
    }

    #[test]
    fn lr_では_fork_バーが縦になる() {
        let d = parse(&st("direction LR\nstate f <<fork>>\n[*] --> f")).unwrap();
        assert!(
            d.nodes
                .iter()
                .any(|n| matches!(n.shape, NodeShape::ForkBar(true)))
        );
    }

    #[test]
    fn note_の_2_形式() {
        let d = parse(&st(
            "s1: 状態\nnote right of s1 : 一行ノート\nnote left of s1\n複数行の\nノート\nend note",
        ))
        .unwrap();
        let notes: Vec<_> = d
            .nodes
            .iter()
            .filter(|n| n.shape == NodeShape::NoteBox)
            .collect();
        assert_eq!(notes.len(), 2);
        assert_eq!(notes[0].label, ["一行ノート"]);
        assert_eq!(notes[1].label, ["複数行の", "ノート"]);
        // 点線リンクで繋がる
        assert_eq!(d.edges.len(), 2);
    }

    #[test]
    fn concurrency_は_region_クラスタに分かれる() {
        let d = parse(&st(
            "state Active {\n  NumLockOff --> NumLockOn\n  --\n  CapsLockOff --> CapsLockOn\n}",
        ))
        .unwrap();
        let regions = d.subgraphs.iter().filter(|s| s.region).count();
        assert_eq!(regions, 2);
        // 各領域のノードは別 region に所属
        let num = d.nodes.iter().find(|n| n.label == ["NumLockOff"]).unwrap();
        let caps = d.nodes.iter().find(|n| n.label == ["CapsLockOff"]).unwrap();
        assert_ne!(num.subgraph, caps.subgraph);
        // region の親は composite
        assert!(
            d.subgraphs[num.subgraph.unwrap()].region
                && !d.subgraphs[d.subgraphs[num.subgraph.unwrap()].parent.unwrap()].region
        );
    }

    #[test]
    fn 未対応の修飾は_unsupported() {
        // `<<history>>` は未対応（choice/fork/join のみ受理）
        let err = parse(&st("state h <<history>>")).unwrap_err();
        assert!(err.is_unsupported(), "{err}");
    }

    /// 状態名から解決済みスタイルを取り出す
    fn state_style(d: &FlowchartDiagram, label: &str) -> crate::common::style::Style {
        let n = d
            .nodes
            .iter()
            .find(|n| n.label == [label])
            .unwrap_or_else(|| panic!("状態 {label} が見つかりません"));
        n.style.clone().unwrap_or_default()
    }

    #[test]
    fn インラインクラスと後置_classdef_が効く() {
        // classDef が使用行より後にあっても解決される
        let d = parse(&st("s1:::warn --> s2\nclassDef warn fill:#f96,stroke:#333")).unwrap();
        let s = state_style(&d, "s1");
        assert_eq!(s.fill.as_deref(), Some("#f96"));
        assert_eq!(s.stroke.as_deref(), Some("#333"));
        // クラスなしの状態はスタイルなし
        assert!(
            d.nodes
                .iter()
                .find(|n| n.label == ["s2"])
                .unwrap()
                .style
                .is_none()
        );
    }

    #[test]
    fn classdef_default_が全状態既定になる() {
        let d = parse(&st("s1 --> s2\nclassDef default fill:#eee")).unwrap();
        for label in ["s1", "s2"] {
            assert_eq!(
                state_style(&d, label).fill.as_deref(),
                Some("#eee"),
                "{label}"
            );
        }
    }

    #[test]
    fn class_文と_style_文が適用される() {
        // class 文で複数状態へ、style 文はプロパティ単位で後勝ち
        let d = parse(&st(
            "s1 --> s2\nclassDef hot fill:#f96\nclass s1,s2 hot\nstyle s1 stroke:#111",
        ))
        .unwrap();
        assert_eq!(state_style(&d, "s2").fill.as_deref(), Some("#f96"));
        let s1 = state_style(&d, "s1");
        assert_eq!(s1.fill.as_deref(), Some("#f96"));
        assert_eq!(s1.stroke.as_deref(), Some("#111"));
    }

    #[test]
    fn インラインクラスと説明を併記できる() {
        let d = parse(&st("s1:::warn : 開始\nclassDef warn fill:#f96")).unwrap();
        let n = d
            .nodes
            .iter()
            .find(|n| n.label == ["開始"])
            .expect("説明が反映される");
        assert_eq!(
            n.style.clone().unwrap_or_default().fill.as_deref(),
            Some("#f96")
        );
    }

    #[test]
    fn 構文エラーの検出() {
        assert!(
            parse(&st("state Foo {\n  a --> b")).is_err(),
            "閉じ括弧なし"
        );
        assert!(parse(&st("}")).is_err(), "対応しない閉じ括弧");
        assert!(parse(&st("-- ")).is_err(), "composite 外の --");
        assert!(
            parse(&st("note right of s1\n未閉鎖")).is_err(),
            "end note なし"
        );
        assert!(parse("s1 --> s2").is_err(), "ヘッダなし");
    }
}
