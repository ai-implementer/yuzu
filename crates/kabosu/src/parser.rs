//! TOML の文法（v0.1 対応範囲）と木構築。
//!
//! 再帰下降（改行を文法に含める行指向ハイブリッド）。重複キー・テーブル競合は
//! 挿入時に検出し、最初の 1 件で `ParseError` として停止する。
//! テーブルの再定義規則は TOML 1.0 に従う:
//! - ヘッダ経路の中間として暗黙に作られたテーブルは、後から `[a]` で定義できる
//! - dotted key で作られたテーブルをヘッダで再定義することはできない（逆も同じ）。
//!   ただし**子テーブルを足すのは可**（`apple.color = "red"` の後の
//!   `[fruit.apple.texture]`）。禁止は終端の再定義だけで、中間経路としては通れる
//! - インラインテーブルは自己完結していて、子テーブルも足せない

use alloc::string::String;
use alloc::vec::Vec;

use crate::error::{ParseError, ParseErrorKind, TomlV11};
use crate::lexer::{
    Cursor, ScalarClass, classify_scalar, parse_datetime, parse_float, parse_integer,
    parse_radix_integer,
};
use crate::model::{KeySegment, Node, Span, Table, TableOrigin, Value};

/// 解析深度の上限（テーブル・配列・dotted key の合算。kabosu.md「型変換と診断」）
const MAX_DEPTH: usize = 128;

/// ドキュメント全体をパースし、(ルートテーブルの Node, コメント span 列) を返す
pub(crate) fn parse(src: &str) -> Result<(Node, Vec<Span>), ParseError> {
    let mut cur = Cursor::new(src);
    let mut root = Table::new(TableOrigin::Root);
    let mut comments: Vec<Span> = Vec::new();
    // 現在のセクション（ヘッダ経路のキー名）。空 = ルート直下
    let mut current_path: Vec<String> = Vec::new();

    loop {
        cur.skip_ws();
        if cur.is_eof() {
            break;
        }
        if cur.at_newline() {
            cur.eat_newline()?;
            continue;
        }
        if cur.at_comment() {
            comments.push(cur.read_comment()?);
            cur.eat_newline()?;
            continue;
        }
        if cur.peek() == Some(b'[') {
            parse_header(&mut cur, &mut root, &mut current_path, &mut comments)?;
            continue;
        }
        parse_keyval(&mut cur, &mut root, &current_path, &mut comments)?;
    }

    // EOF: 現在のセクションの末尾 span を確定する
    let end = Span::point(src.len());
    table_at_mut(&mut root, &current_path).set_end_span(end);
    let root_span = Span {
        start: 0,
        end: src.len(),
    };
    Ok((Node::new(Value::Table(root), root_span), comments))
}

/// `[a.b.c]` ヘッダ行と `[[a.b.c]]` 配列ヘッダ行
fn parse_header(
    cur: &mut Cursor<'_>,
    root: &mut Table,
    current_path: &mut Vec<String>,
    comments: &mut Vec<Span>,
) -> Result<(), ParseError> {
    let start = cur.pos();
    // 直前のセクションの末尾はこのヘッダの直前
    table_at_mut(root, current_path).set_end_span(Span::point(start));

    cur.eat(b'[');
    let is_array = cur.eat(b'[');

    let mut segments: Vec<KeySegment> = Vec::new();
    loop {
        cur.skip_ws();
        let seg = cur.read_key_segment()?;
        segments.push(seg);
        cur.skip_ws();
        if cur.eat(b'.') {
            continue;
        }
        if cur.eat(b']') {
            break;
        }
        return Err(ParseError::new(
            ParseErrorKind::UnclosedTableHeader,
            Span::point(cur.pos()),
        ));
    }
    if is_array && !cur.eat(b']') {
        return Err(ParseError::new(
            ParseErrorKind::UnclosedTableHeader,
            Span::point(cur.pos()),
        ));
    }
    if segments.len() > MAX_DEPTH {
        return Err(ParseError::new(
            ParseErrorKind::DepthExceeded,
            Span {
                start,
                end: cur.pos(),
            },
        ));
    }

    let header_span = Span {
        start,
        end: cur.pos(),
    };
    if is_array {
        define_array_table(root, &segments, header_span)?;
    } else {
        define_header_table(root, &segments)?;
    }
    *current_path = segments.iter().map(|s| String::from(s.name())).collect();
    // 定義したあとの実際の深さで見る（`[[a]]` は 2 段なのでセグメント数では足りない）
    if section_depth(root, current_path) > MAX_DEPTH {
        return Err(ParseError::new(ParseErrorKind::DepthExceeded, header_span));
    }

    cur.skip_ws();
    if cur.at_comment() {
        comments.push(cur.read_comment()?);
    }
    cur.eat_newline()
}

/// ヘッダ経路の中間として降りられるノードか。
///
/// **dotted key で作られたテーブルも中間としては通れる**（TOML 1.0 の
/// `apple.color = "red"` の後に `[fruit.apple.texture]` を足せる例）。
/// 禁じられているのは終端での再定義（`[fruit.apple]`）だけなので、
/// そちらは `define_header_table` の終端判定で見る。
/// インラインテーブルは自己完結していて子テーブルも足せないので不可、
/// 配列は `[[...]]` が作ったもの（最後の要素が `ArrayHeader`）だけ可
fn can_descend(node: &Node) -> bool {
    match node.value() {
        Value::Table(t) => t.origin() != TableOrigin::Inline,
        Value::Array(items) => is_array_of_tables(items),
        _ => false,
    }
}

