//! YAML の読み込み口（frontmatter・frontmatter の lint・openapi / jsonschema が共有）。
//!
//! パーサは saphyr（YAML 1.2）。スカラーは原文のまま受け取り（`early_parse` 無効）、
//! 型の解釈は [`resolve`] が行う。解釈は置き換え前の serde_yaml_ng に合わせてある
//! （先頭 0 の 10 進数・`_` 区切りは文字列、`0b`・符号付きの 16 進 / 8 進は整数、u64 まで）。
//! 読み込みでそろえるもの:
//! - 重複キーはエラー。引用符やタグだけが違う同名のキー（`draft` と `"draft"`）も重複とみる
//!   （saphyr は `early_parse` 無効だとスタイルとタグも比べて別のキーとして通すので、[`validate`] が見る）
//! - 明示のコアタグ（`!!bool` / `!!int` / `!!float` / `!!null`）に合わない値はエラー
//!   （serde_yaml_ng と同じ。合う値は引用符より優先して型を決める）
//! - 別名（alias）で展開するノード数に上限を掛ける（saphyr は展開を数えないので、
//!   入れ子の別名で指数的に膨らむ文書がメモリを食い尽くす）
//! - 位置は 1 始まりの行とバイト基準の列（yuzu.toml のエラー・診断と同じ規約）。
//!   saphyr の位置は文字数なので [`byte_offset`] で直す

use std::collections::{HashMap, HashSet};

use saphyr::{AnnotatedMapping, MarkedYaml, ScalarStyle, Tag, YamlData, YamlLoader};
use saphyr_parser::{Event, Marker, Parser, ScanError, Span, SpannedEventReceiver};

/// 位置付きのノード
pub(crate) type Node<'a> = MarkedYaml<'a>;

/// 別名で展開するノード数の上限（文書全体の合計）。
/// 共通部品を別名で使い回す程度の文書は収まり、入れ子の別名で膨らむ文書は止まる
/// （10 個ずつ 6 段で 100 万ノード）
const MAX_ALIAS_NODES: usize = 200_000;

/// YAML を読めなかった理由（[`to_json`] の公開用。位置は入力の中の 1 始まりの行・バイト基準の列）
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct YamlError {
    pub line: usize,
    pub col: usize,
    pub message: String,
}

impl std::fmt::Display for YamlError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}（{} 行 {} 列）", self.message, self.line, self.col)
    }
}

impl std::error::Error for YamlError {}

/// 読み込みのエラー（位置は入力の中のバイト位置）
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LoadError {
    pub offset: usize,
    pub message: String,
}

/// 1 つの文書を位置付きの木として読む。文書が無い（空・コメントだけ）なら `None`
pub(crate) fn load(text: &str) -> Result<Option<Node<'_>>, LoadError> {
    let mut receiver = Budgeted::default();
    receiver.loader.early_parse(false);
    let scanned = Parser::new_from_str(text).load(&mut receiver, true);
    // 上限を超えた後は loader へ渡していないので、loader 側の結果より先に見る
    if let Some(mark) = receiver.exceeded {
        return Err(LoadError {
            offset: byte_offset(text, mark),
            message: format!(
                "別名（`*名前`）で展開する値が多すぎます（上限 {MAX_ALIAS_NODES} 個）"
            ),
        });
    }
    if let Some(e) = scanned.err().as_ref().or(receiver.loader.error()) {
        return Err(LoadError {
            offset: byte_offset(text, *e.marker()),
            message: scan_message(e),
        });
    }
    let mut docs = receiver.loader.into_documents();
    if let Some(second) = docs.get(1) {
        return Err(LoadError {
            offset: byte_offset(text, second.span.start),
            message: "YAML の文書が複数あります（`---` で区切った 2 つ目以降は読めません）"
                .to_string(),
        });
    }
    let root = docs.pop();
    if let Some(Err((mark, message))) = root.as_ref().map(validate) {
        return Err(LoadError {
            offset: byte_offset(text, mark),
            message,
        });
    }
    Ok(root)
}

