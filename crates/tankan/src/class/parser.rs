//! classDiagram の行指向パーサ。
//!
//! - 関係トークンは「左マーカー？ ＋ 線種（`--`/`..`）＋ 右マーカー？」。
//!   線種の位置を軸に走査し、`o` マーカーだけはクラス名末尾の `o` と衝突するため
//!   語境界（前後が空白/端）を要求する
//! - `:` ラベルの分割は関係トークン検出を**先に**行う（メンバー代入 `A : +int x`
//!   と関係 `A <|-- B : label` を取り違えないため）
//! - ジェネリクス `Box~T~` は表示名 `Box<T>` へ変換する（intern キーは生の名前）

use std::collections::HashMap;

use crate::class::model::{Class, ClassDiagram, Marker, Relation};
use crate::common::style::StyleCollector;
use crate::common::text::{decode_entities, split_br_lines};
use crate::error::Error;
use crate::kind::trim_line;

pub(crate) fn parse(source: &str) -> Result<ClassDiagram, Error> {
    let mut p = ClassParser::default();

    let mut in_directive = false;
    let mut in_frontmatter = false;
    let mut seen_header = false;
    let mut first_content = true;
    let mut body_block: Option<usize> = None; // クラス本体 `{ }` の中

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
            if line == "classDiagram" || line == "classDiagram-v2" {
                seen_header = true;
                continue;
            }
            return Err(Error::Parse {
                line: line_no,
                message: "classDiagram ヘッダがありません".to_string(),
            });
        }

        // クラス本体ブロック中
        if let Some(cid) = body_block {
            if line == "}" {
                body_block = None;
            } else {
                p.member_line(cid, line, line_no)?;
            }
            continue;
        }

        p.statement(line, line_no, &mut body_block)?;
    }

    if body_block.is_some() {
        return Err(Error::Parse {
            line: source.lines().count(),
            message: "閉じられていないクラス本体があります（} 不足）".to_string(),
        });
    }

    // classDef / cssClass / `:::` / style を各クラスへ配る
    if !p.styles.is_empty() {
        let mut names = vec![String::new(); p.diagram.classes.len()];
        for (name, &idx) in &p.index {
            names[idx] = name.clone();
        }
        for (class, name) in p.diagram.classes.iter_mut().zip(&names) {
            if let Some(style) = p.styles.resolve(name) {
                class.style = Some(style);
            }
        }
    }

    Ok(p.diagram)
}

#[derive(Default)]
struct ClassParser {
    diagram: ClassDiagram,
    index: HashMap<String, usize>,
    /// classDef / cssClass / `:::` / style を蓄積し、パース後に各クラスへ配る
    styles: StyleCollector,
}

impl ClassParser {
    /// 生の名前でクラスを取得（なければ作成）。表示名はジェネリクス変換する。
    /// 末尾 `:::class` のインラインクラス指定はここで剥がして登録する
    fn intern(&mut self, name: &str) -> usize {
        let name = self.take_inline(name);
        if let Some(&id) = self.index.get(name) {
            return id;
        }
        let id = self.diagram.classes.len();
        self.diagram.classes.push(Class {
            display: convert_generics(name),
            ..Class::default()
        });
        self.index.insert(name.to_string(), id);
        id
    }

    /// `name:::class` の末尾 `:::class` を剥がしてクラス登録し、素の名前を返す。
    /// 登録キーは index と同じ生の名前（ジェネリクス変換前）
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

