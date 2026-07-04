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
    Quadrant,
    C4,
    Unknown,
}

impl DiagramKind {
    /// tankan がレンダリングできる図種か（M1 時点では sequence のみ）
    pub fn is_supported(self) -> bool {
        matches!(self, Self::Sequence)
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
        "quadrantChart" => DiagramKind::Quadrant,
        k if k.starts_with("C4") => DiagramKind::C4,
        _ => DiagramKind::Unknown,
    }
}

/// 半角空白・タブ・全角空白（U+3000）・CR を trim する
pub(crate) fn trim_line(line: &str) -> &str {
    line.trim_matches(|c: char| c == ' ' || c == '\t' || c == '\u{3000}' || c == '\r')
}

#[cfg(test)]
mod tests {
    use super::{DiagramKind, detect};

    #[test]
    fn 図種の判別() {
        assert_eq!(detect("sequenceDiagram\nA->>B: x"), DiagramKind::Sequence);
        assert_eq!(detect("flowchart LR\nA-->B"), DiagramKind::Flowchart);
        assert_eq!(detect("graph TD;\nA-->B"), DiagramKind::Flowchart);
        assert_eq!(detect("stateDiagram-v2\n[*] --> Still"), DiagramKind::State);
        assert_eq!(detect("erDiagram\nA ||--o{ B : has"), DiagramKind::Er);
        assert_eq!(detect("gantt\ntitle x"), DiagramKind::Gantt);
        assert_eq!(detect("pie\n\"a\": 1"), DiagramKind::Pie);
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
    fn is_supported_は_sequence_のみ() {
        assert!(DiagramKind::Sequence.is_supported());
        assert!(!DiagramKind::Flowchart.is_supported());
        assert!(!DiagramKind::Unknown.is_supported());
    }
}
