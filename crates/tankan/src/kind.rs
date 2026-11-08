//! 図種の判別（先頭キーワード dispatch）

/// mermaid の図種。ヘッダキーワードから判別する
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum DiagramKind {
    Sequence,
    Flowchart,
    Class,
    State,
    Er,
    Gantt,
    Pie,
    GitGraph,
    Journey,
    Mindmap,
    Timeline,
    Packet,
    Quadrant,
    C4,
    Unknown,
}

impl DiagramKind {
    /// tankan がレンダリングできる図種か
    pub fn is_supported(self) -> bool {
        matches!(
            self,
            Self::Sequence
                | Self::Flowchart
                | Self::Class
                | Self::State
                | Self::Er
                | Self::Gantt
                | Self::Pie
                | Self::Mindmap
                | Self::Timeline
                | Self::Packet
        )
    }
}

/// 図種を判別する。`%%` コメント・`%%{init}%%` ディレクティブ・
/// YAML frontmatter（`---` 区切り）をスキップした最初の実行行で判定する
pub(crate) fn detect(source: &str) -> DiagramKind {
    match header_line(source) {
        Some(line) => kind_of_header(line),
        None => DiagramKind::Unknown,
    }
}

/// ヘッダ行（図種キーワードを含む最初の実行行）を返す。
/// パーサ側と同じスキップ規則を共有する
pub(crate) fn header_line(source: &str) -> Option<&str> {
    let mut in_directive = false;
    let mut in_frontmatter = false;
    let mut first_content = true;

    for raw in source.lines() {
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
            }
            continue;
        }
        // frontmatter は最初の内容行が "---" のときのみ
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
        return Some(line);
    }
    None
}

fn kind_of_header(line: &str) -> DiagramKind {
    let keyword = line.split_whitespace().next().unwrap_or("");
    // "flowchart LR" のように方向指定が続くものは先頭トークンで判定
    match keyword {
        "sequenceDiagram" => DiagramKind::Sequence,
        "flowchart" | "graph" => DiagramKind::Flowchart,
        "classDiagram" | "classDiagram-v2" => DiagramKind::Class,
        "stateDiagram" | "stateDiagram-v2" => DiagramKind::State,
        "erDiagram" => DiagramKind::Er,
        "gantt" => DiagramKind::Gantt,
        "pie" => DiagramKind::Pie,
        "gitGraph" => DiagramKind::GitGraph,
        "journey" => DiagramKind::Journey,
        "mindmap" => DiagramKind::Mindmap,
        "timeline" => DiagramKind::Timeline,
        "packet" | "packet-beta" => DiagramKind::Packet,
        "quadrantChart" => DiagramKind::Quadrant,
        k if k.starts_with("C4") => DiagramKind::C4,
        _ => DiagramKind::Unknown,
    }
}

/// 半角空白・タブ・全角空白（U+3000）・CR を trim する
pub(crate) fn trim_line(line: &str) -> &str {
    line.trim_matches(|c: char| c == ' ' || c == '\t' || c == '\u{3000}' || c == '\r')
}

/// 行頭の語（最初の空白文字の手前まで）。戻り値は必ず `line` の先頭部分なので、
/// 各図種のパーサは `&line["classDef".len()..]` のように語の長さで残りを切り出せる。
/// `split_whitespace` で取ると、[`trim_line`] が外さない空白（垂直タブ・U+2028 等）が行頭に
/// あるときに語を読み飛ばし、切り出しが文字の境界の途中になって panic していた（Phase 92 の fuzz）。
/// 行頭がそうした空白なら空文字列を返す（どの語にも当たらない）
pub(crate) fn keyword_of(line: &str) -> &str {
    line.split(char::is_whitespace).next().unwrap_or("")
}

#[cfg(test)]
mod tests {
    use super::{DiagramKind, detect, keyword_of};