/// `[[...]]` が作った配列か（最後の要素が `ArrayHeader` 起源のテーブル）
fn is_array_of_tables(items: &[Node]) -> bool {
    matches!(
        items.last().map(Node::value),
        Some(Value::Table(t)) if t.origin() == TableOrigin::ArrayHeader
    )
}

/// ヘッダ経路の 1 段を降りる（配列なら**最後の要素**へ）
fn descend_mut(node: &mut Node) -> Option<&mut Table> {
    match node.value_mut() {
        Value::Table(t) => Some(t),
        Value::Array(items) => match items.last_mut()?.value_mut() {
            Value::Table(t) if t.origin() == TableOrigin::ArrayHeader => Some(t),
            _ => None,
        },
        _ => None,
    }
}

/// ヘッダ経路の中間セグメントを辿る（無ければ `HeaderImplicit` で作る）
fn walk_intermediates<'t>(
    root: &'t mut Table,
    segments: &[KeySegment],
) -> Result<&'t mut Table, ParseError> {
    let mut t = root;
    for seg in segments {
        match t.get(seg.name()) {
            None => {
                let mut sub = Table::new(TableOrigin::HeaderImplicit);
                sub.set_end_span(Span::point(seg.span().end));
                t.insert(seg.clone(), Node::new(Value::Table(sub), seg.span()));
            }
            Some(entry) => {
                if !can_descend(entry.node()) {
                    return Err(ParseError::with_previous(
                        ParseErrorKind::TableConflict,
                        seg.span(),
                        entry.key_span(),
                    ));
                }
            }
        }
        let entry = t.get_mut(seg.name()).expect("直前に作ったか検査済み");
        t = descend_mut(entry.node_mut()).expect("直前に検査済み");
    }
    Ok(t)
}

/// `[a.b]` の終端テーブルを定義する
fn define_header_table(root: &mut Table, segments: &[KeySegment]) -> Result<(), ParseError> {
    let (last, intermediates) = segments.split_last().expect("1 つ以上ある");
    let t = walk_intermediates(root, intermediates)?;
    if t.get(last.name()).is_none() {
        let mut table = Table::new(TableOrigin::Header);
        table.set_end_span(Span::point(last.span().end));
        t.insert(last.clone(), Node::new(Value::Table(table), last.span()));
        return Ok(());
    }
    let prev_span = t.get(last.name()).expect("存在確認済み").key_span();
    let entry = t.get_mut(last.name()).expect("存在確認済み");
    let Value::Table(sub) = entry.node_mut().value_mut() else {
        // 値・配列が入っているキーへのテーブル定義（`[[a]]` の後の `[a]` を含む）
        return Err(ParseError::with_previous(
            ParseErrorKind::TableConflict,
            last.span(),
            prev_span,
        ));
    };
    match sub.origin() {
        // ヘッダ経路の中間として暗黙に作られたテーブルは後から明示定義できる
        TableOrigin::HeaderImplicit => {
            sub.set_origin(TableOrigin::Header);
            Ok(())
        }
        // `[a]` の再定義
        TableOrigin::Header | TableOrigin::Root | TableOrigin::ArrayHeader => Err(
            ParseError::with_previous(ParseErrorKind::DuplicateKey, last.span(), prev_span),
        ),
        // dotted key・インラインテーブルで作られたテーブルは閉じている
        TableOrigin::Dotted | TableOrigin::Inline => Err(ParseError::with_previous(
            ParseErrorKind::TableConflict,
            last.span(),
            prev_span,
        )),
    }
}

/// `[[a.b]]` の要素テーブルを配列末尾に追加する。
/// 配列が無ければ作り、`[[...]]` 以外で作られたキーとは衝突させる
fn define_array_table(
    root: &mut Table,
    segments: &[KeySegment],
    header_span: Span,
) -> Result<(), ParseError> {
    let (last, intermediates) = segments.split_last().expect("1 つ以上ある");
    let t = walk_intermediates(root, intermediates)?;
    let mut element = Table::new(TableOrigin::ArrayHeader);
    element.set_end_span(Span::point(header_span.end));
    let element = Node::new(Value::Table(element), header_span);

    let Some(entry) = t.get(last.name()) else {
        t.insert(
            last.clone(),
            Node::new(Value::Array(alloc::vec![element]), last.span()),
        );
        return Ok(());
    };
    let prev_span = entry.key_span();
    let extendable =
        matches!(entry.node().value(), Value::Array(items) if is_array_of_tables(items));
    if !extendable {
        // 静的配列・テーブル・値への `[[a]]`
        return Err(ParseError::with_previous(
            ParseErrorKind::TableConflict,
            last.span(),
            prev_span,
        ));
    }
    let entry = t.get_mut(last.name()).expect("存在確認済み");
    let Value::Array(items) = entry.node_mut().value_mut() else {
        unreachable!("直前に検査済み");
    };
    items.push(element);
    Ok(())
}

