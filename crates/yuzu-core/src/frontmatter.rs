//! frontmatter（YAML）の読み込み。
//!
//! YAML の読み込みは [`crate::yaml`]（openapi / jsonschema と共有）。ここではキーごとに型を
//! 検査し、書き間違いを位置付きで全件集める（yuzu.toml の設定エラーと同じ考え方）。
//! 未知のキーは読み飛ばす（`frontmatter-unknown-key` の lint が警告する）

use saphyr::ScalarStyle;

use crate::model::Frontmatter;
use crate::yaml::{self, Node, Plain, View, resolve, view};

/// [`Frontmatter`] が受理するトップレベルキー（lint の未知キー検出用）。
/// フィールドを増やすときはここにも足す（乖離は下のテストで検知する）
pub(crate) const KNOWN_KEYS: &[&str] = &[
    "title",
    "order",
    "draft",
    "description",
    "llms",
    "aliases",
    "lintDisable",
    "readingTime",
    "pageMd",
];

/// frontmatter の書き間違い 1 件。位置は raw の中のバイト位置で、raw はファイルの
/// 先頭から始まる（comrak は frontmatter を 1 行目からしか切り出さない）ので、
/// [`yaml::line_col`] で raw の中の行・列にすればそのままファイル上の位置になる
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Problem {
    pub offset: usize,
    pub message: String,
}

/// comrak の front matter extension が切り出した生テキスト
/// （`---` 区切り行を含む）から YAML 部分を取り出して読む。
/// 書き間違いはキーごとに検査して全件返す（構文エラーは最初の 1 件で止まる）
pub(crate) fn parse_frontmatter(raw: &str) -> Result<Frontmatter, Vec<Problem>> {
    let (start, body) = yaml_body_at(raw);
    let root = yaml::load(body).map_err(|e| {
        vec![Problem {
            offset: start + e.offset,
            message: e.message,
        }]
    })?;
    let mut decoder = Decoder {
        start,
        body,
        problems: Vec::new(),
    };
    let frontmatter = match root {
        Some(root) => decoder.frontmatter(&root),
        None => Frontmatter::default(),
    };
    if decoder.problems.is_empty() {
        Ok(frontmatter)
    } else {
        Err(decoder.problems)
    }
}

/// キーごとの型の検査。型が違う値は既定値のままにして、書き間違いを集める
struct Decoder<'b> {
    /// body の raw の中での開始位置
    start: usize,
    body: &'b str,
    problems: Vec<Problem>,
}

