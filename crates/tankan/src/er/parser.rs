//! erDiagram の行指向パーサ。
//!
//! - リレーショントークンは「左基数 2 文字 ＋ 線種 2 文字 ＋ 右基数 2 文字」の
//!   固定 6 文字（例: `||--o{`, `}o..o|`）。ラベルの `:` 分割を**先に**行う
//!   （基数トークンに `|` が含まれるため、雑な分割は壊れる）
//! - エンティティ名は引用符付き（空白可）とエイリアス `E[表示名]` に対応

use std::collections::HashMap;

use crate::common::style::StyleCollector;
use crate::common::text::{decode_entities, split_br_lines};
use crate::er::model::{Attribute, Cardinality, Entity, ErDiagram, Relation};
use crate::error::Error;
use crate::kind::trim_line;

pub(crate) fn parse(source: &str) -> Result<ErDiagram, Error> {
    let mut p = ErParser::default();

    let mut in_directive = false;
    let mut in_frontmatter = false;
    let mut seen_header = false;
    let mut first_content = true;
    let mut attr_block: Option<usize> = None; // 属性ブロック中のエンティティ

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

        if !seen_header {
            if line == "erDiagram" {
                seen_header = true;
                continue;
            }
            return Err(Error::Parse {
                line: line_no,
                message: "erDiagram ヘッダがありません".to_string(),
            });
        }

        // 属性ブロック中
        if let Some(entity) = attr_block {
            if line == "}" {
                attr_block = None;
            } else {
                let attr = parse_attribute(line, line_no)?;
                p.diagram.entities[entity].attributes.push(attr);
            }
            continue;
        }

        let keyword = line.split_whitespace().next().unwrap_or("");
        match keyword {
            "classDef" => {
                p.styles.class_def(trim_line(&line["classDef".len()..]));
                continue;
            }
            "class" => {
                p.styles.apply_class(trim_line(&line["class".len()..]));
                continue;
            }
            "style" => {
                p.styles.apply_style(trim_line(&line["style".len()..]));
                continue;
            }
            // v1 では direction は受理して無視（ER は常に TB レイアウト）
            "direction" => continue,
            _ => {}
        }

        // `ENTITY {` → 属性ブロック開始
        if let Some(rest) = line.strip_suffix('{') {
            let rest = trim_line(rest);
            if !rest.is_empty() && !rest.contains("--") && !rest.contains("..") {
                attr_block = Some(p.intern_ref(rest, line_no)?);
                continue;
            }
        }

        // リレーション or 単独エンティティ宣言
        if find_relation_token(line).is_some() {
            p.relation(line, line_no)?;
        } else {
            p.intern_ref(line, line_no)?;
        }
    }

    if attr_block.is_some() {
        return Err(Error::Parse {
            line: source.lines().count(),
            message: "閉じられていない属性ブロックがあります（} 不足）".to_string(),
        });
    }

    // classDef / class / `:::` / style を各エンティティへ配る
    if !p.styles.is_empty() {
        let mut names = vec![String::new(); p.diagram.entities.len()];
        for (name, &idx) in &p.index {
            names[idx] = name.clone();
        }
        for (entity, name) in p.diagram.entities.iter_mut().zip(&names) {
            if let Some(style) = p.styles.resolve(name) {
                entity.style = Some(style);
            }
        }
    }

    Ok(p.diagram)
}

#[derive(Default)]
struct ErParser {
    diagram: ErDiagram,
    index: HashMap<String, usize>,
    /// classDef / class / `:::` / style を蓄積し、パース後に各エンティティへ配る
    styles: StyleCollector,
}

impl ErParser {
    fn intern(&mut self, name: &str, display: Option<String>) -> usize {
        let id = match self.index.get(name) {
            Some(&id) => id,
            None => {
                let id = self.diagram.entities.len();
                self.diagram.entities.push(Entity {
                    display: name.to_string(),
                    attributes: Vec::new(),
                    style: None,
                });
                self.index.insert(name.to_string(), id);
                id
            }
        };
        if let Some(display) = display {
            self.diagram.entities[id].display = display;
        }
        id
    }

