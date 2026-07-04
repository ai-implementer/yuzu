//! sequenceDiagram の行指向パーサ（手書き）。
//!
//! 文法は完全に行指向（1 行 1 文、ブロックは `end` のスタック管理のみ）なので
//! 先読み不要の素朴な dispatch で足りる。mermaid.js の実装が事実上の仕様。

use std::collections::HashMap;

use crate::common::text::{decode_entities, split_br_lines};
use crate::error::Error;
use crate::kind::trim_line;
use crate::sequence::model::{
    BlockKind, Event, HeadKind, LineKind, NotePos, PBox, Participant, SequenceDiagram,
};

/// 矢印トークン（**最長一致**のためこの順序で探索する）
const ARROWS: &[(&str, LineKind, HeadKind)] = &[
    ("<<-->>", LineKind::Dotted, HeadKind::BothArrow),
    ("<<->>", LineKind::Solid, HeadKind::BothArrow),
    ("-->>", LineKind::Dotted, HeadKind::Arrow),
    ("->>", LineKind::Solid, HeadKind::Arrow),
    ("--x", LineKind::Dotted, HeadKind::Cross),
    ("-x", LineKind::Solid, HeadKind::Cross),
    ("--)", LineKind::Dotted, HeadKind::Open),
    ("-)", LineKind::Solid, HeadKind::Open),
    ("-->", LineKind::Dotted, HeadKind::None),
    ("->", LineKind::Solid, HeadKind::None),
];

/// 明示的に「未対応」としてフォールバックさせる構文の先頭キーワード
const UNSUPPORTED_KEYWORDS: &[&str] = &["create", "destroy", "link", "links", "properties"];

struct Parser {
    participants: Vec<Participant>,
    index: HashMap<String, usize>,
    boxes: Vec<PBox>,
    current_box: Option<usize>,
    events: Vec<Event>,
    block_stack: Vec<BlockKind>,
    title: Option<Vec<String>>,
    /// 参加者ごとの activation 深さ（不整合検出用）
    activation_depth: HashMap<usize, i32>,
}

pub(crate) fn parse(source: &str) -> Result<SequenceDiagram, Error> {
    let mut p = Parser {
        participants: Vec::new(),
        index: HashMap::new(),
        boxes: Vec::new(),
        current_box: None,
        events: Vec::new(),
        block_stack: Vec::new(),
        title: None,
        activation_depth: HashMap::new(),
    };

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

        if !seen_header {
            if line == "sequenceDiagram" {
                seen_header = true;
                continue;
            }
            return Err(Error::Parse {
                line: line_no,
                message: "sequenceDiagram ヘッダがありません".to_string(),
            });
        }

        p.statement(line, line_no)?;
    }

    if !p.block_stack.is_empty() {
        return Err(Error::Parse {
            line: source.lines().count(),
            message: "閉じられていないブロックがあります（end 不足）".to_string(),
        });
    }
    if p.current_box.is_some() {
        return Err(Error::Parse {
            line: source.lines().count(),
            message: "閉じられていない box があります（end 不足）".to_string(),
        });
    }

    Ok(SequenceDiagram {
        title: p.title,
        participants: p.participants,
        boxes: p.boxes,
        events: p.events,
    })
}

impl Parser {
    fn statement(&mut self, line: &str, line_no: usize) -> Result<(), Error> {
        let keyword = line.split_whitespace().next().unwrap_or("");
        let rest = trim_line(&line[keyword.len().min(line.len())..]);

        if UNSUPPORTED_KEYWORDS.contains(&keyword.to_ascii_lowercase().as_str()) {
            return Err(Error::UnsupportedSyntax {
                line: line_no,
                construct: keyword.to_string(),
            });
        }
        // 参加者ステレオタイプ等のメタデータ構文（v11.12+）は未対応
        if line.contains("@{") {
            return Err(Error::UnsupportedSyntax {
                line: line_no,
                construct: "@{...}".to_string(),
            });
        }

        match keyword.to_ascii_lowercase().as_str() {
            "participant" => self.declare_participant(rest, false, line_no),
            "actor" => self.declare_participant(rest, true, line_no),
            "box" => self.begin_box(rest, line_no),
            "end" if rest.is_empty() => self.end_block(line_no),
            "activate" => {
                let id = self.intern(rest);
                self.push_activation(id, line_no)?;
                self.events.push(Event::Activate(id));
                Ok(())
            }
            "deactivate" => {
                let id = self.intern(rest);
                self.pop_activation(id, line_no)?;
                self.events.push(Event::Deactivate(id));
                Ok(())
            }
            "note" => self.note(rest, line_no),
            "loop" => self.begin_block(BlockKind::Loop, rest),
            "alt" => self.begin_block(BlockKind::Alt, rest),
            "opt" => self.begin_block(BlockKind::Opt, rest),
            "par" => self.begin_block(BlockKind::Par, rest),
            "critical" => self.begin_block(BlockKind::Critical, rest),
            "break" => self.begin_block(BlockKind::Break, rest),
            "rect" => self.begin_block(BlockKind::Rect(rest.to_string()), ""),
            "else" | "and" | "option" => self.separator(keyword, rest, line_no),
            "autonumber" => self.autonumber(rest, line_no),
            "title" => {
                let text = rest.strip_prefix(':').map(trim_line).unwrap_or(rest);
                self.title = Some(split_text(text));
                Ok(())
            }
            _ => self.message(line, line_no),
        }
    }