    #[test]
    fn 行頭の語は必ず行の先頭部分() {
        assert_eq!(keyword_of("state A {"), "state");
        assert_eq!(keyword_of("classDef\tx fill:#f00"), "classDef");
        assert_eq!(keyword_of("end"), "end");
        assert_eq!(keyword_of(""), "");
        // trim_line が外さない空白で始まる行は、どの語にも当たらない
        assert_eq!(keyword_of("\u{b}state A {"), "");
        assert_eq!(keyword_of("\u{2028}\u{2028}classDef x"), "");
    }

    /// 行頭に trim_line が外さない空白（U+2028 等の複数バイト）が並ぶ行で、各図種のパーサが
    /// 語の長さで残りを切り出して、文字の境界の途中で panic していた（Phase 92 の fuzz で
    /// state を見つけ、同じ書き方の class・flowchart・gantt でも再現した）。結果は問わない
    #[test]
    fn 行頭に複数バイトの空白がある行でも_panic_しない() {
        let lead = "\u{2028}\u{2028}\u{2028}";
        for (header, line) in [
            ("flowchart TD", "classDef a fill:#f00"),
            ("classDiagram", "class A"),
            ("sequenceDiagram", "participant A"),
            ("gantt", "title x"),
            ("erDiagram", "direction LR"),
            ("stateDiagram-v2", "state A"),
            ("stateDiagram-v2", "state A {"),
        ] {
            let source = format!("{header}\n{lead}{line}\n");
            let _ = crate::render_svg(&source, &crate::Options::default());
            let source = format!("{header}\n\u{85}{line}\n");
            let _ = crate::render_svg(&source, &crate::Options::default());
        }
    }

    #[test]
    fn 図種の判別() {
        assert_eq!(detect("sequenceDiagram\nA->>B: x"), DiagramKind::Sequence);
        assert_eq!(detect("flowchart LR\nA-->B"), DiagramKind::Flowchart);
        assert_eq!(detect("graph TD;\nA-->B"), DiagramKind::Flowchart);
        assert_eq!(detect("stateDiagram-v2\n[*] --> Still"), DiagramKind::State);
        assert_eq!(detect("erDiagram\nA ||--o{ B : has"), DiagramKind::Er);
        assert_eq!(detect("gantt\ntitle x"), DiagramKind::Gantt);
        assert_eq!(detect("pie\n\"a\": 1"), DiagramKind::Pie);
        assert_eq!(detect("packet\n0-15: \"Port\""), DiagramKind::Packet);
        assert_eq!(detect("packet-beta\n0-15: \"Port\""), DiagramKind::Packet);
        assert_eq!(detect("C4Context\n"), DiagramKind::C4);
        assert_eq!(detect("なにこれ\n"), DiagramKind::Unknown);
        assert_eq!(detect(""), DiagramKind::Unknown);
    }

    #[test]
    fn コメントとディレクティブを越えて判別する() {
        assert_eq!(
            detect("%% コメント\n%%{init: {'theme':'dark'}}%%\nsequenceDiagram\n"),
            DiagramKind::Sequence
        );
        // 複数行ディレクティブ
        assert_eq!(
            detect("%%{\n  init: { \"theme\": \"forest\" }\n}%%\nflowchart TD\n"),
            DiagramKind::Flowchart
        );
    }

    #[test]
    fn frontmatter_を越えて判別する() {
        assert_eq!(
            detect("---\ntitle: シーケンス\n---\nsequenceDiagram\nA->>B: x"),
            DiagramKind::Sequence
        );
    }

    #[test]
    fn is_supported_の対応図種() {
        assert!(DiagramKind::Sequence.is_supported());
        assert!(DiagramKind::Flowchart.is_supported());
        assert!(DiagramKind::Class.is_supported());
        assert!(DiagramKind::Er.is_supported());
        assert!(DiagramKind::Gantt.is_supported());
        assert!(DiagramKind::Pie.is_supported());
        assert!(DiagramKind::Mindmap.is_supported());
        assert!(DiagramKind::Timeline.is_supported());
        assert!(DiagramKind::Packet.is_supported());
        assert!(!DiagramKind::Journey.is_supported());
        assert!(!DiagramKind::Unknown.is_supported());
    }
}