    /// エンティティ参照（末尾に `:::class` インラインクラス可）を intern する
    fn intern_ref(&mut self, text: &str, line_no: usize) -> Result<usize, Error> {
        let (text, inline) = split_inline_class(text);
        let (name, display) = parse_entity_name(text, line_no)?;
        let id = self.intern(&name, display);
        if let Some(cls) = inline {
            self.styles.add_inline(&name, cls);
        }
        Ok(id)
    }

    /// `A ||--o{ B : label`（両端に `A:::cls` インラインクラス可）
    fn relation(&mut self, line: &str, line_no: usize) -> Result<(), Error> {
        // ラベル分割を先に（基数トークンの `|` と衝突しないように、
        // `:` は左辺のリレーション部より後ろにしか現れない前提で最後の分割にする）。
        // `:::` インラインクラスの `:` はラベル区切りと誤認しない
        let (left, label) = match split_label(line) {
            (l, Some(t)) => (trim_line(l), split_text(t)),
            (l, None) => (trim_line(l), Vec::new()),
        };

        let Some((token_pos, token)) = find_relation_token(left) else {
            return Err(Error::Parse {
                line: line_no,
                message: "リレーションとして解釈できません".to_string(),
            });
        };

        let from_str = trim_line(&left[..token_pos]);
        let to_str = trim_line(&left[token_pos + token.len()..]);
        let from = self.intern_ref(from_str, line_no)?;
        let to = self.intern_ref(to_str, line_no)?;

        let (from_card, identifying, to_card) = decode_relation_token(token, line_no)?;
        self.diagram.relations.push(Relation {
            from,
            to,
            from_card,
            to_card,
            identifying,
            label,
        });
        Ok(())
    }
}

/// リレーショントークン（6 文字）を探す。戻り値: (位置, トークン)
fn find_relation_token(line: &str) -> Option<(usize, &str)> {
    const LEFT: [&str; 4] = ["|o", "||", "}o", "}|"];
    const LINE: [&str; 2] = ["--", ".."];
    const RIGHT: [&str; 4] = ["o|", "||", "o{", "|{"];
    let bytes = line.as_bytes();
    for i in 0..bytes.len().saturating_sub(5) {
        if !line.is_char_boundary(i) {
            continue;
        }
        let window = &line[i..];
        for l in LEFT {
            if !window.starts_with(l) {
                continue;
            }
            let after_l = &window[2..];
            for m in LINE {
                if !after_l.starts_with(m) {
                    continue;
                }
                let after_m = &after_l[2..];
                for r in RIGHT {
                    if after_m.starts_with(r) {
                        return Some((i, &line[i..i + 6]));
                    }
                }
            }
        }
    }
    None
}

fn decode_relation_token(
    token: &str,
    line_no: usize,
) -> Result<(Cardinality, bool, Cardinality), Error> {
    let from_card = match &token[0..2] {
        "|o" => Cardinality::ZeroOne,
        "||" => Cardinality::One,
        "}o" => Cardinality::ZeroMany,
        "}|" => Cardinality::OneMany,
        other => {
            return Err(Error::Parse {
                line: line_no,
                message: format!("不明な基数: {other}"),
            });
        }
    };
    let identifying = &token[2..4] == "--";
    let to_card = match &token[4..6] {
        "o|" => Cardinality::ZeroOne,
        "||" => Cardinality::One,
        "o{" => Cardinality::ZeroMany,
        "|{" => Cardinality::OneMany,
        other => {
            return Err(Error::Parse {
                line: line_no,
                message: format!("不明な基数: {other}"),
            });
        }
    };
    Ok((from_card, identifying, to_card))
}

