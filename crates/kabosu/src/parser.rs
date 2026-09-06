//! TOML の文法（v0.1 対応範囲）と木構築。
//!
//! 再帰下降（改行を文法に含める行指向ハイブリッド）。重複キー・テーブル競合は
//! 挿入時に検出し、最初の 1 件で `ParseError` として停止する。
//! テーブルの再定義規則は TOML 1.0 に従う:
//! - ヘッダ経路の中間として暗黙に作られたテーブルは、後から `[a]` で定義できる
//! - dotted key で作られたテーブルをヘッダで再定義することはできない（逆も同じ）

use alloc::string::String;
use alloc::vec::Vec;

use crate::error::{ParseError, ParseErrorKind, UnsupportedFeature};
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
            comments.push(cur.read_comment());
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

/// `[a.b.c]` ヘッダ行
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
    if cur.peek() == Some(b'[') {
        return Err(ParseError::new(
            ParseErrorKind::Unsupported(UnsupportedFeature::ArrayOfTables),
            Span {
                start,
                end: start + 2,
            },
        ));
    }

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
    if segments.len() > MAX_DEPTH {
        return Err(ParseError::new(
            ParseErrorKind::DepthExceeded,
            Span {
                start,
                end: cur.pos(),
            },
        ));
    }

    define_header_table(root, &segments)?;
    *current_path = segments.iter().map(|s| String::from(s.name())).collect();

    cur.skip_ws();
    if cur.at_comment() {
        comments.push(cur.read_comment());
    }
    cur.eat_newline()
}

/// ヘッダ経路をルートから辿り、終端テーブルを定義する
fn define_header_table(root: &mut Table, segments: &[KeySegment]) -> Result<(), ParseError> {
    let mut t = root;
    for (i, seg) in segments.iter().enumerate() {
        let last = i == segments.len() - 1;
        if t.get(seg.name()).is_none() {
            let origin = if last {
                TableOrigin::Header
            } else {
                TableOrigin::HeaderImplicit
            };
            let mut table = Table::new(origin);
            table.set_end_span(Span::point(seg.span().end));
            t.insert(seg.clone(), Node::new(Value::Table(table), seg.span()));
            let entry = t.get_mut(seg.name()).expect("直前に挿入した");
            let Value::Table(sub) = entry.node_mut().value_mut() else {
                unreachable!("直前にテーブルを挿入した");
            };
            t = sub;
            continue;
        }
        let prev_span = t.get(seg.name()).expect("存在確認済み").key_span();
        let entry = t.get_mut(seg.name()).expect("存在確認済み");
        let Value::Table(sub) = entry.node_mut().value_mut() else {
            // 値が入っているキーへのテーブル定義
            return Err(ParseError::with_previous(
                ParseErrorKind::TableConflict,
                seg.span(),
                prev_span,
            ));
        };
        if sub.origin() == TableOrigin::Dotted {
            // dotted key で作られたテーブルはヘッダで再定義できない
            return Err(ParseError::with_previous(
                ParseErrorKind::TableConflict,
                seg.span(),
                prev_span,
            ));
        }
        if last {
            if sub.origin() == TableOrigin::HeaderImplicit {
                sub.set_origin(TableOrigin::Header);
            } else {
                // `[a]` の再定義
                return Err(ParseError::with_previous(
                    ParseErrorKind::DuplicateKey,
                    seg.span(),
                    prev_span,
                ));
            }
        }
        t = sub;
    }
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

    let depth = current_path.len() + segments.len();
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
        comments.push(cur.read_comment());
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

/// ヘッダ確定済みの経路を辿る
fn table_at_mut<'t>(root: &'t mut Table, path: &[String]) -> &'t mut Table {
    let mut t = root;
    for name in path {
        let entry = t.get_mut(name).expect("ヘッダ確定済みの経路");
        let Value::Table(sub) = entry.node_mut().value_mut() else {
            unreachable!("ヘッダ経路は常にテーブル");
        };
        t = sub;
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
        Some(b'{') => Err(ParseError::new(
            ParseErrorKind::Unsupported(UnsupportedFeature::InlineTable),
            Span {
                start: cur.pos(),
                end: cur.pos() + 1,
            },
        )),
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
    if depth + 1 > MAX_DEPTH {
        return Err(ParseError::new(
            ParseErrorKind::DepthExceeded,
            Span::point(start),
        ));
    }
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

/// 配列内の空白・改行・コメントを読み飛ばす
fn skip_trivia(cur: &mut Cursor<'_>, comments: &mut Vec<Span>) -> Result<(), ParseError> {
    loop {
        cur.skip_ws();
        if cur.at_comment() {
            comments.push(cur.read_comment());
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
    use crate::error::{ParseErrorKind, UnsupportedFeature};
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
    fn 未対応構文は位置付きで区別される() {
        assert_eq!(
            err_kind("x = { a = 1 }\n"),
            ParseErrorKind::Unsupported(UnsupportedFeature::InlineTable)
        );
        assert_eq!(
            err_kind("[[x]]\n"),
            ParseErrorKind::Unsupported(UnsupportedFeature::ArrayOfTables)
        );
        // span はエラー箇所を指す
        let e = Document::parse("x = { a = 1 }\n").unwrap_err();
        assert_eq!((e.span().start, e.span().end), (4, 5));
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