/// `a.b = value` 行
fn parse_keyval(
    cur: &mut Cursor<'_>,
    root: &mut Table,
    current_path: &[String],
    comments: &mut Vec<Span>,
) -> Result<(), ParseError> {
    let mut segments: Vec<KeySegment> = Vec::new();
    loop {
        let seg = cur.read_key_segment()?;
        segments.push(seg);
        cur.skip_ws();
        if cur.eat(b'.') {
            cur.skip_ws();
            continue;
        }
        break;
    }
    if !cur.eat(b'=') {
        return Err(ParseError::new(
            ParseErrorKind::ExpectedEquals,
            Span::point(cur.pos()),
        ));
    }
    cur.skip_ws();

    let depth = section_depth(root, current_path) + segments.len();
    if depth > MAX_DEPTH {
        let span = Span {
            start: segments.first().expect("1 つ以上ある").span().start,
            end: segments.last().expect("1 つ以上ある").span().end,
        };
        return Err(ParseError::new(ParseErrorKind::DepthExceeded, span));
    }

    let node = parse_value(cur, comments, depth)?;

    cur.skip_ws();
    if cur.at_comment() {
        comments.push(cur.read_comment()?);
    }
    cur.eat_newline()?;

    let table = table_at_mut(root, current_path);
    insert_dotted(table, &segments, node)
}

/// dotted key を現在のテーブルへ挿入する（中間は Dotted 起源のテーブルだけ辿れる）
fn insert_dotted(table: &mut Table, segments: &[KeySegment], node: Node) -> Result<(), ParseError> {
    let mut t = table;
    let (last, intermediates) = segments.split_last().expect("1 つ以上ある");
    for seg in intermediates {
        if t.get(seg.name()).is_none() {
            let mut sub = Table::new(TableOrigin::Dotted);
            sub.set_end_span(Span::point(seg.span().end));
            t.insert(seg.clone(), Node::new(Value::Table(sub), seg.span()));
        } else {
            let prev_span = t.get(seg.name()).expect("存在確認済み").key_span();
            let entry = t.get(seg.name()).expect("存在確認済み");
            let ok = matches!(entry.node().value(), Value::Table(sub) if sub.origin() == TableOrigin::Dotted);
            if !ok {
                return Err(ParseError::with_previous(
                    ParseErrorKind::TableConflict,
                    seg.span(),
                    prev_span,
                ));
            }
        }
        let entry = t.get_mut(seg.name()).expect("存在確認済み");
        let Value::Table(sub) = entry.node_mut().value_mut() else {
            unreachable!("直前に検査済み");
        };
        t = sub;
    }
    if let Some(existing) = t.get(last.name()) {
        return Err(ParseError::with_previous(
            ParseErrorKind::DuplicateKey,
            last.span(),
            existing.key_span(),
        ));
    }
    t.insert(last.clone(), node);
    Ok(())
}

/// 現在のセクションの入れ子の深さ。
///
/// **`[[a]]` は「配列」と「その要素テーブル」で 2 段**になる。
/// エンコーダ（`TableEncoder::field` と `ArrayEncoder::element` が 1 段ずつ）も
/// そう数えるので、ここを経路のセグメント数（1 段）で代用すると
/// **「パースできたのにエンコードできない」木が作れる**（fuzz が見つけた）
fn section_depth(root: &Table, path: &[String]) -> usize {
    let mut depth = 0;
    let mut table = root;
    for name in path {
        let Some(entry) = table.get(name) else {
            return depth;
        };
        match entry.node().value() {
            Value::Table(sub) => {
                depth += 1;
                table = sub;
            }
            Value::Array(items) => {
                depth += 2;
                match items.last().map(Node::value) {
                    Some(Value::Table(sub)) => table = sub,
                    _ => return depth,
                }
            }
            _ => return depth,
        }
    }
    depth
}

/// ヘッダ確定済みの経路を辿る（配列は最後の要素へ降りる）
fn table_at_mut<'t>(root: &'t mut Table, path: &[String]) -> &'t mut Table {
    let mut t = root;
    for name in path {
        let entry = t.get_mut(name).expect("ヘッダ確定済みの経路");
        t = descend_mut(entry.node_mut()).expect("ヘッダ経路は常にテーブルか配列の要素");
    }
    t
}

/// 値（文字列・整数・float・真偽値・配列。未対応構文は位置付き Unsupported）
fn parse_value(
    cur: &mut Cursor<'_>,
    comments: &mut Vec<Span>,
    depth: usize,
) -> Result<Node, ParseError> {
    match cur.peek() {
        None => Err(ParseError::new(
            ParseErrorKind::ExpectedValue,
            Span::point(cur.pos()),
        )),
        Some(b'"' | b'\'') => {
            let (s, span) = cur.read_string_value()?;
            Ok(Node::new(Value::String(s), span))
        }
        Some(b'[') => parse_array(cur, comments, depth),
        Some(b'{') => parse_inline_table(cur, comments, depth),
        Some(_) => {
            let (blob, span) = cur.read_scalar_blob();
            match classify_scalar(blob) {
                ScalarClass::True => Ok(Node::new(Value::Boolean(true), span)),
                ScalarClass::False => Ok(Node::new(Value::Boolean(false), span)),
                ScalarClass::Integer => {
                    Ok(Node::new(Value::Integer(parse_integer(blob, span)?), span))
                }
                ScalarClass::RadixInteger(radix) => Ok(Node::new(
                    Value::Integer(parse_radix_integer(blob, radix, span)?),
                    span,
                )),
                ScalarClass::Float => Ok(Node::new(Value::Float(parse_float(blob, span)?), span)),
                ScalarClass::Datetime => Ok(Node::new(
                    Value::Datetime(parse_datetime(blob, span)?),
                    span,
                )),
                ScalarClass::TomlV11(feature) => {
                    Err(ParseError::new(ParseErrorKind::Unsupported(feature), span))
                }
                ScalarClass::InvalidInteger => {
                    Err(ParseError::new(ParseErrorKind::InvalidInteger, span))
                }
                ScalarClass::InvalidLiteral => {
                    Err(ParseError::new(ParseErrorKind::InvalidLiteral, span))
                }
                ScalarClass::NotAValue => Err(ParseError::new(
                    ParseErrorKind::ExpectedValue,
                    if blob.is_empty() {
                        Span::point(cur.pos())
                    } else {
                        span
                    },
                )),
            }
        }
    }
}