/// 重複キーの文言（saphyr が見つけた重複と [`validate`] が見つけた重複で同じにする）
const DUPLICATE_KEY: &str =
    "キーが重複しています（同じキーは 1 回だけ書けます。引用符の有無は区別しません）";

/// saphyr が見ない 2 つを確かめる: 引用符やタグだけが違う同名のキーと、明示のコアタグに
/// 合わない値。キーはスカラーの原文で比べる（frontmatter も JSON への写しもキーを原文で使うので、
/// 原文が同じなら同じキーになる）
fn validate(node: &Node<'_>) -> Result<(), (Marker, String)> {
    match view(node) {
        View::Scalar { text, tag, .. } => {
            if let Some(message) = tag_problem(text, tag) {
                return Err((node.span.start, message));
            }
        }
        View::Sequence(items) => {
            for item in items {
                validate(item)?;
            }
        }
        View::Mapping(map) => {
            let mut seen = HashSet::new();
            for (key, value) in map {
                if let View::Scalar { text, .. } = view(key)
                    && !seen.insert(text)
                {
                    return Err((key.span.start, DUPLICATE_KEY.to_string()));
                }
                validate(key)?;
                validate(value)?;
            }
        }
        View::Other => {}
    }
    Ok(())
}

/// 明示のコアタグに合わない値の文言（`!!int "x"`・`!!bool yes`）
fn tag_problem(text: &str, tag: Option<&Tag>) -> Option<String> {
    let suffix = core_tag(tag)?;
    let resolved = resolve_tagged(text, suffix)?;
    let (fits, kind) = match suffix {
        "bool" => (matches!(resolved, Plain::Bool(_)), "真偽値"),
        "int" => (matches!(resolved, Plain::Int(_) | Plain::UInt(_)), "整数"),
        "float" => (matches!(resolved, Plain::Float(_)), "小数"),
        _ => (resolved == Plain::Null, "値なし（null）"),
    };
    (!fits).then(|| format!("`!!{suffix}` のタグが付いた `{text}` は{kind}として読めません"))
}

/// コアタグ（`tag:yaml.org,2002:` の `!!bool` など）の名前
fn core_tag(tag: Option<&Tag>) -> Option<&str> {
    tag.filter(|t| t.is_yaml_core_schema())
        .map(|t| t.suffix.as_str())
}

/// 型を指定するコアタグ（`!!bool` / `!!int` / `!!float` / `!!null`）が付いた値の解釈。
/// 引用符の有無に依らず原文を解釈する。ほかのタグは `None`（タグで型を決めない）
fn resolve_tagged(text: &str, suffix: &str) -> Option<Plain> {
    // ブロック（`!!int |`）の値は末尾に改行が付くので外してから読む
    let resolved = resolve_plain(text.trim_end_matches(['\n', '\r']));
    match suffix {
        "bool" | "int" | "null" => Some(resolved),
        // `!!float 1` は小数の 1.0
        "float" => Some(match resolved {
            Plain::Int(v) => Plain::Float(v as f64),
            Plain::UInt(v) => Plain::Float(v as f64),
            other => other,
        }),
        _ => None,
    }
}

/// 別名の展開を数えながら saphyr の loader へイベントを渡す。
/// アンカーごとに部分木のノード数（入れ子の別名の展開を含む）を覚え、別名 1 つで
/// その数だけ足す。上限を超えたら以降を渡さない（loader が部分木を複製しない）
#[derive(Default)]
struct Budgeted<'a> {
    loader: YamlLoader<'a, Node<'a>>,
    /// ここまでのノード数（別名の展開を含む）
    nodes: usize,
    /// 別名で展開したノード数
    via_alias: usize,
    /// アンカー ID → 部分木のノード数
    anchors: HashMap<usize, usize>,
    /// 開いている集合の（アンカー ID、開いた時点のノード数）
    open: Vec<(usize, usize)>,
    exceeded: Option<Marker>,
}

