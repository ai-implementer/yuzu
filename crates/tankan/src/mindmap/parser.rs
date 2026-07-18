//! mindmap の行指向パーサ。
//!
//! 対応構文（mermaid 互換）:
//! - ヘッダ: `mindmap`
//! - ノード行: インデント階層でツリーを表す（最初の内容行がルート、以降は
//!   「直前より深ければ子・同じなら兄弟・浅ければ祖先の兄弟」）
//! - 形状: 無印 / `[四角]` / `(角丸)` / `((円))` / `))バン((` / `)雲(` / `{{六角形}}`。
//!   `root((中心))` のような形状開始記号より前の id トークンは表示に使わず捨てる
//! - `::icon(...)` 行・`:::class` 行は受理して無視する（mermaid 公式例に含まれ
//!   corpus 全件受理と衝突するため。アイコン・クラス装飾は Phase 27 では非対応）
//! - markdown 文字列（`"` ＋ バッククォート）は行指向で扱えないため UnsupportedSyntax
//! - `%%` コメント・`%%{init}%%` ディレクティブ・YAML frontmatter（title を拾う）
//!
//! ⚠️ インデントは `trim_line` が潰すため、**生の行から先に計測**する。
//! 換算は半角空白 = 1・タブ = 4・全角空白 = 2（視覚カラム近似）。mermaid は
//! 相対比較しかしないので、単一種のインデントで書かれた文書では換算値の差は
//! 挙動に影響しない

use crate::error::Error;
use crate::kind::trim_line;
use crate::mindmap::model::{MindmapDiagram, Node, NodeShape};

pub(crate) fn parse(source: &str) -> Result<MindmapDiagram, Error> {
    let mut diagram = MindmapDiagram::default();

    let mut in_directive = false;
    let mut in_frontmatter = false;
    let mut seen_header = false;
    let mut first_content = true;
    // (インデント量, ノード添字) のスタック。top が「直前に確定したノード」
    let mut stack: Vec<(usize, usize)> = Vec::new();

    for (idx, raw) in source.lines().enumerate() {
        let line_no = idx + 1;
        let indent = indent_of(raw);
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
                diagram.title = Some(t.trim().to_string());
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
            if line != "mindmap" {
                return Err(Error::Parse {
                    line: line_no,
                    message: "mindmap ヘッダがありません".to_string(),
                });
            }
            seen_header = true;
            continue;
        }

        // アイコン・クラス装飾行は受理して無視（スタックは変えない）
        if line.starts_with("::icon(") || line.starts_with(":::") {
            continue;
        }

        let (shape, text) = parse_shape(line, line_no)?;
        let node_idx = diagram.nodes.len();
        diagram.nodes.push(Node {
            text,
            shape,
            children: Vec::new(),
        });

        if diagram.nodes.len() == 1 {
            // 最初の内容行 = ルート（インデントは任意）
            stack.push((indent, node_idx));
            continue;
        }
        // 深ければ子・同じなら兄弟・浅ければ祖先へ pop（中間値も最近傍祖先の子）
        while stack.last().is_some_and(|&(i, _)| i >= indent) {
            stack.pop();
        }
        let Some(&(_, parent)) = stack.last() else {
            return Err(Error::Parse {
                line: line_no,
                message: "ルートノードは 1 つだけです（ルートと同じ深さの行があります）"
                    .to_string(),
            });
        };
        diagram.nodes[parent].children.push(node_idx);
        stack.push((indent, node_idx));
    }

    if !seen_header {
        return Err(Error::Parse {
            line: 1,
            message: "mindmap ヘッダがありません".to_string(),
        });
    }
    if diagram.nodes.is_empty() {
        return Err(Error::Parse {
            line: 1,
            message: "ノードがありません".to_string(),
        });
    }
    Ok(diagram)
}

/// 生の行の先頭からインデント量を計測する（半角空白 = 1・タブ = 4・全角空白 = 2）
fn indent_of(raw: &str) -> usize {
    let mut n = 0;
    for c in raw.chars() {
        n += match c {
            ' ' => 1,
            '\t' => 4,
            '\u{3000}' => 2,
            _ => break,
        };
    }
    n
}