/// 配列。複数行・末尾カンマ・要素間コメントを許す（v0.1 対応範囲）
fn parse_array(
    cur: &mut Cursor<'_>,
    comments: &mut Vec<Span>,
    depth: usize,
) -> Result<Node, ParseError> {
    let start = cur.pos();
    cur.eat(b'[');
    let mut items: Vec<Node> = Vec::new();
    loop {
        skip_trivia(cur, comments)?;
        if cur.eat(b']') {
            break;
        }
        if cur.is_eof() {
            return Err(ParseError::new(
                ParseErrorKind::UnclosedArray,
                Span {
                    start,
                    end: cur.pos(),
                },
            ));
        }
        // 深さの検査は**要素ごと**に行う。空の `[]` は入れ子を 1 段も増やさない
        // ので、入口で弾くと空テーブルと同じ「再パースできない出力」が作れる
        if depth + 1 > MAX_DEPTH {
            return Err(ParseError::new(
                ParseErrorKind::DepthExceeded,
                Span::point(cur.pos()),
            ));
        }
        items.push(parse_value(cur, comments, depth + 1)?);
        skip_trivia(cur, comments)?;
        if cur.eat(b',') {
            continue;
        }
        if cur.eat(b']') {
            break;
        }
        return Err(ParseError::new(
            ParseErrorKind::UnclosedArray,
            Span {
                start,
                end: cur.pos(),
            },
        ));
    }
    Ok(Node::new(
        Value::Array(items),
        Span {
            start,
            end: cur.pos(),
        },
    ))
}

/// インラインテーブル `{ k = v, a.b = 1 }`。
/// TOML 1.0 では 1 行に収める（改行・コメント・末尾カンマは不可）。
/// 作ったテーブルは `TableOrigin::Inline` = 閉じていて、後から
/// `[x.y]` ヘッダや `x.z = 1` で拡張できない
fn parse_inline_table(
    cur: &mut Cursor<'_>,
    comments: &mut Vec<Span>,
    depth: usize,
) -> Result<Node, ParseError> {
    let start = cur.pos();
    cur.eat(b'{');
    // 深さの検査は**キーごと**に行う（下の `value_depth`）。
    // 空の `{}` は入れ子を 1 段も増やさないので、ここで弾いてはいけない
    // （エンコーダも空テーブルでは深度を消費しないため、
    // 弾くと「to_string は通るのに再パースできない」出力が作れる）
    let unclosed = |cur: &Cursor<'_>| {
        ParseError::new(
            ParseErrorKind::UnclosedInlineTable,
            Span {
                start,
                end: cur.pos(),
            },
        )
    };
    // 改行・コメント・末尾カンマは TOML 1.1 の記法。書き間違いと区別して案内する
    let v11 = |cur: &Cursor<'_>| {
        ParseError::new(
            ParseErrorKind::Unsupported(TomlV11::InlineTable),
            Span::point(cur.pos()),
        )
    };
    let mut table = Table::new(TableOrigin::Inline);
    cur.skip_ws();
    if !cur.eat(b'}') {
        loop {
            cur.skip_ws();
            if cur.at_newline() || cur.at_comment() {
                return Err(v11(cur));
            }
            if cur.is_eof() {
                return Err(unclosed(cur));
            }
            // ここへ来るのはカンマの後だけ（空の `{}` は上で処理済み）
            if cur.peek() == Some(b'}') {
                return Err(v11(cur));
            }
            // キー（dotted 可）
            let mut segments: Vec<KeySegment> = Vec::new();
            loop {
                segments.push(cur.read_key_segment()?);
                cur.skip_ws();
                if cur.eat(b'.') {
                    cur.skip_ws();
                    continue;
                }
                break;
            }
            if !cur.eat(b'=') {
                return Err(ParseError::new(
                    ParseErrorKind::ExpectedEquals,
                    Span::point(cur.pos()),
                ));
            }
            cur.skip_ws();
            // 深度はキーのセグメント 1 つにつき 1（`parse_keyval` と同じ数え方）。
            // **エンコーダの `TableEncoder::field` も 1 段につき 1 なので、
            // ここを 2 段ぶん数えると「to_string は通るのに再パースで
            // DepthExceeded」になる**
            let value_depth = depth + segments.len();
            if value_depth > MAX_DEPTH {
                return Err(ParseError::new(
                    ParseErrorKind::DepthExceeded,
                    Span::point(cur.pos()),
                ));
            }
            let node = parse_value(cur, comments, value_depth)?;
            insert_dotted(&mut table, &segments, node)?;
            cur.skip_ws();
            if cur.eat(b',') {
                continue;
            }
            if cur.eat(b'}') {
                break;
            }
            if cur.at_newline() || cur.at_comment() {
                return Err(v11(cur));
            }
            return Err(unclosed(cur));
        }
    }
    let span = Span {
        start,
        end: cur.pos(),
    };
    table.set_end_span(Span::point(span.end));
    Ok(Node::new(Value::Table(table), span))
}