impl<'a> SpannedEventReceiver<'a> for Budgeted<'a> {
    fn on_event(&mut self, ev: Event<'a>, span: Span) {
        if self.exceeded.is_some() {
            return;
        }
        match &ev {
            Event::Scalar(_, _, anchor, _) => {
                self.nodes += 1;
                if *anchor > 0 {
                    self.anchors.insert(*anchor, 1);
                }
            }
            Event::SequenceStart(anchor, _) | Event::MappingStart(anchor, _) => {
                self.open.push((*anchor, self.nodes));
                self.nodes += 1;
            }
            Event::SequenceEnd | Event::MappingEnd => {
                if let Some((anchor, start)) = self.open.pop()
                    && anchor > 0
                {
                    self.anchors.insert(anchor, self.nodes - start);
                }
            }
            Event::Alias(id) => {
                let size = self.anchors.get(id).copied().unwrap_or(1);
                self.nodes += size;
                self.via_alias += size;
                if self.via_alias > MAX_ALIAS_NODES {
                    self.exceeded = Some(span.start);
                    return;
                }
            }
            _ => {}
        }
        self.loader.on_event(ev, span);
    }
}

/// saphyr の構文エラーの文言を日本語にする（よく当たるものだけ。ほかは原文を添える）
fn scan_message(e: &ScanError) -> String {
    let info = e.info();
    let message = match info {
        "duplicated key in mapping" => DUPLICATE_KEY,
        "while parsing a flow sequence, expected ',' or ']'" => {
            "`[` で始めた配列が閉じていません（`]` がありません）"
        }
        "while parsing a flow mapping, did not find expected ',' or '}'" => {
            "`{` で始めたマッピングが閉じていません（`}` がありません）"
        }
        "while scanning a quoted scalar, found unexpected end of stream" => {
            "引用符で始めた文字列が閉じていません"
        }
        "mapping values are not allowed in this context" => {
            "値の中に `: ` があります（`: ` を含む値は引用符で囲んでください）"
        }
        "while parsing a block mapping, did not find expected key" => {
            "キーの字下げがそろっていません"
        }
        "while parsing a quoted scalar, found unknown escape character" => {
            "`\"` で囲んだ文字列に使えない `\\` があります（`\\` を含む値は `'...'` で囲んでください）"
        }
        "while parsing node, found unknown anchor" => "定義されていない別名（`*名前`）があります",
        "recursion limit exceeded" => "入れ子が深すぎます",
        // saphyr は `title:<タブ>値` を通さない（serde_yaml_ng は通していた）
        "':' must be followed by a valid YAML whitespace" => {
            "`:` の後にタブがあります（`:` の後は空白にしてください）"
        }
        "'-' must be followed by a valid YAML whitespace" => {
            "`-` の後にタブがあります（`-` の後は空白にしてください）"
        }
        _ if info.contains("tab") => "タブで字下げしています（字下げは空白で書いてください）",
        _ => return format!("YAML の構文エラーです（{info}）"),
    };
    message.to_string()
}

/// saphyr の位置（文字数）を入力の中のバイト位置にする
pub(crate) fn byte_offset(text: &str, mark: Marker) -> usize {
    text.char_indices()
        .nth(mark.index())
        .map_or(text.len(), |(i, _)| i)
}

/// バイト位置を 1 始まりの行・バイト基準の列にする（yuzu.toml のエラーと同じ数え方）
pub(crate) fn line_col(text: &str, offset: usize) -> (usize, usize) {
    let head = &text[..offset.min(text.len())];
    let line = head.matches('\n').count() + 1;
    let col = head.rsplit_once('\n').map_or(head.len(), |(_, l)| l.len()) + 1;
    (line, col)
}