impl Decoder<'_> {
    fn frontmatter(&mut self, root: &Node<'_>) -> Frontmatter {
        let mut fm = Frontmatter::default();
        let map = match view(root) {
            View::Mapping(map) => map,
            // `---` の間がコメントだけ・`~` だけ
            View::Scalar { text, style, tag } if resolve(text, style, tag) == Plain::Null => {
                return fm;
            }
            _ => {
                self.push(
                    root,
                    "frontmatter は `title: ページ名` のような「キー: 値」の行で書いてください"
                        .to_string(),
                );
                return fm;
            }
        };
        // KNOWN_KEYS と同じ集合（全キーを読むことは下のテストが縛る）
        for (key, value) in map {
            let View::Scalar { text: key, .. } = view(key) else {
                continue;
            };
            match key {
                "title" => fm.title = self.string(key, value),
                "order" => fm.order = self.int(key, value),
                "draft" => self.bool(key, value, &mut fm.draft),
                "description" => fm.description = self.string(key, value),
                "llms" => self.bool(key, value, &mut fm.llms),
                "aliases" => fm.aliases = self.strings(key, value),
                "lintDisable" => fm.lint_disable = self.strings(key, value),
                "readingTime" => self.bool(key, value, &mut fm.reading_time),
                "pageMd" => self.bool(key, value, &mut fm.page_md),
                _ => {}
            }
        }
        fm
    }

    /// 文字列。スカラーは書いたとおりの原文（`title: 1.0` は `"1.0"`）、値なしは `None`
    fn string(&mut self, key: &str, node: &Node<'_>) -> Option<String> {
        match view(node) {
            View::Scalar { text, style, tag } => {
                (resolve(text, style, tag) != Plain::Null).then(|| text.to_string())
            }
            _ => {
                self.mismatch(key, node, "文字列", "");
                None
            }
        }
    }

    /// 整数。値なしは `None`（指定しないのと同じ）
    fn int(&mut self, key: &str, node: &Node<'_>) -> Option<i64> {
        let View::Scalar { text, style, tag } = view(node) else {
            self.mismatch(key, node, "整数", "");
            return None;
        };
        let out_of_range = match resolve(text, style, tag) {
            Plain::Int(v) => return Some(v),
            Plain::Null => return None,
            Plain::UInt(_) => true,
            // i64 にも u64 にも収まらない 10 進数は小数として読まれる
            Plain::Float(_) => is_decimal(unsigned(text)),
            _ => false,
        };
        if out_of_range {
            self.push(
                node,
                format!(
                    "`{key}` の整数が範囲外です（-9223372036854775808 から 9223372036854775807 まで）"
                ),
            );
        } else {
            let hint = int_hint(text, style);
            self.mismatch(key, node, "整数", &hint);
        }
        None
    }

    /// 真偽値。型が違えば `slot` を既定値のままにする
    fn bool(&mut self, key: &str, node: &Node<'_>, slot: &mut bool) {
        let View::Scalar { text, style, tag } = view(node) else {
            self.mismatch(key, node, "真偽値", "");
            return;
        };
        match resolve(text, style, tag) {
            Plain::Bool(b) => *slot = b,
            Plain::Null => self.push(
                node,
                format!("`{key}` に値がありません（`true` か `false` を書いてください）"),
            ),
            _ => {
                let hint = bool_hint(text, style);
                self.mismatch(key, node, "真偽値", &hint);
            }
        }
    }

    /// 文字列の配列。値なしは空
    fn strings(&mut self, key: &str, node: &Node<'_>) -> Vec<String> {
        match view(node) {
            View::Sequence(items) => {
                let mut out = Vec::new();
                for (n, item) in items.iter().enumerate() {
                    match view(item) {
                        View::Scalar { text, style, tag }
                            if resolve(text, style, tag) != Plain::Null =>
                        {
                            out.push(text.to_string());
                        }
                        _ => {
                            let found = kind_name(item);
                            self.push(
                                item,
                                format!(
                                    "`{key}` の {} 番目の型が違います（期待: 文字列、実際: {found}）",
                                    n + 1
                                ),
                            );
                        }
                    }
                }
                out
            }
            View::Scalar { text, style, tag } if resolve(text, style, tag) == Plain::Null => {
                Vec::new()
            }
            View::Scalar { text, .. } => {
                let hint = format!("。1 つだけでも `[{text}]` のように配列で書いてください");
                self.mismatch(key, node, "文字列の配列", &hint);
                Vec::new()
            }
            _ => {
                self.mismatch(key, node, "文字列の配列", "");
                Vec::new()
            }
        }
    }

    fn mismatch(&mut self, key: &str, node: &Node<'_>, expected: &str, hint: &str) {
        let found = kind_name(node);
        self.push(
            node,
            format!("`{key}` の型が違います（期待: {expected}、実際: {found}）{hint}"),
        );
    }

    fn push(&mut self, node: &Node<'_>, message: String) {
        self.problems.push(Problem {
            offset: self.start + yaml::byte_offset(self.body, node.span.start),
            message,
        });
    }
}