/// 配列内の空白・改行・コメントを読み飛ばす
fn skip_trivia(cur: &mut Cursor<'_>, comments: &mut Vec<Span>) -> Result<(), ParseError> {
    loop {
        cur.skip_ws();
        if cur.at_comment() {
            comments.push(cur.read_comment()?);
            continue;
        }
        if cur.at_newline() {
            cur.eat_newline()?;
            continue;
        }
        return Ok(());
    }
}

#[cfg(test)]
mod tests {
    use alloc::string::ToString;

    use crate::datetime::DatetimeKind;
    use crate::error::{ParseErrorKind, TomlV11};
    use crate::model::Document;

    fn parse(src: &str) -> Document {
        Document::parse(src).unwrap()
    }

    fn err_kind(src: &str) -> ParseErrorKind {
        Document::parse(src).unwrap_err().kind().clone()
    }

    #[test]
    fn 基本形をパースできる() {
        let d = parse("title = \"yuzu\"\ncount = 42\nok = true\n");
        let root = d.root();
        assert_eq!(root.len(), 3);
        assert_eq!(root.get("title").unwrap().node().as_str(), Some("yuzu"));
        assert_eq!(root.get("count").unwrap().node().as_integer(), Some(42));
        assert_eq!(root.get("ok").unwrap().node().as_boolean(), Some(true));
    }

    #[test]
    fn テーブルとネストテーブルを構築できる() {
        let d = parse("[site]\ntitle = \"a\"\n[markdown.highlight]\nenabled = false\n");
        let site = d.root().get("site").unwrap().node().as_table().unwrap();
        assert_eq!(site.get("title").unwrap().node().as_str(), Some("a"));
        let hl = d
            .root()
            .get("markdown")
            .unwrap()
            .node()
            .as_table()
            .unwrap()
            .get("highlight")
            .unwrap()
            .node()
            .as_table()
            .unwrap();
        assert_eq!(hl.get("enabled").unwrap().node().as_boolean(), Some(false));
    }

    #[test]
    fn dotted_key_と暗黙ヘッダの後定義() {
        // ヘッダ経路の中間 a は後から [a] で定義できる
        let d = parse("[a.b]\nx = 1\n[a]\ny = 2\n");
        let a = d.root().get("a").unwrap().node().as_table().unwrap();
        assert_eq!(a.get("y").unwrap().node().as_integer(), Some(2));

        // dotted key はテーブル内でネストを作る
        let d = parse("[srv]\nnet.host = \"h\"\nnet.port = 80\n");
        let net = d
            .root()
            .get("srv")
            .unwrap()
            .node()
            .as_table()
            .unwrap()
            .get("net")
            .unwrap()
            .node()
            .as_table()
            .unwrap();
        assert_eq!(net.get("port").unwrap().node().as_integer(), Some(80));
    }