/// ノードの中身（タグ付きの集合は中を見る。位置はタグの外側のノードを使うこと =
/// saphyr はタグの中のノードに位置を付けない）
pub(crate) enum View<'n, 'a> {
    Scalar {
        text: &'n str,
        style: ScalarStyle,
        tag: Option<&'n Tag>,
    },
    Sequence(&'n [Node<'a>]),
    Mapping(&'n AnnotatedMapping<'a, Node<'a>>),
    /// 読み込みでは作られない（解決できない別名は構文エラーになる）
    Other,
}

pub(crate) fn view<'n, 'a>(node: &'n Node<'a>) -> View<'n, 'a> {
    match &node.data {
        YamlData::Representation(text, style, tag) => View::Scalar {
            text,
            style: *style,
            tag: tag.as_deref(),
        },
        YamlData::Sequence(items) => View::Sequence(items),
        YamlData::Mapping(map) => View::Mapping(map),
        YamlData::Tagged(_, inner) => view(inner),
        _ => View::Other,
    }
}

/// スカラーの解釈（YAML 1.2 の core schema。境界は serde_yaml_ng に合わせる）
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Plain {
    Null,
    Bool(bool),
    Int(i64),
    /// i64 に収まらない正の整数
    UInt(u64),
    Float(f64),
    Str,
}

/// スカラーを解釈する。明示のコアタグを引用符より先に見る（`!!bool "true"` は真偽値・
/// `!!str 1` は文字列。serde_yaml_ng と同じ。タグに合わない値は [`load`] がエラーにしている）。
/// タグが無ければ、引用符・ブロック（`|` / `>`）は文字列
pub(crate) fn resolve(text: &str, style: ScalarStyle, tag: Option<&Tag>) -> Plain {
    match core_tag(tag) {
        Some("str") => Plain::Str,
        Some(suffix) => match resolve_tagged(text, suffix) {
            Some(resolved) => resolved,
            None => untagged(text, style),
        },
        None => untagged(text, style),
    }
}

fn untagged(text: &str, style: ScalarStyle) -> Plain {
    if style == ScalarStyle::Plain {
        resolve_plain(text)
    } else {
        Plain::Str
    }
}

/// 引用符の無いスカラーを解釈する
pub(crate) fn resolve_plain(text: &str) -> Plain {
    match text {
        "" | "~" | "null" | "Null" | "NULL" => Plain::Null,
        "true" | "True" | "TRUE" => Plain::Bool(true),
        "false" | "False" | "FALSE" => Plain::Bool(false),
        _ => int_of(text)
            .or_else(|| float_of(text))
            .unwrap_or(Plain::Str),
    }
}

/// 符号の後が 10 進・`0x`・`0o`・`0b` の整数か。先頭 0 の 10 進数（`010`）は YAML 1.1 の
/// 8 進数と紛らわしいので文字列にする（serde_yaml_ng と同じ）。範囲外の 10 進数は小数
fn int_of(text: &str) -> Option<Plain> {
    let (negative, digits) = match text.as_bytes().first() {
        Some(b'-') => (true, &text[1..]),
        Some(b'+') => (false, &text[1..]),
        _ => (false, text),
    };
    let (radix, body) = if let Some(hex) = digits.strip_prefix("0x") {
        (16, hex)
    } else if let Some(oct) = digits.strip_prefix("0o") {
        (8, oct)
    } else if let Some(bin) = digits.strip_prefix("0b") {
        (2, bin)
    } else if digits.len() > 1 && digits.starts_with('0') {
        return None;
    } else {
        (10, digits)
    };
    if body.is_empty() || !body.chars().all(|c| c.is_digit(radix)) {
        return None;
    }
    let magnitude = u64::from_str_radix(body, radix).ok()?;
    if negative {
        // i64::MIN の絶対値は i64 に収まらないので 0 から引く
        0i64.checked_sub_unsigned(magnitude).map(Plain::Int)
    } else {
        Some(i64::try_from(magnitude).map_or(Plain::UInt(magnitude), Plain::Int))
    }
}