/// 値の種類の日本語名（型の違いの文言用。yuzu.toml の文言と同じ呼び方）
fn kind_name(node: &Node<'_>) -> &'static str {
    match view(node) {
        View::Scalar { text, style, tag } => match resolve(text, style, tag) {
            Plain::Null => "値なし",
            Plain::Bool(_) => "真偽値",
            Plain::Int(_) | Plain::UInt(_) => "整数",
            Plain::Float(_) => "小数",
            Plain::Str => "文字列",
        },
        View::Sequence(_) => "配列",
        View::Mapping(_) => "マッピング",
        View::Other => "値",
    }
}

/// 真偽値のつもりで書かれた文字列への案内。YAML 1.1 の真偽値（`yes` / `no`・`on` / `off`）は
/// YAML 1.2 では文字列なので、黙って読み替えずに書き直しを案内する（Phase 88 の決定）
fn bool_hint(text: &str, style: ScalarStyle) -> String {
    let yaml11 = matches!(
        text.to_ascii_lowercase().as_str(),
        "yes" | "no" | "on" | "off" | "y" | "n"
    );
    if style == ScalarStyle::Plain && yaml11 {
        format!(
            "。`{text}` は YAML 1.1 の真偽値の書き方で、YAML 1.2 では文字列です。`true` か `false` で書いてください"
        )
    } else if style != ScalarStyle::Plain && matches!(yaml::resolve_plain(text), Plain::Bool(_)) {
        "。引用符を外してください".to_string()
    } else {
        String::new()
    }
}

/// 整数のつもりで書かれた文字列への案内。先頭 0 の 10 進数（`010`）は YAML 1.1 では
/// 8 進数なので、どちらにも読み替えずに書き直しを案内する（Phase 88 の決定）
fn int_hint(text: &str, style: ScalarStyle) -> String {
    let digits = unsigned(text);
    if style != ScalarStyle::Plain {
        if matches!(yaml::resolve_plain(text), Plain::Int(_)) {
            return "。引用符を外してください".to_string();
        }
    } else if digits.len() > 1 && digits.starts_with('0') && is_decimal(digits) {
        return format!(
            "。`{text}` は YAML 1.1 では 8 進数の書き方です。先頭の 0 を外して書いてください"
        );
    } else if digits.contains('_') && is_decimal(&digits.replace('_', "")) {
        return "。`_` で区切らずに書いてください".to_string();
    }
    String::new()
}

/// 先頭の符号を外す
fn unsigned(text: &str) -> &str {
    text.strip_prefix(['+', '-']).unwrap_or(text)
}

/// 空でない 10 進の数字だけの並びか
fn is_decimal(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())
}

/// frontmatter のつもりで書かれたのに、frontmatter として読まれていない形
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Unrecognized {
    /// `---` で始まり 2 行目が `キー:` なのに、閉じの `---` が無い
    UnclosedYaml,
    /// `+++` で始まり 2 行目が `キー =`（TOML 形式。yuzu は YAML だけ読む）
    Toml,
    /// 先頭の区切り線から次の `---` までが frontmatter として読まれている
    /// （comrak は中身を問わず切り出す。YAML がコメント扱いで通ると本文が黙って消える）
    MisreadThematicBreak,
}

impl Unrecognized {
    /// 診断・警告の文面。直し方と、区切り線のつもりだった場合の逃げ道を添える
    pub(crate) fn message(self) -> &'static str {
        match self {
            Self::UnclosedYaml => {
                "frontmatter の閉じの `---` がありません（本文として表示されます）。frontmatter なら属性の後に `---` だけの行を足してください。区切り線のつもりなら `---` の次に空行を入れてください"
            }
            Self::Toml => {
                "TOML 形式の frontmatter（`+++`）には対応していません（本文として表示されます）。`---` で囲んだ YAML で書いてください"
            }
            Self::MisreadThematicBreak => {
                "先頭の `---` から次の `---` までが frontmatter として読まれ、本文に表示されません。先頭の区切り線のつもりなら `***` で書いてください"
            }
        }
    }
}