    fn statement(
        &mut self,
        line: &str,
        line_no: usize,
        body_block: &mut Option<usize>,
    ) -> Result<(), Error> {
        let keyword = line.split_whitespace().next().unwrap_or("");
        // スタイル構文。`class` は宣言キーワードなので、複数適用は `cssClass`（Mermaid 準拠）
        match keyword {
            "classDef" => {
                self.styles.class_def(trim_line(&line["classDef".len()..]));
                return Ok(());
            }
            "cssClass" => {
                // Mermaid は ids を引用符で囲む: `cssClass "A,B" name`。引用符は除去する
                let rest = trim_line(&line["cssClass".len()..]).replace('"', "");
                self.styles.apply_class(&rest);
                return Ok(());
            }
            "style" => {
                self.styles.apply_style(trim_line(&line["style".len()..]));
                return Ok(());
            }
            _ => {}
        }
        // 未対応構文（note・click・namespace 等）は静かにフォールバックさせる
        if matches!(
            keyword,
            "note" | "click" | "callback" | "call" | "link" | "namespace"
        ) {
            return Err(Error::UnsupportedSyntax {
                line: line_no,
                construct: keyword.to_string(),
            });
        }
        // direction は受理して無視（レイアウトは常に TB。er に合わせる）
        if keyword == "direction" {
            return Ok(());
        }

        // クラス宣言・本体ブロック開始
        if keyword == "class" {
            return self.class_decl(trim_line(&line["class".len()..]), line_no, body_block);
        }

        // 単独アノテーション行 `<<interface>> Shape`
        if line.starts_with("<<") {
            let (anno, name) = parse_annotation_line(line, line_no)?;
            let id = self.intern(&name);
            self.diagram.classes[id].annotation = Some(anno);
            return Ok(());
        }

        // 関係かメンバー代入かは、`:::` を跨がない最初の `:` の左側で判定する
        let (head, member) = split_label(line);
        let head = trim_line(head);
        if find_relation_token(head).is_some() {
            return self.relation(line, line_no);
        }

        // メンバー代入 `Class : member`
        if let Some(member) = member {
            if head.is_empty() {
                return Err(Error::Parse {
                    line: line_no,
                    message: "メンバーの所属クラス名がありません".to_string(),
                });
            }
            let id = self.intern(head);
            self.add_member(id, trim_line(member));
            return Ok(());
        }

        // 裸のクラス宣言
        if line.contains(char::is_whitespace) {
            return Err(Error::Parse {
                line: line_no,
                message: format!("文として解釈できません: `{line}`"),
            });
        }
        self.intern(line);
        Ok(())
    }

    /// `class Foo` / `class Foo { ` / `class Box~T~`
    fn class_decl(
        &mut self,
        rest: &str,
        line_no: usize,
        body_block: &mut Option<usize>,
    ) -> Result<(), Error> {
        let (name, opens) = match rest.strip_suffix('{') {
            Some(r) => (trim_line(r), true),
            None => (rest, false),
        };
        if name.is_empty() {
            return Err(Error::Parse {
                line: line_no,
                message: "クラス名がありません".to_string(),
            });
        }
        if name.contains(char::is_whitespace) {
            return Err(Error::Parse {
                line: line_no,
                message: format!("クラス名として解釈できません: `{name}`"),
            });
        }
        let id = self.intern(name);
        if opens {
            *body_block = Some(id);
        }
        Ok(())
    }

    /// クラス本体 `{ }` 内の 1 行（アノテーション or メンバー）
    fn member_line(&mut self, cid: usize, line: &str, line_no: usize) -> Result<(), Error> {
        if line.starts_with("<<") {
            let anno = line
                .strip_prefix("<<")
                .and_then(|s| s.strip_suffix(">>"))
                .map(trim_line);
            match anno {
                Some(a) if !a.is_empty() => {
                    self.diagram.classes[cid].annotation = Some(a.to_string());
                    Ok(())
                }
                _ => Err(Error::Parse {
                    line: line_no,
                    message: "アノテーションは `<<種別>>` の形にしてください".to_string(),
                }),
            }
        } else {
            // `Class : member` 形式で本体内に書かれた場合の `:` も許容する
            let member = line
                .split_once(':')
                .map(|(_, m)| trim_line(m))
                .unwrap_or(line);
            self.add_member(cid, member);
            Ok(())
        }
    }

    /// メンバー行を属性/メソッドに振り分けて追加する（`(` を含めばメソッド）
    fn add_member(&mut self, cid: usize, member: &str) {
        let text = convert_generics(member);
        let class = &mut self.diagram.classes[cid];
        if member.contains('(') {
            class.methods.push(text);
        } else {
            class.attributes.push(text);
        }
    }