/// `1.5`・`.5`・`5.`・`1e3`・`.inf`・`.nan` の形か。`.` も指数も無い数字だけの並びは
/// 整数の側で扱う（先頭 0 の `010` を小数として読まない）。有限でない結果（`1e400`）は文字列
fn float_of(text: &str) -> Option<Plain> {
    let unsigned = text.strip_prefix(['+', '-']).unwrap_or(text);
    if matches!(unsigned, ".inf" | ".Inf" | ".INF") {
        let sign = if text.starts_with('-') { -1.0 } else { 1.0 };
        return Some(Plain::Float(sign * f64::INFINITY));
    }
    if matches!(text, ".nan" | ".NaN" | ".NAN") {
        return Some(Plain::Float(f64::NAN));
    }
    let bytes = unsigned.as_bytes();
    let digits = |from: usize| {
        bytes[from..]
            .iter()
            .take_while(|b| b.is_ascii_digit())
            .count()
    };
    let mut i = digits(0);
    let int_digits = i;
    let mut point = false;
    let mut frac_digits = 0;
    if bytes.get(i) == Some(&b'.') {
        point = true;
        frac_digits = digits(i + 1);
        i += 1 + frac_digits;
    }
    if int_digits + frac_digits == 0 {
        return None;
    }
    let mut exponent = false;
    if matches!(bytes.get(i), Some(b'e' | b'E')) {
        i += 1;
        if matches!(bytes.get(i), Some(b'+' | b'-')) {
            i += 1;
        }
        let n = digits(i);
        if n == 0 {
            return None;
        }
        i += n;
        exponent = true;
    }
    if i != bytes.len() || !(point || exponent) {
        return None;
    }
    text.parse::<f64>()
        .ok()
        .filter(|v| v.is_finite())
        .map(Plain::Float)
}

/// YAML（JSON を含む）を JSON の値へ写す（openapi / jsonschema の読み込み）。
/// マッピングのキーはスカラーの原文（`200:` は `"200"`）。有限でない小数（`.inf`・`.nan`）は
/// JSON に無いので null。マッピングの記述順を保つ
pub fn to_json(text: &str) -> Result<serde_json::Value, YamlError> {
    let located = |offset: usize, message: String| {
        let (line, col) = line_col(text, offset);
        YamlError { line, col, message }
    };
    let root = load(text).map_err(|e| located(e.offset, e.message))?;
    match root {
        None => Ok(serde_json::Value::Null),
        Some(node) => json_of(&node).map_err(|mark| {
            located(
                byte_offset(text, mark),
                "マッピングのキーに配列やマッピングは使えません".to_string(),
            )
        }),
    }
}