/// frontmatter の候補なのに frontmatter として読まれていない（または区切り線が
/// frontmatter として読まれている）かを判定する。
///
/// 先頭の `---` は CommonMark では正当な区切り線でもあるので、「先頭が `---` で
/// 閉じが無い」だけでは閉じ忘れと区別できない（`---`・空行・見出し の文書は正常）。
/// **2 行目が ASCII のキーと `:` の形**（YAML のマッピング行）のときだけ閉じ忘れの
/// 候補とする。2 行目が空行・見出し・日本語の文（`用語: 説明`）なら候補にしない。
///
/// 逆に、2 行目がキーの形でないのに comrak が frontmatter として切り出していて、
/// 中身が YAML のマッピングとして読めなければ、区切り線の誤読とみる
/// （[`looks_like_misread_thematic_break`]。引用符付きキー・フロー形式は正しい frontmatter）。
///
/// `frontmatter_raw` は comrak が切り出した frontmatter（区切り行込み）。
/// 1 行目が `---` のときだけ呼ぶ（comrak のパースを全ページで走らせないため）
pub(crate) fn detect_unrecognized(
    source: &str,
    frontmatter_raw: impl FnOnce() -> Option<String>,
) -> Option<Unrecognized> {
    let source = source.strip_prefix('\u{feff}').unwrap_or(source);
    let mut lines = source.lines();
    let (first, second) = (lines.next()?, lines.next()?);
    match first {
        // `+++` は comrak の区切り（`---`）と違うので閉じがあっても読まれない
        "+++" if is_toml_key_line(second) => Some(Unrecognized::Toml),
        "---" if is_yaml_key_line(second) => frontmatter_raw()
            .is_none()
            .then_some(Unrecognized::UnclosedYaml),
        "---" => frontmatter_raw()
            .is_some_and(|raw| looks_like_misread_thematic_break(&raw))
            .then_some(Unrecognized::MisreadThematicBreak),
        _ => None,
    }
}

/// 行頭の ASCII キー（英字か `_` で始まり、英数字・`_`・`-` が続く）の長さ
fn key_len(line: &str) -> Option<usize> {
    let mut chars = line.char_indices();
    let (_, c) = chars.next()?;
    if !(c.is_ascii_alphabetic() || c == '_') {
        return None;
    }
    Some(
        chars
            .find(|(_, c)| !(c.is_ascii_alphanumeric() || matches!(c, '_' | '-')))
            .map_or(line.len(), |(i, _)| i),
    )
}

/// `title: x` / `title:` の形か（YAML はコロンの後に空白か行末が要る）
fn is_yaml_key_line(line: &str) -> bool {
    key_len(line).is_some_and(|n| {
        line[n..]
            .trim_start_matches([' ', '\t'])
            .strip_prefix(':')
            .is_some_and(|rest| rest.is_empty() || rest.starts_with([' ', '\t']))
    })
}

/// `title = "x"` の形か
fn is_toml_key_line(line: &str) -> bool {
    key_len(line).is_some_and(|n| line[n..].trim_start_matches([' ', '\t']).starts_with('='))
}

/// comrak が切り出した frontmatter が、区切り線の誤読らしいか。
/// comrak は 1 行目が `---` で後ろに `---` だけの行があれば中身を問わず
/// frontmatter とみなすので、区切り線を 2 本使う文書は間が YAML として読まれる
/// （見出しの `# …` は YAML ではコメントなので、パースが通って本文が黙って消える）。
///
/// 判定は**中身を YAML として読んだ結果**で行う（行の形では見ない。引用符付きの
/// キー `"title": x` やフロー形式 `{title: x}` も正しい frontmatter のため）:
/// - マッピング → 正しい frontmatter（誤読ではない）
/// - 空でないのに null（コメントだけ = 見出しだけの区間）・文字列・配列 → 誤読
/// - YAML として壊れている → キーの形の行が 1 つも無ければ誤読（壊れた frontmatter の
///   エラーに区切り線の案内を添えるかどうかにだけ使う）
pub(crate) fn looks_like_misread_thematic_break(raw: &str) -> bool {
    let (_, body) = yaml_body_at(raw);
    if body.trim().is_empty() {
        return false;
    }
    match yaml::load(body) {
        Ok(Some(root)) => !matches!(view(&root), View::Mapping(_)),
        // コメントだけ
        Ok(None) => true,
        Err(_) => !body.lines().any(|l| is_yaml_key_line(l.trim_start())),
    }
}