    /// `A "1" <|-- "many" B : label`（両端に `A:::cls` インラインクラス可）
    fn relation(&mut self, line: &str, line_no: usize) -> Result<(), Error> {
        // ラベル区切りは `:::` インラインクラスを跨がない最初の単独 `:`
        let (main, label) = match split_label(line) {
            (m, Some(t)) => (trim_line(m), split_text(t)),
            (m, None) => (trim_line(m), Vec::new()),
        };
        let Some(tok) = find_relation_token(main) else {
            return Err(Error::Parse {
                line: line_no,
                message: "関係として解釈できません".to_string(),
            });
        };
        let before = trim_line(&main[..tok.start]);
        let after = trim_line(&main[tok.end..]);
        let (from_name, from_card) = parse_end(before, true, line_no)?;
        let (to_name, to_card) = parse_end(after, false, line_no)?;
        let from = self.intern(&from_name);
        let to = self.intern(&to_name);
        self.diagram.relations.push(Relation {
            from,
            to,
            from_marker: tok.left,
            to_marker: tok.right,
            dashed: tok.dashed,
            label,
            from_card,
            to_card,
        });
        Ok(())
    }
}

/// 関係トークンの解析結果（位置は `main` 内のバイト範囲）
struct RelToken {
    start: usize,
    end: usize,
    left: Marker,
    right: Marker,
    dashed: bool,
}

/// 関係トークンを走査する。引用符（多重度）の中はスキップする
fn find_relation_token(s: &str) -> Option<RelToken> {
    let bytes = s.as_bytes();
    let mut in_quote = false;
    let mut i = 0;
    while i < s.len() {
        if !s.is_char_boundary(i) {
            i += 1;
            continue;
        }
        if bytes[i] == b'"' {
            in_quote = !in_quote;
            i += 1;
            continue;
        }
        if !in_quote {
            if let Some(tok) = match_token_at(s, i) {
                return Some(tok);
            }
        }
        i += 1;
    }
    None
}

/// 位置 i から関係トークン（左マーカー？ ＋ 線種 ＋ 右マーカー？）の一致を試みる
fn match_token_at(s: &str, i: usize) -> Option<RelToken> {
    let rest = &s[i..];
    // 左マーカー（長い順。`o` だけはクラス名末尾との衝突回避に語境界を要求）
    let (left, after_left) = if let Some(r) = rest.strip_prefix("<|") {
        (Marker::Triangle, r)
    } else if let Some(r) = rest.strip_prefix('*') {
        (Marker::DiamondFilled, r)
    } else if let Some(r) = rest.strip_prefix('o') {
        let prev_boundary = i == 0 || s[..i].chars().next_back().is_some_and(char::is_whitespace);
        if !prev_boundary {
            return None;
        }
        (Marker::DiamondHollow, r)
    } else if let Some(r) = rest.strip_prefix('<') {
        (Marker::Arrow, r)
    } else {
        (Marker::None, rest)
    };

    // 線種（必須。実線 `--` でなければ点線 `..` を要求）
    let (dashed, after_line) = if let Some(r) = after_left.strip_prefix("--") {
        (false, r)
    } else {
        (true, after_left.strip_prefix("..")?)
    };

    // 右マーカー（同様に `o` は後続の語境界を要求）
    let (right, after_right) = if let Some(r) = after_line.strip_prefix("|>") {
        (Marker::Triangle, r)
    } else if let Some(r) = after_line.strip_prefix('*') {
        (Marker::DiamondFilled, r)
    } else if let Some(r) = after_line.strip_prefix('o') {
        if r.chars().next().is_none_or(char::is_whitespace) {
            (Marker::DiamondHollow, r)
        } else {
            (Marker::None, after_line)
        }
    } else if let Some(r) = after_line.strip_prefix('>') {
        (Marker::Arrow, r)
    } else {
        (Marker::None, after_line)
    };

    let end = s.len() - after_right.len();
    Some(RelToken {
        start: i,
        end,
        left,
        right,
        dashed,
    })
}