/// ノードを JSON の値にする。キーが集合なら、その位置を返す
fn json_of(node: &Node<'_>) -> Result<serde_json::Value, Marker> {
    use serde_json::Value;
    Ok(match view(node) {
        View::Scalar { text, style, tag } => match resolve(text, style, tag) {
            Plain::Null => Value::Null,
            Plain::Bool(b) => Value::Bool(b),
            Plain::Int(v) => Value::from(v),
            Plain::UInt(v) => Value::from(v),
            Plain::Float(v) => serde_json::Number::from_f64(v).map_or(Value::Null, Value::Number),
            Plain::Str => Value::String(text.to_string()),
        },
        View::Sequence(items) => Value::Array(items.iter().map(json_of).collect::<Result<_, _>>()?),
        View::Mapping(map) => {
            let mut object = serde_json::Map::new();
            for (key, value) in map {
                let View::Scalar { text, .. } = view(key) else {
                    return Err(key.span.start);
                };
                object.insert(text.to_string(), json_of(value)?);
            }
            Value::Object(object)
        }
        View::Other => Value::Null,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn スカラーの解釈は置き換え前と同じ境界にする() {
        for (text, expected) in [
            ("~", Plain::Null),
            ("", Plain::Null),
            ("NULL", Plain::Null),
            ("nULL", Plain::Str),
            ("True", Plain::Bool(true)),
            ("tRUE", Plain::Str),
            ("yes", Plain::Str),
            ("0", Plain::Int(0)),
            ("-0", Plain::Int(0)),
            ("+5", Plain::Int(5)),
            ("010", Plain::Str),
            ("00", Plain::Str),
            ("1_000", Plain::Str),
            ("0x1F", Plain::Int(31)),
            ("0X1F", Plain::Str),
            ("-0x10", Plain::Int(-16)),
            ("0o17", Plain::Int(15)),
            ("-0o7", Plain::Int(-7)),
            ("0b101", Plain::Int(5)),
            ("0x", Plain::Str),
            ("-9223372036854775808", Plain::Int(i64::MIN)),
            (
                "9223372036854775808",
                Plain::UInt(9_223_372_036_854_775_808),
            ),
            ("18446744073709551615", Plain::UInt(u64::MAX)),
            ("1.0", Plain::Float(1.0)),
            (".5", Plain::Float(0.5)),
            ("5.", Plain::Float(5.0)),
            ("1e3", Plain::Float(1000.0)),
            ("-.Inf", Plain::Float(f64::NEG_INFINITY)),
            ("1e400", Plain::Str),
            ("1e", Plain::Str),
            (".", Plain::Str),
            ("inf", Plain::Str),
            ("3.0.1", Plain::Str),
            ("1:30", Plain::Str),
            ("2024-01-01", Plain::Str),
        ] {
            assert_eq!(resolve_plain(text), expected, "{text:?}");
        }
        assert!(matches!(resolve_plain(".nan"), Plain::Float(v) if v.is_nan()));
    }

    #[test]
    fn 引用符とブロックと_str_タグは文字列() {
        let root = load("a: \"1\"\nb: '~'\nc: |\n  true\nd: !!str 12\ne: !custom 12\n")
            .unwrap()
            .unwrap();
        let View::Mapping(map) = view(&root) else {
            panic!("マッピング");
        };
        let kinds: Vec<Plain> = map
            .values()
            .map(|v| match view(v) {
                View::Scalar { text, style, tag } => resolve(text, style, tag),
                _ => panic!("スカラー"),
            })
            .collect();
        assert_eq!(
            kinds,
            [
                Plain::Str,
                Plain::Str,
                Plain::Str,
                Plain::Str,
                Plain::Int(12)
            ]
        );
    }

    #[test]
    fn json_へ写すとキーは原文で順序を保つ() {
        let value = to_json("b: 1\na: [x, 2.5, ~, true]\n200: {z: .inf}\n").unwrap();
        assert_eq!(
            value,
            json!({"b": 1, "a": ["x", 2.5, null, true], "200": {"z": null}})
        );
        let keys: Vec<&String> = value.as_object().unwrap().keys().collect();
        assert_eq!(keys, ["b", "a", "200"], "記述順");
        // JSON もそのまま読める
        assert_eq!(
            to_json(r#"{"a": "あ", "b": [1, -2]}"#).unwrap(),
            json!({"a": "あ", "b": [1, -2]})
        );
        assert_eq!(to_json("").unwrap(), json!(null));
        assert_eq!(to_json("# コメントだけ\n").unwrap(), json!(null));
    }

    #[test]
    fn 別名は展開し入れ子で膨らむ文書は上限で止める() {
        assert_eq!(
            to_json("a: &x [1, 2]\nb: *x\n").unwrap(),
            json!({"a": [1, 2], "b": [1, 2]})
        );
        // 10 個ずつ 6 段 = 100 万ノード
        let mut text = String::from("a0: &a0 [x,x,x,x,x,x,x,x,x,x]\n");
        for i in 1..6 {
            let p = i - 1;
            let refs = vec![format!("*a{p}"); 10].join(",");
            text.push_str(&format!("a{i}: &a{i} [{refs}]\n"));
        }
        let e = to_json(&text).unwrap_err();
        assert!(e.message.contains("多すぎます"), "{e}");
        assert_eq!(e.line, 6, "上限を超えた別名の行: {e}");
    }

    #[test]
    fn エラーは日本語で位置は行とバイト基準の列() {
        let e = to_json("タイトル: x\ntitle: a\ntitle: b\n").unwrap_err();
        assert_eq!((e.line, e.col), (3, 1));
        assert!(e.message.contains("重複"), "{e}");
        // 列はバイト基準（「タイトル」は 12 バイト）
        let e = to_json("タイトル: [x\n").unwrap_err();
        assert!(e.message.contains("閉じていません"), "{e}");
        let e = to_json("title:\tx\n").unwrap_err();
        assert!(e.message.contains("タブ"), "{e}");
        let e = to_json("a: 1\n---\nb: 2\n").unwrap_err();
        assert_eq!(e.line, 3, "2 つ目の文書の位置: {e}");
        assert!(e.message.contains("複数"), "{e}");
        assert_eq!(
            to_json("[a]: 1\n").unwrap_err().message,
            "マッピングのキーに配列やマッピングは使えません"
        );
    }

    /// PR #55 のレビュー指摘: saphyr は `early_parse` 無効だとスタイルとタグも比べるので、
    /// 引用符だけが違う同名のキーを別のキーとして通し、後の値が黙って勝っていた
    #[test]
    fn 引用符やタグだけが違う同名のキーも重複にする() {
        for (text, line) in [
            ("a: 1\n\"a\": 2\n", 2),
            ("a: 1\n'a': 2\n", 2),
            ("200: x\n\"200\": y\n", 2),
            ("a: 1\n!!str a: 2\n", 2),
            ("x:\n  a: 1\n  b: 2\n  \"a\": 3\n", 4),
            ("- {a: 1, \"a\": 2}\n", 1),
        ] {
            let e = to_json(text).unwrap_err();
            assert_eq!(e.message, DUPLICATE_KEY, "{text:?}");
            assert_eq!(e.line, line, "{text:?}");
        }
        // 別の階層の同名キーは重複ではない
        assert!(to_json("a: 1\nb:\n  a: 2\n").is_ok());
    }

    /// PR #55 のレビュー指摘: 引用符を先に見て `!!bool "true"` を文字列にしていた
    /// （OpenAPI の `nullable: !!bool "true"` の「null 許容」が消えた）
    #[test]
    fn 明示のコアタグは引用符より先に見る() {
        assert_eq!(
            to_json(
                "b: !!bool \"true\"\nf: !!bool 'false'\ni: !!int \"42\"\nh: !!int \"0x10\"\nfl: !!float \"1\"\nn: !!null \"~\"\ns: !!str 1\nblock: !!int |\n  42\ncustom: !x \"1\"\n"
            )
            .unwrap(),
            json!({"b": true, "f": false, "i": 42, "h": 16, "fl": 1.0, "n": null, "s": "1", "block": 42, "custom": "1"})
        );
        // タグに合わない値はエラー（serde_yaml_ng と同じ）
        for (text, expected) in [
            (
                "v: !!int \"x\"\n",
                "`!!int` のタグが付いた `x` は整数として読めません",
            ),
            (
                "v: !!bool yes\n",
                "`!!bool` のタグが付いた `yes` は真偽値として読めません",
            ),
            (
                "v: !!int 010\n",
                "`!!int` のタグが付いた `010` は整数として読めません",
            ),
            (
                "v: !!null x\n",
                "`!!null` のタグが付いた `x` は値なし（null）として読めません",
            ),
        ] {
            let e = to_json(text).unwrap_err();
            assert_eq!(e.message, expected, "{text:?}");
            assert_eq!(e.line, 1, "値の位置: {text:?}");
        }
    }

    #[test]
    fn 位置の変換は文字数からバイトへ() {
        let text = "タイトル: 値\n";
        let root = load(text).unwrap().unwrap();
        let View::Mapping(map) = view(&root) else {
            panic!("マッピング");
        };
        let value = map.values().next().unwrap();
        let offset = byte_offset(text, value.span.start);
        assert_eq!(&text[offset..], "値\n");
        assert_eq!(line_col(text, offset), (1, 15));
    }
}