/// トップレベルのキーと、その行（raw の中の 1 始まり = ファイル上の行）。
/// YAML として読めない・マッピングでないなら空（読めない frontmatter は build が止める）
pub(crate) fn top_level_keys(raw: &str) -> Vec<(String, usize)> {
    let (start, body) = yaml_body_at(raw);
    let Ok(Some(root)) = yaml::load(body) else {
        return Vec::new();
    };
    let View::Mapping(map) = view(&root) else {
        return Vec::new();
    };
    map.keys()
        .filter_map(|key| match view(key) {
            View::Scalar { text, .. } => {
                let offset = start + yaml::byte_offset(body, key.span.start);
                Some((text.to_string(), yaml::line_col(raw, offset).0))
            }
            _ => None,
        })
        .collect()
}

/// 生テキストから `---` 区切りを外した YAML 部分と、その raw の中での開始位置（バイト）。
/// 先頭の空行は外し（位置は開始位置に足す）、閉じの `---` の直前の改行は残す
/// （最後のキーがブロックスカラー `|` / `>` のとき、YAML の仕様どおり値の末尾に改行が付く。
/// 以前は切り出しで改行を落としていたので、最後のキーのときだけ改行が無かった。Phase 88 の決定）
pub(crate) fn yaml_body_at(raw: &str) -> (usize, &str) {
    let lead = raw.len() - raw.trim_start().len();
    let trimmed = raw.trim();
    let (start, inner) = match trimmed
        .strip_prefix("---")
        .and_then(|s| s.strip_suffix("---"))
    {
        Some(inner) => (lead + "---".len(), inner),
        None => (lead, trimmed),
    };
    // 空行だけを外す（最初のキーの字下げは残す = 全体を字下げしたマッピングも読める）
    let space = inner.len() - inner.trim_start().len();
    let blank = inner[..space].rfind('\n').map_or(0, |i| i + 1);
    (start + blank, &inner[blank..])
}

#[cfg(test)]
mod tests {
    use super::parse_frontmatter;

    #[test]
    fn 基本キーをパースできる() {
        let fm = parse_frontmatter("---\ntitle: はじめに\norder: 2\ndraft: true\n---\n").unwrap();
        assert_eq!(fm.title.as_deref(), Some("はじめに"));
        assert_eq!(fm.order, Some(2));
        assert!(fm.draft);
        assert!(fm.description.is_none());
    }

    #[test]
    fn 空の_frontmatter_はデフォルトになる() {
        let fm = parse_frontmatter("---\n---\n").unwrap();
        assert!(fm.title.is_none());
        assert!(!fm.draft);
    }

    #[test]
    fn llms_は省略時_true_で_false_を指定できる() {
        let fm = parse_frontmatter("---\ntitle: x\n---\n").unwrap();
        assert!(fm.llms, "省略時は収録する");
        let fm = parse_frontmatter("---\nllms: false\n---\n").unwrap();
        assert!(!fm.llms);
    }

    #[test]
    fn page_md_は省略時_true_で_false_を指定できる() {
        let fm = parse_frontmatter("---\ntitle: x\n---\n").unwrap();
        assert!(fm.page_md, "省略時は原稿の .md を配信する");
        let fm = parse_frontmatter("---\npageMd: false\n---\n").unwrap();
        assert!(!fm.page_md);
    }

    #[test]
    fn lintdisable_はリストで受理し省略時は空() {
        let fm = parse_frontmatter("---\nlintDisable:\n  - term-variant\n  - duplicate-h1\n---\n")
            .unwrap();
        assert_eq!(fm.lint_disable, ["term-variant", "duplicate-h1"]);
        let fm = parse_frontmatter("---\nlintDisable: [katakana-choon]\n---\n").unwrap();
        assert_eq!(fm.lint_disable, ["katakana-choon"], "インライン形式も受理");
        let fm = parse_frontmatter("---\ntitle: x\n---\n").unwrap();
        assert!(fm.lint_disable.is_empty());
    }