/// 関係の端（クラス名＋多重度）。from 側は `名前 "多重度"`、to 側は `"多重度" 名前`
fn parse_end(
    text: &str,
    from_side: bool,
    line_no: usize,
) -> Result<(String, Option<String>), Error> {
    let text = trim_line(text);
    let (name, card) = if from_side {
        // `名前 "多重度"`（多重度は末尾の引用符）
        match split_trailing_quote(text) {
            Some((rest, quoted)) => (trim_line(rest), Some(quoted)),
            None => (text, None),
        }
    } else {
        // `"多重度" 名前`（多重度は先頭の引用符）
        match split_leading_quote(text) {
            Some((quoted, rest)) => (trim_line(rest), Some(quoted)),
            None => (text, None),
        }
    };
    if name.is_empty() {
        return Err(Error::Parse {
            line: line_no,
            message: "関係の端にクラス名がありません".to_string(),
        });
    }
    if name.contains(char::is_whitespace) {
        return Err(Error::Parse {
            line: line_no,
            message: format!("クラス名として解釈できません: `{name}`"),
        });
    }
    Ok((name.to_string(), card.map(|c| c.to_string())))
}

/// 末尾の `"..."` を切り出す（前が名前）
fn split_trailing_quote(text: &str) -> Option<(&str, &str)> {
    let close = text.strip_suffix('"')?;
    let open = close.rfind('"')?;
    Some((&close[..open], &close[open + 1..]))
}

/// 先頭の `"..."` を切り出す（後ろが名前）
fn split_leading_quote(text: &str) -> Option<(&str, &str)> {
    let rest = text.strip_prefix('"')?;
    let close = rest.find('"')?;
    Some((&rest[..close], &rest[close + 1..]))
}

/// `<<interface>> Shape` → (`interface`, `Shape`)
fn parse_annotation_line(line: &str, line_no: usize) -> Result<(String, String), Error> {
    let rest = line.strip_prefix("<<").expect("<< で始まる");
    let Some((anno, after)) = rest.split_once(">>") else {
        return Err(Error::Parse {
            line: line_no,
            message: "アノテーションの >> が閉じられていません".to_string(),
        });
    };
    let anno = trim_line(anno);
    let name = trim_line(after);
    if anno.is_empty() || name.is_empty() {
        return Err(Error::Parse {
            line: line_no,
            message: "アノテーション行は `<<種別>> クラス名` の形にしてください".to_string(),
        });
    }
    Ok((anno.to_string(), name.to_string()))
}

/// ジェネリクス `~` を山括弧へ変換する。
/// 直後が識別子文字なら開き `<`、それ以外（`~`・空白・末尾）なら閉じ `>` とみなす
/// （`Box~T~` → `Box<T>`、`List~List~int~~` → `List<List<int>>`）
fn convert_generics(s: &str) -> String {
    if !s.contains('~') {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '~' {
            let opens = chars
                .peek()
                .is_some_and(|n| n.is_alphanumeric() || *n == '_');
            out.push(if opens { '<' } else { '>' });
        } else {
            out.push(c);
        }
    }
    out
}

fn split_text(text: &str) -> Vec<String> {
    let text = trim_line(text);
    let text = text.trim_matches('"');
    split_br_lines(&decode_entities(text))
}