    #[test]
    fn 配列は複数行と末尾カンマとコメントを許す() {
        let d = parse(
            "a = [1, 2, 3]\nb = [\n  \"x\", # コメント\n  \"y\",\n]\nc = [[1], [2, 3]]\nempty = []\n",
        );
        assert_eq!(
            d.root().get("a").unwrap().node().as_array().unwrap().len(),
            3
        );
        assert_eq!(
            d.root().get("b").unwrap().node().as_array().unwrap().len(),
            2
        );
        let c = d.root().get("c").unwrap().node().as_array().unwrap();
        assert_eq!(c[1].as_array().unwrap().len(), 2);
        assert!(
            d.root()
                .get("empty")
                .unwrap()
                .node()
                .as_array()
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn 引用キーと日本語キー() {
        let d = parse("[lint.terms]\n\"サーバー\" = [\"サーバ\"]\n'リテラル' = []\n");
        let terms = d
            .root()
            .get("lint")
            .unwrap()
            .node()
            .as_table()
            .unwrap()
            .get("terms")
            .unwrap()
            .node()
            .as_table()
            .unwrap();
        assert!(terms.get("サーバー").is_some());
        assert!(terms.get("リテラル").is_some());
    }

    #[test]
    fn コメントが_span_付きで収集される() {
        let d = parse("# 先頭\na = 1 # 行末\n");
        let comments: alloc::vec::Vec<_> = d.comments().map(|c| c.text().to_string()).collect();
        assert_eq!(comments, ["# 先頭", "# 行末"]);
    }

    #[test]
    fn crlf_と全角を含む原文() {
        let d = parse("a = \"あい\"\r\n[t]\r\nb = 2\r\n");
        assert_eq!(d.root().get("a").unwrap().node().as_str(), Some("あい"));
    }

    #[test]
    fn 重複キーは先行位置付きのエラー() {
        let e = Document::parse("a = 1\na = 2\n").unwrap_err();
        assert_eq!(*e.kind(), ParseErrorKind::DuplicateKey);
        assert!(e.previous_span().is_some());
        assert!(e.span().start > e.previous_span().unwrap().start);
    }

    #[test]
    fn テーブルの再定義と競合はエラー() {
        assert_eq!(err_kind("[a]\n[a]\n"), ParseErrorKind::DuplicateKey);
        assert_eq!(err_kind("a = 1\n[a]\n"), ParseErrorKind::TableConflict);
        // dotted key で作ったテーブルのヘッダ再定義は不可
        assert_eq!(err_kind("a.b = 1\n[a]\n"), ParseErrorKind::TableConflict);
        // ヘッダで定義済みのテーブルを dotted key で拡張するのも不可
        assert_eq!(
            err_kind("[a.b]\n[a]\nb.c = 1\n"),
            ParseErrorKind::TableConflict
        );
        // 値のキーを dotted key が横断
        assert_eq!(err_kind("a = 1\na.b = 2\n"), ParseErrorKind::TableConflict);
    }

    #[test]
    fn dotted_key_で作ったテーブルにも子テーブルを足せる() {
        // TOML 1.0 の例文: `[fruit.apple.texture]` は足せる
        let d = parse(
            "[fruit]\n\
             apple.color = \"red\"\n\
             apple.taste = \"sweet\"\n\
             \n\
             [fruit.apple.texture]\n\
             smooth = true\n",
        );
        let apple = d
            .root()
            .get("fruit")
            .unwrap()
            .node()
            .as_table()
            .unwrap()
            .get("apple")
            .unwrap()
            .node()
            .as_table()
            .unwrap();
        assert_eq!(apple.get("color").unwrap().node().as_str(), Some("red"));
        assert!(
            apple
                .get("texture")
                .unwrap()
                .node()
                .as_table()
                .unwrap()
                .get("smooth")
                .unwrap()
                .node()
                .as_boolean()
                .unwrap()
        );
        // ヘッダ・配列ヘッダのどちらでも中間経路として通れる
        assert!(Document::parse("a.b = 1\n[a.c]\nx = 1\n").is_ok());
        assert!(Document::parse("a.b = 1\n[[a.c]]\nx = 1\n").is_ok());
        // 終端での再定義は不可のまま
        assert_eq!(err_kind("a.b = 1\n[a]\n"), ParseErrorKind::TableConflict);
        assert_eq!(
            err_kind("[fruit]\napple.color = \"red\"\n[fruit.apple]\n"),
            ParseErrorKind::TableConflict
        );
        // dotted key が作った「値」は経路にできない
        assert_eq!(
            err_kind("a.b = 1\n[a.b.c]\n"),
            ParseErrorKind::TableConflict
        );
    }

    #[test]
    fn インラインテーブルを読める() {
        let d = parse(
            "empty = {}\n\
             point = { x = 1, y = 2 }\n\
             dotted = { a.b = \"v\" }\n\
             nested = { inner = { deep = true } }\n\
             list = [{ id = 1 }, { id = 2 }]\n",
        );
        let root = d.root();
        assert_eq!(
            root.get("empty").unwrap().node().as_table().unwrap().len(),
            0
        );
        let point = root.get("point").unwrap().node().as_table().unwrap();
        assert_eq!(point.get("y").unwrap().node().as_integer(), Some(2));
        let inner = root
            .get("dotted")
            .unwrap()
            .node()
            .as_table()
            .unwrap()
            .get("a")
            .unwrap()
            .node()
            .as_table()
            .unwrap();
        assert_eq!(inner.get("b").unwrap().node().as_str(), Some("v"));
        assert!(
            root.get("nested")
                .unwrap()
                .node()
                .as_table()
                .unwrap()
                .get("inner")
                .unwrap()
                .node()
                .as_table()
                .is_some()
        );
        let list = root.get("list").unwrap().node().as_array().unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(
            list[1]
                .as_table()
                .unwrap()
                .get("id")
                .unwrap()
                .node()
                .as_integer(),
            Some(2)
        );
        // span は `{` から `}` まで
        let span = root.get("point").unwrap().node().span();
        assert_eq!(&d.source()[span.start..span.end], "{ x = 1, y = 2 }");
    }

    #[test]
    fn インラインテーブルは_1_行で閉じている() {
        // 改行・コメント・末尾カンマは TOML 1.1 の記法（1.0 では不可）。
        // 書き間違いではないので Unsupported として区別する
        let v11 = ParseErrorKind::Unsupported(TomlV11::InlineTable);
        assert_eq!(err_kind("x = {\n a = 1 }\n"), v11);
        assert_eq!(err_kind("x = { a = 1,\n b = 2 }\n"), v11);
        assert_eq!(err_kind("x = { a = 1 # c\n }\n"), v11);
        assert_eq!(err_kind("x = { a = 1, }\n"), v11);
        // 閉じ忘れ（EOF）と区切り忘れは素直に構文エラー
        assert_eq!(
            err_kind("x = { a = 1 "),
            ParseErrorKind::UnclosedInlineTable
        );
        assert_eq!(
            err_kind("x = { a = 1 b = 2 }\n"),
            ParseErrorKind::UnclosedInlineTable
        );
        // 重複キーは中でも検出する
        assert_eq!(
            err_kind("x = { a = 1, a = 2 }\n"),
            ParseErrorKind::DuplicateKey
        );
    }

    #[test]
    fn toml_1_1_の記法は書き間違いと区別される() {
        assert_eq!(
            err_kind("x = \"\\e\"\n"),
            ParseErrorKind::Unsupported(TomlV11::Escape)
        );
        assert_eq!(
            err_kind("x = \"\\x41\"\n"),
            ParseErrorKind::Unsupported(TomlV11::Escape)
        );
        assert_eq!(
            err_kind("x = 07:32\n"),
            ParseErrorKind::Unsupported(TomlV11::TimeWithoutSeconds)
        );
        // 未知のエスケープは書き間違いのまま
        assert_eq!(err_kind("x = \"\\q\"\n"), ParseErrorKind::InvalidEscape);
    }

    /// 引用符なしのキーは TOML 1.0 / 1.1 とも `A-Za-z0-9_-` だけ。
    /// **1.1 でも許されない**ので「1.1 の記法」として案内してはいけない
    #[test]
    fn 引用符なしの非_ascii_キーは普通のキー構文エラー() {
        assert_eq!(err_kind("サーバ = 1\n"), ParseErrorKind::ExpectedKey);
        assert_eq!(err_kind("[サーバ]\n"), ParseErrorKind::ExpectedKey);
        assert_eq!(err_kind("a.サーバ = 1\n"), ParseErrorKind::ExpectedKey);
        assert_eq!(err_kind("= 1\n"), ParseErrorKind::ExpectedKey);
        // 引用すればどちらの版でも書ける
        assert!(Document::parse("\"サーバ\" = 1\n").is_ok());
        assert!(Document::parse("[\"サーバ\"]\n").is_ok());
    }

    #[test]
    fn インラインテーブルは閉じていて後から拡張できない() {
        assert_eq!(
            err_kind("x = { a = 1 }\n[x.b]\n"),
            ParseErrorKind::TableConflict
        );
        assert_eq!(
            err_kind("x = { a = 1 }\nx.b = 2\n"),
            ParseErrorKind::TableConflict
        );
        assert_eq!(
            err_kind("x = { a = 1 }\n[x]\n"),
            ParseErrorKind::TableConflict
        );
    }

    #[test]
    fn テーブルの配列を読める() {
        let d = parse(
            "[[products]]\n\
             name = \"Hammer\"\n\
             [products.spec]\n\
             weight = 1\n\
             [[products]]\n\
             name = \"Nail\"\n\
             [[products.tags]]\n\
             label = \"metal\"\n",
        );
        let products = d.root().get("products").unwrap().node().as_array().unwrap();
        assert_eq!(products.len(), 2);
        let first = products[0].as_table().unwrap();
        assert_eq!(first.get("name").unwrap().node().as_str(), Some("Hammer"));
        // `[products.spec]` は直前の要素の中
        assert_eq!(
            first
                .get("spec")
                .unwrap()
                .node()
                .as_table()
                .unwrap()
                .get("weight")
                .unwrap()
                .node()
                .as_integer(),
            Some(1)
        );
        let second = products[1].as_table().unwrap();
        assert_eq!(second.get("name").unwrap().node().as_str(), Some("Nail"));
        // ネストした `[[products.tags]]` も最後の要素の中
        let tags = second.get("tags").unwrap().node().as_array().unwrap();
        assert_eq!(
            tags[0]
                .as_table()
                .unwrap()
                .get("label")
                .unwrap()
                .node()
                .as_str(),
            Some("metal")
        );
    }

    #[test]
    fn テーブルの配列と静的配列やテーブルは衝突する() {
        // 静的配列への `[[a]]`（逆も）
        assert_eq!(err_kind("a = [1]\n[[a]]\n"), ParseErrorKind::TableConflict);
        assert_eq!(err_kind("a = []\n[[a]]\n"), ParseErrorKind::TableConflict);
        assert_eq!(err_kind("a = 1\n[[a]]\n"), ParseErrorKind::TableConflict);
        // `[[a]]` の後の `a = 1` は要素の中のキー（衝突しない）
        assert!(Document::parse("[[a]]\na = 1\n").is_ok());
        // インラインテーブルの配列は静的扱い
        assert_eq!(
            err_kind("a = [{ b = 1 }]\n[[a]]\n"),
            ParseErrorKind::TableConflict
        );
        // `[a]` と `[[a]]` の混在
        assert_eq!(err_kind("[a]\n[[a]]\n"), ParseErrorKind::TableConflict);
        assert_eq!(err_kind("[[a]]\n[a]\n"), ParseErrorKind::TableConflict);
        // 閉じ忘れ
        assert_eq!(err_kind("[[a]\n"), ParseErrorKind::UnclosedTableHeader);
    }

    #[test]
    fn date_time_を値として読める() {
        let d = parse(
            "odt = 1979-05-27T07:32:00Z\n\
             odt2 = 1979-05-27T00:32:00.999999-07:00\n\
             ldt = 1979-05-27t07:32:00\n\
             sp = 1979-05-27 07:32:00\n\
             ld = 1979-05-27\n\
             lt = 07:32:00.5\n",
        );
        let root = d.root();
        let dt = |key: &str| root.get(key).unwrap().node().as_datetime().unwrap();
        assert_eq!(dt("odt").kind(), DatetimeKind::OffsetDatetime);
        assert_eq!(dt("odt").to_string(), "1979-05-27T07:32:00Z");
        assert_eq!(dt("odt2").to_string(), "1979-05-27T00:32:00.999999-07:00");
        assert_eq!(dt("ldt").kind(), DatetimeKind::LocalDatetime);
        // 小文字の `t` 区切りも受理し、正規形は大文字の `T`
        assert_eq!(dt("ldt").to_string(), "1979-05-27T07:32:00");
        // 空白区切りは 1 塊として読む（正規形は `T` 区切り）
        assert_eq!(dt("sp").to_string(), "1979-05-27T07:32:00");
        assert_eq!(dt("ld").kind(), DatetimeKind::LocalDate);
        assert_eq!(dt("lt").kind(), DatetimeKind::LocalTime);
        assert_eq!(dt("lt").time().unwrap().nanosecond(), 500_000_000);
        // 日付・時刻は数値としては読めない（型は区別する）
        assert_eq!(root.get("ld").unwrap().node().as_integer(), None);
        // span は原文のリテラル全体（空白区切りも含めて 1 つ）
        let sp = root.get("sp").unwrap().node().span();
        assert_eq!(&d.source()[sp.start..sp.end], "1979-05-27 07:32:00");
    }

    #[test]
    fn 日付の後の空白はコメントや行末と繋げない() {
        let d = parse("x = 1979-05-27 # コメント\ny = 1979-05-27\n");
        let dt = |key: &str| {
            d.root()
                .get(key)
                .unwrap()
                .node()
                .as_datetime()
                .unwrap()
                .to_string()
        };
        assert_eq!(dt("x"), "1979-05-27");
        assert_eq!(dt("y"), "1979-05-27");
    }

    #[test]
    fn float_と進数整数と複数行文字列を値として読める() {
        let d = parse(
            "pi = 2.5\nneg = -inf\nn = nan\nhex = 0xFF\noct = 0o17\nbin = 0b101\n\
             ml = \"\"\"\nline 1\nline 2\"\"\"\nraw = '''\\n'''\n",
        );
        let root = d.root();
        assert_eq!(root.get("pi").unwrap().node().as_float(), Some(2.5));
        assert_eq!(
            root.get("neg").unwrap().node().as_float(),
            Some(f64::NEG_INFINITY)
        );
        assert!(root.get("n").unwrap().node().as_float().unwrap().is_nan());
        // 進数整数は Integer（表記は保持しない）
        assert_eq!(root.get("hex").unwrap().node().as_integer(), Some(255));
        assert_eq!(root.get("oct").unwrap().node().as_integer(), Some(15));
        assert_eq!(root.get("bin").unwrap().node().as_integer(), Some(5));
        // float は整数として読めない（型は区別する）
        assert_eq!(root.get("pi").unwrap().node().as_integer(), None);
        assert_eq!(
            root.get("ml").unwrap().node().as_str(),
            Some("line 1\nline 2")
        );
        assert_eq!(root.get("raw").unwrap().node().as_str(), Some("\\n"));
        // span は原文のリテラル全体
        let ml = root.get("ml").unwrap().node().span();
        assert_eq!(
            &d.source()[ml.start..ml.end],
            "\"\"\"\nline 1\nline 2\"\"\""
        );
    }

    #[test]
    fn 複数行文字列はキーになれない() {
        assert_eq!(
            err_kind("\"\"\"k\"\"\" = 1\n"),
            ParseErrorKind::MultilineStringAsKey
        );
        assert_eq!(
            err_kind("'''k''' = 1\n"),
            ParseErrorKind::MultilineStringAsKey
        );
        assert_eq!(
            err_kind("[\"\"\"t\"\"\"]\n"),
            ParseErrorKind::MultilineStringAsKey
        );
    }

    #[test]
    fn 構文エラーの検出() {
        assert_eq!(err_kind("a\n"), ParseErrorKind::ExpectedEquals);
        assert_eq!(err_kind("a =\n"), ParseErrorKind::ExpectedValue);
        assert_eq!(err_kind("a = hello\n"), ParseErrorKind::ExpectedValue);
        assert_eq!(err_kind("a = 042\n"), ParseErrorKind::InvalidInteger);
        assert_eq!(err_kind("[a\n"), ParseErrorKind::UnclosedTableHeader);
        assert_eq!(err_kind("a = [1, 2\n"), ParseErrorKind::UnclosedArray);
        assert_eq!(err_kind("a = 1 b = 2\n"), ParseErrorKind::ExpectedNewline);
        assert_eq!(err_kind("[] \n"), ParseErrorKind::ExpectedKey);
        assert_eq!(
            err_kind("a = 99999999999999999999\n"),
            ParseErrorKind::IntegerOutOfRange
        );
        assert_eq!(
            err_kind("a = 0xFFFF_FFFF_FFFF_FFFF\n"),
            ParseErrorKind::IntegerOutOfRange
        );
        assert_eq!(err_kind("a = 1.\n"), ParseErrorKind::InvalidLiteral);
        assert_eq!(err_kind("a = 1e\n"), ParseErrorKind::InvalidLiteral);
        assert_eq!(err_kind("a = 0x\n"), ParseErrorKind::InvalidLiteral);
        assert_eq!(err_kind("a = +0x1\n"), ParseErrorKind::InvalidLiteral);
        assert_eq!(err_kind("a = .5\n"), ParseErrorKind::ExpectedValue);
    }

    #[test]
    fn 深度_128_を超えるとエラー() {
        // dotted key で 129 段
        let key: alloc::string::String = core::iter::repeat_n("k", 129)
            .collect::<alloc::vec::Vec<_>>()
            .join(".");
        let src = alloc::format!("{key} = 1\n");
        assert_eq!(err_kind(&src), ParseErrorKind::DepthExceeded);

        // 配列のネスト 129 段
        let src = alloc::format!("a = {}1{}\n", "[".repeat(129), "]".repeat(129));
        assert_eq!(err_kind(&src), ParseErrorKind::DepthExceeded);
    }

    #[test]
    fn テーブル末尾の_end_span_は次のヘッダ直前() {
        let src = "a = 1\n[t]\nb = 2\n";
        let d = parse(src);
        // ルートのセクション末尾は最初のヘッダの直前
        assert_eq!(d.root().end_span().start, src.find("[t]").unwrap());
        // 最後のセクションの末尾は EOF
        let t = d.root().get("t").unwrap().node().as_table().unwrap();
        assert_eq!(t.end_span().start, src.len());
    }
}