    /// 参加者を登録し添字を返す（既出ならその添字）
    fn intern(&mut self, name: &str) -> usize {
        let name = trim_line(name);
        if let Some(&id) = self.index.get(name) {
            return id;
        }
        let id = self.participants.len();
        self.participants.push(Participant {
            display: split_text(name),
            is_actor: false,
        });
        self.index.insert(name.to_string(), id);
        if let Some(box_id) = self.current_box {
            self.boxes[box_id].members.push(id);
        }
        id
    }

    fn declare_participant(
        &mut self,
        rest: &str,
        is_actor: bool,
        line_no: usize,
    ) -> Result<(), Error> {
        if rest.is_empty() {
            return Err(Error::Parse {
                line: line_no,
                message: "participant/actor に名前がありません".to_string(),
            });
        }
        let (name, display) = match rest.split_once(" as ") {
            Some((id, disp)) => (trim_line(id), Some(trim_line(disp))),
            None => (rest, None),
        };
        let id = self.intern(name);
        if let Some(disp) = display {
            self.participants[id].display = split_text(disp);
        }
        self.participants[id].is_actor = is_actor;
        Ok(())
    }

    fn begin_box(&mut self, rest: &str, line_no: usize) -> Result<(), Error> {
        if self.current_box.is_some() {
            return Err(Error::Parse {
                line: line_no,
                message: "box は入れ子にできません".to_string(),
            });
        }
        let (color, label) = split_box_color(rest);
        self.boxes.push(PBox {
            label: split_text(label),
            color,
            members: Vec::new(),
        });
        self.current_box = Some(self.boxes.len() - 1);
        Ok(())
    }

    fn begin_block(&mut self, kind: BlockKind, label: &str) -> Result<(), Error> {
        self.events.push(Event::BlockBegin {
            kind: kind.clone(),
            label: split_text(label),
        });
        self.block_stack.push(kind);
        Ok(())
    }

    fn separator(&mut self, keyword: &str, rest: &str, line_no: usize) -> Result<(), Error> {
        let expected = self
            .block_stack
            .last()
            .and_then(|kind| kind.separator_keyword());
        if expected != Some(keyword) {
            return Err(Error::Parse {
                line: line_no,
                message: format!("`{keyword}` が対応するブロックの外にあります"),
            });
        }
        self.events.push(Event::BlockSeparator {
            label: split_text(rest),
        });
        Ok(())
    }

    fn end_block(&mut self, line_no: usize) -> Result<(), Error> {
        if self.block_stack.pop().is_some() {
            self.events.push(Event::BlockEnd);
            return Ok(());
        }
        // ブロックが無ければ box の終端
        if self.current_box.take().is_some() {
            return Ok(());
        }
        Err(Error::Parse {
            line: line_no,
            message: "対応するブロックのない end です".to_string(),
        })
    }

    fn note(&mut self, rest: &str, line_no: usize) -> Result<(), Error> {
        let Some((pos_part, text)) = rest.split_once(':') else {
            return Err(Error::Parse {
                line: line_no,
                message: "Note に `:` がありません".to_string(),
            });
        };
        let pos_part = trim_line(pos_part);
        let lower = pos_part.to_ascii_lowercase();
        let (pos, actors_str) = if let Some(a) = lower.strip_prefix("left of ") {
            (NotePos::LeftOf, &pos_part[pos_part.len() - a.len()..])
        } else if let Some(a) = lower.strip_prefix("right of ") {
            (NotePos::RightOf, &pos_part[pos_part.len() - a.len()..])
        } else if let Some(a) = lower.strip_prefix("over ") {
            (NotePos::Over, &pos_part[pos_part.len() - a.len()..])
        } else {
            return Err(Error::Parse {
                line: line_no,
                message: "Note の位置指定（left of / right of / over）が不正です".to_string(),
            });
        };

        let (a, b) = match actors_str.split_once(',') {
            Some((x, y)) if pos == NotePos::Over => {
                (self.intern(x), Some(self.intern(trim_line(y))))
            }
            _ => (self.intern(actors_str), None),
        };
        self.events.push(Event::Note {
            pos,
            a,
            b,
            text: split_text(trim_line(text)),
        });
        Ok(())
    }