/// 行を「ラベル `:` の左側」と「ラベル文字列」に分ける。ラベル区切りは単独 `:`。
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
    use super::{convert_generics, parse};
    use crate::class::model::Marker;

    fn cd(body: &str) -> String {
        format!("classDiagram\n{body}")
    }

    #[test]
    fn 基本のクラスとメンバー() {
        let d = parse(&cd(
            "class Animal {\n  +String name\n  +int age\n  +run() void\n}",
        ))
        .unwrap();
        assert_eq!(d.classes.len(), 1);
        let a = &d.classes[0];
        assert_eq!(a.display, "Animal");
        assert_eq!(a.attributes, ["+String name", "+int age"]);
        assert_eq!(a.methods, ["+run() void"]);
    }

    #[test]
    fn メンバー代入形式() {
        let d = parse(&cd("Animal : +String name\nAnimal : +run() void")).unwrap();
        let a = &d.classes[0];
        assert_eq!(a.attributes, ["+String name"]);
        assert_eq!(a.methods, ["+run() void"]);
    }

    #[test]
    fn 関係の向きとマーカー() {
        // A <|-- B は三角マーカーが A（from）側
        let d = parse(&cd("A <|-- B")).unwrap();
        let r = &d.relations[0];
        assert_eq!(d.classes[r.from].display, "A");
        assert_eq!(d.classes[r.to].display, "B");
        assert_eq!(r.from_marker, Marker::Triangle);
        assert_eq!(r.to_marker, Marker::None);
        assert!(!r.dashed);
    }

    #[test]
    fn 全関係種別() {
        let cases = [
            ("A <|-- B", Marker::Triangle, Marker::None, false),
            ("A --|> B", Marker::None, Marker::Triangle, false),
            ("A *-- B", Marker::DiamondFilled, Marker::None, false),
            ("A o-- B", Marker::DiamondHollow, Marker::None, false),
            ("A --> B", Marker::None, Marker::Arrow, false),
            ("A -- B", Marker::None, Marker::None, false),
            ("A ..> B", Marker::None, Marker::Arrow, true),
            ("A ..|> B", Marker::None, Marker::Triangle, true),
            ("A .. B", Marker::None, Marker::None, true),
            ("A --o B", Marker::None, Marker::DiamondHollow, false),
        ];
        for (src, from, to, dashed) in cases {
            let d = parse(&cd(src)).unwrap_or_else(|e| panic!("{src}: {e}"));
            let r = &d.relations[0];
            assert_eq!(
                (r.from_marker, r.to_marker, r.dashed),
                (from, to, dashed),
                "{src}"
            );
        }
    }

    #[test]
    fn ラベルと多重度() {
        let d = parse(&cd("Customer \"1\" --> \"*\" Order : places")).unwrap();
        let r = &d.relations[0];
        assert_eq!(d.classes[r.from].display, "Customer");
        assert_eq!(d.classes[r.to].display, "Order");
        assert_eq!(r.from_card.as_deref(), Some("1"));
        assert_eq!(r.to_card.as_deref(), Some("*"));
        assert_eq!(r.label, ["places"]);
    }

    #[test]
    fn 多重度に含まれる範囲は誤検出しない() {
        // "1..*" の `..` を線種と誤認しないこと
        let d = parse(&cd("Order \"1\" *-- \"1..*\" Item")).unwrap();
        let r = &d.relations[0];
        assert_eq!(r.from_marker, Marker::DiamondFilled);
        assert_eq!(r.to_card.as_deref(), Some("1..*"));
    }

    #[test]
    fn クラス名末尾の_o_を誤ってマーカー扱いしない() {
        // Foo--Bar は Foo と Bar のリンク（Fo と o-- ではない）
        let d = parse(&cd("Foo--Bar")).unwrap();
        let r = &d.relations[0];
        assert_eq!(d.classes[r.from].display, "Foo");
        assert_eq!(d.classes[r.to].display, "Bar");
        assert_eq!((r.from_marker, r.to_marker), (Marker::None, Marker::None));
    }

    #[test]
    fn ジェネリクス変換() {
        assert_eq!(convert_generics("Box~T~"), "Box<T>");
        assert_eq!(convert_generics("List~List~int~~"), "List<List<int>>");
        assert_eq!(convert_generics("Plain"), "Plain");
        let d = parse(&cd("class Box~T~ {\n  +T value\n  +get() T\n}")).unwrap();
        assert_eq!(d.classes[0].display, "Box<T>");
    }

    #[test]
    fn アノテーションの_2_形式() {
        let d = parse(&cd(
            "class Shape {\n  <<interface>>\n  +area() double\n}\nclass Color\n<<enumeration>> Color",
        ))
        .unwrap();
        let shape = d.classes.iter().find(|c| c.display == "Shape").unwrap();
        assert_eq!(shape.annotation.as_deref(), Some("interface"));
        let color = d.classes.iter().find(|c| c.display == "Color").unwrap();
        assert_eq!(color.annotation.as_deref(), Some("enumeration"));
    }

    #[test]
    fn 空クラスと単独宣言() {
        let d = parse(&cd("class Empty\nStandalone")).unwrap();
        assert_eq!(d.classes.len(), 2);
        assert!(
            d.classes
                .iter()
                .all(|c| c.attributes.is_empty() && c.methods.is_empty())
        );
    }

    #[test]
    fn 未対応構文は_unsupported() {
        for src in [
            "note \"これはノート\"",
            "note for Animal \"説明\"",
            "class Animal\nclick Animal href \"https://example.com\"",
            "namespace BaseShapes {\n  class Triangle\n}",
        ] {
            let err = parse(&cd(src)).unwrap_err();
            assert!(err.is_unsupported(), "{src}: {err}");
        }
    }

    /// クラス名から解決済みスタイルを取り出す
    fn class_style(d: &super::ClassDiagram, display: &str) -> crate::common::style::Style {
        let c = d
            .classes
            .iter()
            .find(|c| c.display == display)
            .unwrap_or_else(|| panic!("クラス {display} が見つかりません"));
        c.style.clone().unwrap_or_default()
    }

    #[test]
    fn インラインクラスと後置_classdef_が効く() {
        // `:::` は宣言でもラベルでもなく、後置 classDef も解決される
        let d = parse(&cd(
            "class Animal:::warn\nclassDef warn fill:#f96,stroke:#333",
        ))
        .unwrap();
        let s = class_style(&d, "Animal");
        assert_eq!(s.fill.as_deref(), Some("#f96"));
        assert_eq!(s.stroke.as_deref(), Some("#333"));
    }

    #[test]
    fn cssclass_と_style_文が適用される() {
        // cssClass（引用符付き ids・複数）と style 文（個別・後勝ち）
        let d = parse(&cd(
            "class A\nclass B\nclassDef hot fill:#f96\ncssClass \"A,B\" hot\nstyle A stroke:#111",
        ))
        .unwrap();
        assert_eq!(class_style(&d, "B").fill.as_deref(), Some("#f96"));
        let a = class_style(&d, "A");
        assert_eq!(a.fill.as_deref(), Some("#f96"));
        assert_eq!(a.stroke.as_deref(), Some("#111"));
    }

    #[test]
    fn classdef_default_が全クラス既定になる() {
        let d = parse(&cd("class A\nA <|-- B\nclassDef default fill:#eee")).unwrap();
        for name in ["A", "B"] {
            assert_eq!(
                class_style(&d, name).fill.as_deref(),
                Some("#eee"),
                "{name}"
            );
        }
    }

    #[test]
    fn 宣言の_class_はスタイル適用文と衝突しない() {
        // `class Foo { ... }` は宣言、複数適用は `cssClass`。両立する
        let d = parse(&cd(
            "class Foo {\n  +int x\n}\nclassDef warn fill:#f96\ncssClass \"Foo\" warn",
        ))
        .unwrap();
        assert_eq!(class_style(&d, "Foo").fill.as_deref(), Some("#f96"));
        assert_eq!(
            d.classes[0].attributes,
            vec!["+int x".to_string()],
            "宣言の本体も保持"
        );
    }

    #[test]
    fn 関係の両端にインラインクラスを付けられる() {
        let d = parse(&cd("A:::warn <|-- B : 継承\nclassDef warn fill:#f96")).unwrap();
        assert_eq!(class_style(&d, "A").fill.as_deref(), Some("#f96"));
        assert_eq!(d.relations.len(), 1, "関係も正しく解釈される");
        assert_eq!(d.relations[0].label, vec!["継承".to_string()]);
    }

    #[test]
    fn direction_は受理して無視() {
        let d = parse(&cd("direction LR\nclass A\nA <|-- B")).unwrap();
        assert_eq!(d.classes.len(), 2);
        assert_eq!(d.relations.len(), 1);
    }

    #[test]
    fn 構文エラーの検出() {
        assert!(parse(&cd("class Foo {\n  +int x")).is_err(), "閉じ括弧なし");
        assert!(parse(&cd("class")).is_err(), "クラス名なし");
        assert!(parse("A <|-- B").is_err(), "ヘッダなし");
    }
}