/// エンティティ名（`NAME` / `"空白入り 名前"` / `NAME[表示名]`）
fn parse_entity_name(text: &str, line_no: usize) -> Result<(String, Option<String>), Error> {
    let text = trim_line(text);
    if text.is_empty() {
        return Err(Error::Parse {
            line: line_no,
            message: "エンティティ名がありません".to_string(),
        });
    }
    if let Some(r) = text.strip_prefix('"') {
        let Some((name, rest)) = r.split_once('"') else {
            return Err(Error::Parse {
                line: line_no,
                message: "エンティティ名の引用符が閉じられていません".to_string(),
            });
        };
        if !trim_line(rest).is_empty() {
            return Err(Error::Parse {
                line: line_no,
                message: "エンティティ名の後に解釈できない文字があります".to_string(),
            });
        }
        return Ok((name.to_string(), None));
    }
    if let Some((name, alias)) = text.split_once('[') {
        let alias = alias.strip_suffix(']').ok_or_else(|| Error::Parse {
            line: line_no,
            message: "エイリアスの ] が閉じられていません".to_string(),
        })?;
        return Ok((
            trim_line(name).to_string(),
            Some(alias.trim_matches('"').to_string()),
        ));
    }
    if text.contains(' ') {
        return Err(Error::Parse {
            line: line_no,
            message: format!("エンティティ名として解釈できません: `{text}`"),
        });
    }
    Ok((text.to_string(), None))
}

/// 属性行: `type name [PK|FK|UK[, ...]] ["comment"]`
fn parse_attribute(line: &str, line_no: usize) -> Result<Attribute, Error> {
    // 末尾の引用符コメントを先に切り出す
    let (rest, comment) = match line.split_once('"') {
        Some((before, after)) => {
            let comment = after.strip_suffix('"').unwrap_or(after);
            (trim_line(before), Some(comment.to_string()))
        }
        None => (line, None),
    };
    let mut it = rest.split_whitespace();
    let (Some(type_name), Some(name)) = (it.next(), it.next()) else {
        return Err(Error::Parse {
            line: line_no,
            message: "属性は `型 名前 [キー] [\"コメント\"]` の形にしてください".to_string(),
        });
    };
    let keys: Vec<String> = it
        .collect::<Vec<_>>()
        .join(" ")
        .split(',')
        .map(|k| trim_line(k).to_string())
        .filter(|k| !k.is_empty())
        .collect();
    for key in &keys {
        if !matches!(key.as_str(), "PK" | "FK" | "UK") {
            return Err(Error::Parse {
                line: line_no,
                message: format!("不明な属性キー: {key}（PK/FK/UK のみ）"),
            });
        }
    }
    Ok(Attribute {
        type_name: type_name.to_string(),
        name: name.to_string(),
        keys,
        comment,
    })
}

fn split_text(text: &str) -> Vec<String> {
    let text = trim_line(text);
    let text = text.trim_matches('"');
    split_br_lines(&decode_entities(text))
}

/// エンティティ参照の末尾 `:::class` を (素の参照, クラス名) に分ける。
/// `:::` が無ければ (元テキスト, None)
fn split_inline_class(text: &str) -> (&str, Option<&str>) {
    match text.split_once(":::") {
        Some((base, cls)) => {
            let cls = cls.trim();
            (base.trim(), (!cls.is_empty()).then_some(cls))
        }
        None => (text.trim(), None),
    }
}

/// リレーション行を (リレーション部, ラベル) に分ける。ラベル区切りは単独 `:`。
/// `:::` インラインクラス（3 連コロン）はラベル区切りと誤認しない
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

#[cfg(test)]
mod tests {
    use super::parse;
    use crate::er::model::Cardinality;

    fn er(body: &str) -> String {
        format!("erDiagram\n{body}")
    }

    #[test]
    fn 基本のリレーション() {
        let d = parse(&er("CUSTOMER ||--o{ ORDER : places")).unwrap();
        assert_eq!(d.entities.len(), 2);
        let r = &d.relations[0];
        assert_eq!(r.from_card, Cardinality::One);
        assert_eq!(r.to_card, Cardinality::ZeroMany);
        assert!(r.identifying);
        assert_eq!(r.label, ["places"]);
    }