    fn autonumber(&mut self, rest: &str, line_no: usize) -> Result<(), Error> {
        let parts: Vec<&str> = rest.split_whitespace().collect();
        let event = match parts.as_slice() {
            [] => Event::AutonumberOn { start: 1, step: 1 },
            ["off"] => Event::AutonumberOff,
            [n] => Event::AutonumberOn {
                start: parse_u32(n, line_no)?,
                step: 1,
            },
            [n, m] => Event::AutonumberOn {
                start: parse_u32(n, line_no)?,
                step: parse_u32(m, line_no)?,
            },
            _ => {
                return Err(Error::Parse {
                    line: line_no,
                    message: "autonumber の引数が不正です".to_string(),
                });
            }
        };
        self.events.push(event);
        Ok(())
    }

    fn message(&mut self, line: &str, line_no: usize) -> Result<(), Error> {
        // `:` より後ろはテキスト（`:` 省略は空テキスト）
        let (left, text) = match line.split_once(':') {
            Some((l, t)) => (trim_line(l), trim_line(t)),
            None => (line, ""),
        };

        // 矢印を最長一致で探索
        let mut found: Option<(usize, &(&str, LineKind, HeadKind))> = None;
        for arrow in ARROWS {
            if let Some(pos) = left.find(arrow.0) {
                found = Some((pos, arrow));
                break;
            }
        }
        let Some((pos, &(token, line_kind, head))) = found else {
            return Err(Error::Parse {
                line: line_no,
                message: "文として解釈できません（矢印が見つかりません）".to_string(),
            });
        };

        let from_str = trim_line(&left[..pos]);
        let mut to_str = trim_line(&left[pos + token.len()..]);

        // 矢印直後の +/-（activation 指示）
        let mut activate_to = false;
        let mut deactivate_from = false;
        if let Some(rest) = to_str.strip_prefix('+') {
            activate_to = true;
            to_str = trim_line(rest);
        } else if let Some(rest) = to_str.strip_prefix('-') {
            deactivate_from = true;
            to_str = trim_line(rest);
        }

        if from_str.is_empty() || to_str.is_empty() {
            return Err(Error::Parse {
                line: line_no,
                message: "メッセージの送信元/送信先が空です".to_string(),
            });
        }

        let from = self.intern(from_str);
        let to = self.intern(to_str);

        if activate_to {
            self.push_activation(to, line_no)?;
        }
        if deactivate_from {
            self.pop_activation(from, line_no)?;
        }

        self.events.push(Event::Message {
            from,
            to,
            line: line_kind,
            head,
            text: split_text(text),
            activate_to,
            deactivate_from,
        });
        Ok(())
    }

    fn push_activation(&mut self, id: usize, _line_no: usize) -> Result<(), Error> {
        *self.activation_depth.entry(id).or_insert(0) += 1;
        Ok(())
    }

    fn pop_activation(&mut self, id: usize, line_no: usize) -> Result<(), Error> {
        let depth = self.activation_depth.entry(id).or_insert(0);
        if *depth <= 0 {
            return Err(Error::Parse {
                line: line_no,
                message: "activate されていない参加者を deactivate しようとしました".to_string(),
            });
        }
        *depth -= 1;
        Ok(())
    }
}

/// テキストをデコードし `<br/>` で行分割する
fn split_text(text: &str) -> Vec<String> {
    split_br_lines(&decode_entities(text))
}

fn parse_u32(s: &str, line_no: usize) -> Result<u32, Error> {
    s.parse().map_err(|_| Error::Parse {
        line: line_no,
        message: format!("数値として解釈できません: {s}"),
    })
}

/// `box <色?> <ラベル>` の色部分を切り出す
fn split_box_color(rest: &str) -> (Option<String>, &str) {
    let rest = trim_line(rest);
    // rgb(...) / rgba(...) は括弧まで含めて色
    for prefix in ["rgb(", "rgba("] {
        if rest.to_ascii_lowercase().starts_with(prefix) {
            if let Some(close) = rest.find(')') {
                return (
                    Some(rest[..=close].to_string()),
                    trim_line(&rest[close + 1..]),
                );
            }
        }
    }
    let first = rest.split_whitespace().next().unwrap_or("");
    if first.starts_with('#') || is_css_color_name(first) {
        return (Some(first.to_string()), trim_line(&rest[first.len()..]));
    }
    (None, rest)
}