    #[test]
    fn 未知のキーは無視する() {
        let fm = parse_frontmatter("---\ntitle: x\nfuture_key: 123\n---\n").unwrap();
        assert_eq!(fm.title.as_deref(), Some("x"));
    }

    #[test]
    fn 不正な_yaml_はエラーになる() {
        assert!(parse_frontmatter("---\ntitle: [unclosed\n---\n").is_err());
    }

    /// KNOWN_KEYS と Frontmatter 構造体の乖離検知
    /// （フィールドを追加して KNOWN_KEYS を忘れると未知キー lint が誤検知する）。
    /// キー名はキャッシュの JSON と同じ serde の名前（camelCase の rename）で比べる
    #[test]
    fn known_keys_は_frontmatter_のフィールドと一致する() {
        let value = serde_json::to_value(crate::model::Frontmatter::default()).unwrap();
        let mut fields: Vec<String> = value.as_object().unwrap().keys().cloned().collect();
        fields.sort();
        let mut known: Vec<String> = super::KNOWN_KEYS.iter().map(|k| k.to_string()).collect();
        known.sort();
        assert_eq!(fields, known);
    }

    /// KNOWN_KEYS のすべてを読む（読み込みの match にキーを足し忘れると既定値のまま黙って残る）
    #[test]
    fn known_keys_をすべて読む() {
        let fm = parse_frontmatter(
            "---\ntitle: t\norder: -3\ndraft: true\ndescription: d\nllms: false\naliases: [a/]\nlintDisable: [x]\nreadingTime: false\npageMd: false\n---\n",
        )
        .unwrap();
        let value = serde_json::to_value(&fm).unwrap();
        let default = serde_json::to_value(crate::model::Frontmatter::default()).unwrap();
        for key in super::KNOWN_KEYS {
            assert_ne!(value[key], default[key], "`{key}` が既定値のまま");
        }
    }

    /// 書き間違いの (行, 列, 文言)。raw の行・列 = ファイル上の行・列
    fn problems(raw: &str) -> Vec<(usize, usize, String)> {
        parse_frontmatter(raw)
            .unwrap_err()
            .into_iter()
            .map(|p| {
                let (line, col) = crate::yaml::line_col(raw, p.offset);
                (line, col, p.message)
            })
            .collect()
    }

    #[test]
    fn 型の違いは日本語でファイル上の位置と全件を返す() {
        // 先頭の空行・コメントがあっても行はファイル上の行
        let got = problems(
            "---\n\n# メモ\ntitle: x\norder: \"最初\"\ndraft: maybe\naliases: [a, [b]]\n---\n",
        );
        assert_eq!(
            got,
            [
                (
                    5,
                    8,
                    "`order` の型が違います（期待: 整数、実際: 文字列）".to_string()
                ),
                (
                    6,
                    8,
                    "`draft` の型が違います（期待: 真偽値、実際: 文字列）".to_string()
                ),
                (
                    7,
                    14,
                    "`aliases` の 2 番目の型が違います（期待: 文字列、実際: 配列）".to_string()
                ),
            ]
        );
    }

    #[test]
    fn 列はバイト基準で数える() {
        // 「あいう」は 9 バイト（`[b]` は文字数なら 16 列目、バイトなら 22 列目）
        let got = problems("---\nタイトル: x\naliases: [あいう, [b]]\n---\n");
        assert_eq!((got[0].0, got[0].1), (3, 22));
        let got = problems("---\r\ntitle: x\r\norder: x\r\n---\r\n");
        assert_eq!((got[0].0, got[0].1), (3, 8), "CRLF でも行がずれない");
    }