/// ノード行から (形状, テキスト) を取り出す。
/// 形状開始記号（`[` `(` `)` `{{`）より前の id トークンは捨てる。
/// 開始記号があるのに対応する終了記号で終わらない行は全体をプレーンテキスト扱い
fn parse_shape(line: &str, line_no: usize) -> Result<(NodeShape, String), Error> {
    // 最初に現れる形状開始記号の位置（`{` は `{{` のときだけ開始記号）
    let open_at = line.char_indices().find_map(|(i, c)| match c {
        '[' | '(' | ')' => Some(i),
        '{' if line[i..].starts_with("{{") => Some(i),
        _ => None,
    });
    let Some(at) = open_at else {
        return Ok((NodeShape::Default, clean_text(line, line_no)?));
    };
    let rest = &line[at..];
    // 長い記号から順に前後対照で照合する（`((` を `(` より先に見る）
    let table: [(&str, &str, NodeShape); 6] = [
        ("((", "))", NodeShape::Circle),
        ("))", "((", NodeShape::Bang),
        ("{{", "}}", NodeShape::Hexagon),
        ("(", ")", NodeShape::Rounded),
        (")", "(", NodeShape::Cloud),
        ("[", "]", NodeShape::Square),
    ];
    for (open, close, shape) in table {
        if let Some(inner) = rest.strip_prefix(open) {
            if let Some(inner) = inner.strip_suffix(close) {
                return Ok((shape, clean_text(inner.trim(), line_no)?));
            }
        }
    }
    // 開始記号はあるが形として閉じていない → 行全体をプレーンテキスト扱い
    Ok((NodeShape::Default, clean_text(line, line_no)?))
}

/// ノードテキストの最終整形。markdown 文字列（`"` ＋ バッククォート）は
/// 複数行にまたがり行指向で扱えないため UnsupportedSyntax でフォールバックさせる
fn clean_text(text: &str, line_no: usize) -> Result<String, Error> {
    if text.starts_with('"') {
        return Err(Error::UnsupportedSyntax {
            line: line_no,
            construct: "markdown 文字列".to_string(),
        });
    }
    Ok(crate::common::text::decode_entities(text))
}

#[cfg(test)]
mod tests {
    use super::parse;
    use crate::mindmap::model::NodeShape;

    #[test]
    fn インデント階層がツリーになる() {
        let d = parse("mindmap\n  root((中心))\n    A\n      A1\n      A2\n    B\n").unwrap();
        assert_eq!(d.nodes.len(), 5);
        assert_eq!(d.nodes[0].text, "中心");
        assert_eq!(d.nodes[0].shape, NodeShape::Circle);
        assert_eq!(d.nodes[0].children, [1, 4], "root の子は A と B");
        assert_eq!(d.nodes[1].children, [2, 3], "A の子は A1 と A2");
    }

    #[test]
    fn 全形状をパースできる() {
        let d = parse(
            "mindmap\n  root\n    a[四角]\n    b(角丸)\n    c((円))\n    d))バン((\n    e)雲(\n    f{{六角}}\n    プレーン\n",
        )
        .unwrap();
        use NodeShape::*;
        let shapes: Vec<NodeShape> = d.nodes[1..].iter().map(|n| n.shape).collect();
        assert_eq!(
            shapes,
            [Square, Rounded, Circle, Bang, Cloud, Hexagon, Default]
        );
        assert_eq!(d.nodes[1].text, "四角", "id は捨てられテキストだけ残る");
    }

    #[test]
    fn icon_と_class_行はスキップされツリーを変えない() {
        let d = parse(
            "mindmap\n  root\n    A\n    ::icon(fa fa-book)\n    :::urgent large\n      A1\n",
        )
        .unwrap();
        assert_eq!(d.nodes.len(), 3);
        assert_eq!(d.nodes[1].children, [2], "A1 は A の子のまま");
    }

    #[test]
    fn 複数ルートはエラー() {
        let err = parse("mindmap\n  root1\n  root2\n").unwrap_err();
        assert!(!err.is_unsupported(), "Parse エラーであること: {err}");
    }

    #[test]
    fn タブと全角空白のインデントも扱える() {
        // タブ = 4 なので「タブ 1 個」は「空白 2 個」より深い
        let d = parse("mindmap\nroot\n\tA\n\t\tB\n").unwrap();
        assert_eq!(d.nodes[1].children, [2]);
        let d2 = parse("mindmap\nroot\n\u{3000}A\n\u{3000}\u{3000}B\n").unwrap();
        assert_eq!(d2.nodes[1].children, [2]);
    }

    #[test]
    fn 中間値への_dedent_は最近傍祖先の子になる() {
        // A1 は indent 8、B は indent 3（root=0 と A=4 の中間）→ root の子
        let d = parse("mindmap\nroot\n    A\n        A1\n   B\n").unwrap();
        assert_eq!(d.nodes[0].children, [1, 3]);
    }

    #[test]
    fn markdown_文字列は_unsupported() {
        let err = parse("mindmap\n  root(\"`**太字**`\")\n").unwrap_err();
        assert!(err.is_unsupported(), "{err}");
    }

    #[test]
    fn ヘッダ無しとノードゼロはエラー() {
        assert!(parse("root\n").is_err());
        assert!(parse("mindmap\n").is_err());
    }
}