/// CSS 名前色（標準 148 色 + transparent）か
fn is_css_color_name(name: &str) -> bool {
    const NAMES: &[&str] = &[
        "aliceblue",
        "antiquewhite",
        "aqua",
        "aquamarine",
        "azure",
        "beige",
        "bisque",
        "black",
        "blanchedalmond",
        "blue",
        "blueviolet",
        "brown",
        "burlywood",
        "cadetblue",
        "chartreuse",
        "chocolate",
        "coral",
        "cornflowerblue",
        "cornsilk",
        "crimson",
        "cyan",
        "darkblue",
        "darkcyan",
        "darkgoldenrod",
        "darkgray",
        "darkgreen",
        "darkgrey",
        "darkkhaki",
        "darkmagenta",
        "darkolivegreen",
        "darkorange",
        "darkorchid",
        "darkred",
        "darksalmon",
        "darkseagreen",
        "darkslateblue",
        "darkslategray",
        "darkslategrey",
        "darkturquoise",
        "darkviolet",
        "deeppink",
        "deepskyblue",
        "dimgray",
        "dimgrey",
        "dodgerblue",
        "firebrick",
        "floralwhite",
        "forestgreen",
        "fuchsia",
        "gainsboro",
        "ghostwhite",
        "gold",
        "goldenrod",
        "gray",
        "green",
        "greenyellow",
        "grey",
        "honeydew",
        "hotpink",
        "indianred",
        "indigo",
        "ivory",
        "khaki",
        "lavender",
        "lavenderblush",
        "lawngreen",
        "lemonchiffon",
        "lightblue",
        "lightcoral",
        "lightcyan",
        "lightgoldenrodyellow",
        "lightgray",
        "lightgreen",
        "lightgrey",
        "lightpink",
        "lightsalmon",
        "lightseagreen",
        "lightskyblue",
        "lightslategray",
        "lightslategrey",
        "lightsteelblue",
        "lightyellow",
        "lime",
        "limegreen",
        "linen",
        "magenta",
        "maroon",
        "mediumaquamarine",
        "mediumblue",
        "mediumorchid",
        "mediumpurple",
        "mediumseagreen",
        "mediumslateblue",
        "mediumspringgreen",
        "mediumturquoise",
        "mediumvioletred",
        "midnightblue",
        "mintcream",
        "mistyrose",
        "moccasin",
        "navajowhite",
        "navy",
        "oldlace",
        "olive",
        "olivedrab",
        "orange",
        "orangered",
        "orchid",
        "palegoldenrod",
        "palegreen",
        "paleturquoise",
        "palevioletred",
        "papayawhip",
        "peachpuff",
        "peru",
        "pink",
        "plum",
        "powderblue",
        "purple",
        "rebeccapurple",
        "red",
        "rosybrown",
        "royalblue",
        "saddlebrown",
        "salmon",
        "sandybrown",
        "seagreen",
        "seashell",
        "sienna",
        "silver",
        "skyblue",
        "slateblue",
        "slategray",
        "slategrey",
        "snow",
        "springgreen",
        "steelblue",
        "tan",
        "teal",
        "thistle",
        "tomato",
        "transparent",
        "turquoise",
        "violet",
        "wheat",
        "white",
        "whitesmoke",
        "yellow",
        "yellowgreen",
    ];
    NAMES
        .binary_search(&name.to_ascii_lowercase().as_str())
        .is_ok()
}

#[cfg(test)]
mod tests {
    use super::parse;
    use crate::error::Error;
    use crate::sequence::model::{Event, HeadKind, LineKind, NotePos};

    fn seq(body: &str) -> String {
        format!("sequenceDiagram\n{body}")
    }

    #[test]
    fn 全種類の矢印をパースできる() {
        let cases = [
            ("A->B: x", LineKind::Solid, HeadKind::None),
            ("A-->B: x", LineKind::Dotted, HeadKind::None),
            ("A->>B: x", LineKind::Solid, HeadKind::Arrow),
            ("A-->>B: x", LineKind::Dotted, HeadKind::Arrow),
            ("A<<->>B: x", LineKind::Solid, HeadKind::BothArrow),
            ("A<<-->>B: x", LineKind::Dotted, HeadKind::BothArrow),
            ("A-xB: x", LineKind::Solid, HeadKind::Cross),
            ("A--xB: x", LineKind::Dotted, HeadKind::Cross),
            ("A-)B: x", LineKind::Solid, HeadKind::Open),
            ("A--)B: x", LineKind::Dotted, HeadKind::Open),
        ];
        for (src, line, head) in cases {
            let d = parse(&seq(src)).unwrap_or_else(|e| panic!("{src}: {e}"));
            let Event::Message {
                line: l, head: h, ..
            } = &d.events[0]
            else {
                panic!("{src}: メッセージでない");
            };
            assert_eq!((*l, *h), (line, head), "{src}");
        }
    }