    #[test]
    fn yaml_1_1_の書き方はエラーにして書き直しを案内する() {
        for (raw, expected) in [
            (
                "---\ndraft: yes\n---\n",
                "`yes` は YAML 1.1 の真偽値の書き方",
            ),
            (
                "---\nllms: Off\n---\n",
                "`Off` は YAML 1.1 の真偽値の書き方",
            ),
            (
                "---\norder: 010\n---\n",
                "`010` は YAML 1.1 では 8 進数の書き方",
            ),
            ("---\norder: 1_000\n---\n", "`_` で区切らずに"),
            ("---\ndraft: \"true\"\n---\n", "引用符を外して"),
            ("---\norder: '3'\n---\n", "引用符を外して"),
            ("---\naliases: old/\n---\n", "`[old/]` のように配列で"),
            ("---\ndraft:\n---\n", "`draft` に値がありません"),
            (
                "---\norder: 9223372036854775808\n---\n",
                "`order` の整数が範囲外です",
            ),
            ("---\n- a\n- b\n---\n", "「キー: 値」の行で書いて"),
        ] {
            let got = problems(raw);
            assert!(
                got.len() == 1 && got[0].2.contains(expected),
                "{raw:?} → {got:?}"
            );
        }
    }

    #[test]
    fn 文字列のキーは書いたとおりの原文で読む() {
        // serde_yaml_ng と同じ値（数値・真偽値の形でも文字列のキーなら原文）
        let fm = parse_frontmatter(
            "---\ntitle: 0x10\ndescription: 1.0\nlintDisable: [x, 1, true]\n---\n",
        )
        .unwrap();
        assert_eq!(fm.title.as_deref(), Some("0x10"));
        assert_eq!(fm.description.as_deref(), Some("1.0"));
        assert_eq!(fm.lint_disable, ["x", "1", "true"]);
        // 値なしは未指定と同じ（`aliases:` は空・`order:` は None）
        let fm = parse_frontmatter("---\ntitle: ~\norder:\naliases:\n---\n").unwrap();
        assert!(fm.title.is_none() && fm.order.is_none() && fm.aliases.is_empty());
        // 引用符付きの `null` は文字列
        let fm = parse_frontmatter("---\ntitle: 'null'\n---\n").unwrap();
        assert_eq!(fm.title.as_deref(), Some("null"));
    }

    #[test]
    fn 最後のキーのブロックスカラーも末尾の改行を残す() {
        let last = parse_frontmatter("---\ntitle: >\n  a\n  b\n---\n").unwrap();
        let middle = parse_frontmatter("---\ntitle: >\n  a\n  b\norder: 1\n---\n").unwrap();
        assert_eq!(last.title.as_deref(), Some("a b\n"));
        assert_eq!(last.title, middle.title, "キーの位置で値が変わらない");
        let strip = parse_frontmatter("---\ntitle: >-\n  a\n  b\n---\n").unwrap();
        assert_eq!(strip.title.as_deref(), Some("a b"));
    }

    #[test]
    fn 重複キーは構文エラーで位置を返す() {
        let got = problems("---\ntitle: a\ntitle: b\n---\n");
        assert_eq!((got[0].0, got[0].1), (3, 1));
        assert!(got[0].2.contains("重複"), "{got:?}");
    }

    #[test]
    fn トップレベルのキーをファイル上の行と一緒に返す() {
        let keys = super::top_level_keys("---\n\ntitle: x\nfoo:\n  bar: 1\n\"quoted\": 2\n---\n");
        assert_eq!(
            keys,
            [
                ("title".to_string(), 3),
                ("foo".to_string(), 4),
                ("quoted".to_string(), 6)
            ]
        );
        assert!(super::top_level_keys("---\ntitle: [x\n---\n").is_empty());
    }

    /// comrak の切り出しを模した判定（テストでは「閉じの `---` 行があれば切り出す」）
    fn detect(source: &str) -> Option<super::Unrecognized> {
        super::detect_unrecognized(source, || {
            let rest = source.strip_prefix("---\n")?;
            let end = rest
                .find("\n---\n")
                .or_else(|| rest.ends_with("\n---").then(|| rest.len() - "\n---".len()))?;
            Some(format!("---\n{}\n---", &rest[..end]))
        })
    }