    #[test]
    fn 全基数と線種の組合せ() {
        let cases = [
            (
                "A |o--o| B : x",
                Cardinality::ZeroOne,
                true,
                Cardinality::ZeroOne,
            ),
            ("A ||--|| B : x", Cardinality::One, true, Cardinality::One),
            (
                "A }o--o{ B : x",
                Cardinality::ZeroMany,
                true,
                Cardinality::ZeroMany,
            ),
            (
                "A }|--|{ B : x",
                Cardinality::OneMany,
                true,
                Cardinality::OneMany,
            ),
            (
                "A ||..o{ B : x",
                Cardinality::One,
                false,
                Cardinality::ZeroMany,
            ),
            (
                "A }o..o| B : x",
                Cardinality::ZeroMany,
                false,
                Cardinality::ZeroOne,
            ),
        ];
        for (src, from, identifying, to) in cases {
            let d = parse(&er(src)).unwrap_or_else(|e| panic!("{src}: {e}"));
            let r = &d.relations[0];
            assert_eq!(
                (r.from_card, r.identifying, r.to_card),
                (from, identifying, to),
                "{src}"
            );
        }
    }

    #[test]
    fn 属性ブロックとキー() {
        let d = parse(&er(
            "CAR {\n  string registrationNumber PK\n  string make\n  string model\n  string[] parts\n  int owner FK, UK \"所有者\"\n}",
        ))
        .unwrap();
        let car = &d.entities[0];
        assert_eq!(car.attributes.len(), 5);
        assert_eq!(car.attributes[0].keys, ["PK"]);
        assert_eq!(car.attributes[3].type_name, "string[]");
        assert_eq!(car.attributes[4].keys, ["FK", "UK"]);
        assert_eq!(car.attributes[4].comment.as_deref(), Some("所有者"));
    }

    #[test]
    fn エイリアスと引用符名() {
        let d = parse(&er(
            "p[人物] {\n  string name\n}\n\"注文 履歴\" ||--o{ p : has",
        ))
        .unwrap();
        assert_eq!(d.entities[0].display, "人物");
        assert_eq!(d.entities[1].display, "注文 履歴");
    }

    #[test]
    fn 不明なキーはエラー() {
        let err = parse(&er("A {\n  int x QK\n}")).unwrap_err();
        assert!(err.to_string().contains("QK"), "{err}");
    }

    /// エンティティ名から解決済みスタイルを取り出す
    fn entity_style(d: &super::ErDiagram, name: &str) -> crate::common::style::Style {
        let e = d
            .entities
            .iter()
            .find(|e| e.display == name)
            .unwrap_or_else(|| panic!("エンティティ {name} が見つかりません"));
        e.style.clone().unwrap_or_default()
    }

    #[test]
    fn style_文と_classdef_class_が適用される() {
        // style 文（個別）
        let d = parse(&er(
            "CUSTOMER ||--o{ ORDER : places\nstyle CUSTOMER fill:#f9f,stroke:#333",
        ))
        .unwrap();
        let s = entity_style(&d, "CUSTOMER");
        assert_eq!(s.fill.as_deref(), Some("#f9f"));
        assert_eq!(s.stroke.as_deref(), Some("#333"));
        assert!(
            d.entities
                .iter()
                .find(|e| e.display == "ORDER")
                .unwrap()
                .style
                .is_none(),
            "無指定はスタイルなし"
        );
    }

    #[test]
    fn classdef_default_とインラインクラスが効く() {
        let d = parse(&er(
            "CUSTOMER:::vip ||--o{ ORDER : places\nclassDef default fill:#eee\nclassDef vip fill:#fd8",
        ))
        .unwrap();
        // default は全エンティティ、vip は CUSTOMER のみ（default を上書き）
        assert_eq!(entity_style(&d, "ORDER").fill.as_deref(), Some("#eee"));
        assert_eq!(entity_style(&d, "CUSTOMER").fill.as_deref(), Some("#fd8"));
    }

    #[test]
    fn class_文で複数エンティティへ適用できる() {
        let d = parse(&er("A ||--o{ B : r\nclassDef hot fill:#f96\nclass A,B hot")).unwrap();
        assert_eq!(entity_style(&d, "A").fill.as_deref(), Some("#f96"));
        assert_eq!(entity_style(&d, "B").fill.as_deref(), Some("#f96"));
    }

    #[test]
    fn 構文エラーの検出() {
        assert!(parse(&er("A {\n int x")).is_err(), "閉じ括弧なし");
        assert!(parse(&er("A -- B")).is_err(), "リレーショントークンなし");
        assert!(parse("A ||--o{ B : x").is_err(), "ヘッダなし");
    }
}