    #[test]
    fn 参加者の宣言と暗黙登録() {
        let d = parse(&seq("participant B as ビー\nactor C\nA->>B: hi\nB->>C: ho")).unwrap();
        // 宣言順: B, C, 暗黙の A
        assert_eq!(d.participants.len(), 3);
        assert_eq!(d.participants[0].display, ["ビー"]);
        assert!(d.participants[1].is_actor);
        assert_eq!(d.participants[2].display, ["A"]);
    }

    #[test]
    fn activation_の指示とブロック() {
        let d = parse(&seq(
            "A->>+B: req\nB-->>-A: res\nloop 毎分\nA->>B: ping\nend",
        ))
        .unwrap();
        let Event::Message {
            activate_to,
            deactivate_from,
            ..
        } = &d.events[0]
        else {
            panic!()
        };
        assert!(*activate_to && !*deactivate_from);
        assert!(matches!(d.events[2], Event::BlockBegin { .. }));
        assert!(matches!(d.events[4], Event::BlockEnd));
    }

    #[test]
    fn note_の_4_形() {
        let d = parse(&seq(
            "Note left of A: l\nNote right of A: r\nNote over A: o\nNote over A,B: ab",
        ))
        .unwrap();
        let positions: Vec<(NotePos, bool)> = d
            .events
            .iter()
            .map(|e| match e {
                Event::Note { pos, b, .. } => (*pos, b.is_some()),
                _ => panic!(),
            })
            .collect();
        assert_eq!(
            positions,
            [
                (NotePos::LeftOf, false),
                (NotePos::RightOf, false),
                (NotePos::Over, false),
                (NotePos::Over, true),
            ]
        );
    }

    #[test]
    fn ブロックの不整合はエラー() {
        // alt の外の else
        let err = parse(&seq("loop x\nelse y\nend")).unwrap_err();
        assert!(matches!(err, Error::Parse { line: 3, .. }), "{err}");
        // 深さ 0 の end
        let err = parse(&seq("A->>B: x\nend")).unwrap_err();
        assert!(matches!(err, Error::Parse { line: 3, .. }), "{err}");
        // end 不足
        assert!(parse(&seq("loop x\nA->>B: y")).is_err());
    }

    #[test]
    fn activation_不整合はエラー() {
        let err = parse(&seq("A-->>-B: x")).unwrap_err();
        assert!(matches!(err, Error::Parse { line: 2, .. }), "{err}");
        assert!(parse(&seq("deactivate A")).is_err());
    }

    #[test]
    fn 未対応構文は_unsupported() {
        let err = parse(&seq("create participant D")).unwrap_err();
        assert!(err.is_unsupported(), "{err}");
        let err = parse(&seq("participant A@{ \"type\": \"database\" }")).unwrap_err();
        assert!(err.is_unsupported(), "{err}");
    }

    #[test]
    fn box_と色の解釈() {
        let d = parse(&seq(
            "box Aqua チーム A\nparticipant A\nend\nbox rgb(200, 220, 255) チーム B\nparticipant B\nend\nA->>B: x",
        ))
        .unwrap();
        assert_eq!(d.boxes.len(), 2);
        assert_eq!(d.boxes[0].color.as_deref(), Some("Aqua"));
        assert_eq!(d.boxes[0].label, ["チーム A"]);
        assert_eq!(d.boxes[0].members, [0]);
        assert_eq!(d.boxes[1].color.as_deref(), Some("rgb(200, 220, 255)"));
    }

    #[test]
    fn autonumber_と_title() {
        let d = parse(&seq("autonumber 10 5\ntitle 認証フロー\nA->>B: x")).unwrap();
        assert!(matches!(
            d.events[0],
            Event::AutonumberOn { start: 10, step: 5 }
        ));
        assert_eq!(d.title.as_deref().unwrap(), ["認証フロー"]);
    }

    #[test]
    fn br_とエンティティ() {
        let d = parse(&seq("A->>B: 一行目<br/>二行目 #35;1")).unwrap();
        let Event::Message { text, .. } = &d.events[0] else {
            panic!()
        };
        assert_eq!(text, &["一行目", "二行目 #1"]);
    }

    #[test]
    fn ヘッダなしはエラー() {
        assert!(parse("A->>B: x").is_err());
    }
}