    #[test]
    fn 閉じ忘れは_2_行目がキーの形のときだけ候補にする() {
        use super::Unrecognized::UnclosedYaml;
        assert_eq!(
            detect("---\ntitle: 閉じ忘れ\norder: 5\n\n本文\n"),
            Some(UnclosedYaml)
        );
        assert_eq!(detect("---\ntitle:\n\n本文\n"), Some(UnclosedYaml));
        assert_eq!(
            detect("\u{feff}---\nlintDisable: [x]\n"),
            Some(UnclosedYaml)
        );
        // 閉じがあれば通常の frontmatter
        assert_eq!(detect("---\ntitle: x\n---\n\n本文\n"), None);
    }

    /// 先頭の区切り線で始まる正常な文書を閉じ忘れと誤検出しない（PR #21 のレビュー指摘）
    #[test]
    fn 区切り線で始まる文書は候補にしない() {
        // 2 行目が空行・見出し・日本語の文・URL など、キーの形でないもの
        assert_eq!(detect("---\n\n# Introduction\n\n本文です。\n"), None);
        assert_eq!(detect("---\n# Introduction\n"), None);
        assert_eq!(detect("---\n用語: 説明\n"), None);
        assert_eq!(detect("---\nhttps://example.com\n"), None);
        // コロンの後に空白が無いのは YAML のマッピングではない
        assert_eq!(detect("---\ntitle:x\n"), None);
        // 1 行だけ・区切り線が 3 文字ちょうどでない
        assert_eq!(detect("---\n"), None);
        assert_eq!(detect("----\ntitle: x\n"), None);
    }

    #[test]
    fn toml_形式は閉じの有無によらず候補にする() {
        use super::Unrecognized::Toml;
        assert_eq!(detect("+++\ntitle = \"x\"\n+++\n\n本文\n"), Some(Toml));
        assert_eq!(detect("+++\ntitle=\"x\"\n"), Some(Toml));
        assert_eq!(detect("+++\n\n本文\n"), None);
    }

    #[test]
    fn 区切り線が_frontmatter_として読まれたら誤読として扱う() {
        use super::Unrecognized::MisreadThematicBreak;
        // 間が見出しだけ = YAML ではコメントで、パースが通って本文が消える
        assert_eq!(
            detect("---\n\n# 見出し\n\n---\n\n後半\n"),
            Some(MisreadThematicBreak)
        );
        assert_eq!(
            detect("---\n\n前半の本文。\n---\n後半\n"),
            Some(MisreadThematicBreak)
        );
        // 区切り線の間が箇条書きだけ（YAML では配列）も誤読
        assert_eq!(
            detect("---\n\n- 項目1\n- 項目2\n\n---\n後半\n"),
            Some(MisreadThematicBreak)
        );
        // コメント行の後にキーが続く正しい frontmatter・空の frontmatter は誤読にしない
        assert_eq!(detect("---\n# メモ\ntitle: x\n---\n本文\n"), None);
        assert_eq!(detect("---\n\ntitle: x\n---\n本文\n"), None);
        assert_eq!(detect("---\n---\n本文\n"), None);
    }

    /// YAML として正しいマッピングは、行の形に依らず誤読にしない
    /// （PR #22 のレビュー指摘の回帰テスト: 引用符付きキー・フロー形式）
    #[test]
    fn 引用符付きキーやフロー形式の_frontmatter_は誤読にしない() {
        for source in [
            "---\n\"title\": Quoted\n---\n本文\n",
            "---\n'title': Single\norder: 1\n---\n本文\n",
            "---\n{title: Flow, order: 1}\n---\n本文\n",
            "---\n? title\n: 明示キー\n---\n本文\n",
        ] {
            assert_eq!(detect(source), None, "{source:?}");
        }
    }

    #[test]
    fn 壊れた_yaml_はキーの形の行が無いときだけ誤読とみる() {
        use super::looks_like_misread_thematic_break as misread;
        // キーの形の行がある壊れた frontmatter は、区切り線の案内を添えない
        assert!(!misread("---\ntitle: [閉じていない\n---"));
        // キーの形の行が無く YAML としても壊れている = 区切り線の誤読とみる
        assert!(misread("---\n\n# 見出し\n\n本文: [閉じていない\n---"));
    }
}
