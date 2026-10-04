//! frontmatter（YAML）のパース

use crate::model::Frontmatter;

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
];

/// comrak の front matter extension が切り出した生テキスト
/// （`---` 区切り行を含む）から YAML 部分を取り出してパースする
pub(crate) fn parse_frontmatter(raw: &str) -> Result<Frontmatter, String> {
    let body = yaml_body(raw);
    if body.is_empty() {
        return Ok(Frontmatter::default());
    }
    serde_yaml_ng::from_str(body).map_err(|e| e.to_string())
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
/// 中身にキーの行が 1 つも無ければ、区切り線の誤読とみる。
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
/// 中身が空でなく、キーの行が 1 つも無ければ誤読とみる（コメント行の後にキーが
/// 続く正しい frontmatter は誤読にしない）
pub(crate) fn looks_like_misread_thematic_break(raw: &str) -> bool {
    let body = yaml_body(raw);
    !body.is_empty() && !body.lines().any(|l| is_yaml_key_line(l.trim_start()))
}

/// 生テキストから `---` 区切りを外した YAML 部分を返す
pub(crate) fn yaml_body(raw: &str) -> &str {
    let trimmed = raw.trim();
    trimmed
        .strip_prefix("---")
        .and_then(|s| s.strip_suffix("---"))
        .unwrap_or(trimmed)
        .trim()
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
    /// （フィールドを追加して KNOWN_KEYS を忘れると未知キー lint が誤検知する）
    #[test]
    fn known_keys_は_frontmatter_のフィールドと一致する() {
        let yaml = serde_yaml_ng::to_string(&crate::model::Frontmatter::default()).unwrap();
        let value: serde_yaml_ng::Value = serde_yaml_ng::from_str(&yaml).unwrap();
        let mut fields: Vec<String> = value
            .as_mapping()
            .unwrap()
            .keys()
            .map(|k| k.as_str().unwrap().to_string())
            .collect();
        fields.sort();
        let mut known: Vec<String> = super::KNOWN_KEYS.iter().map(|k| k.to_string()).collect();
        known.sort();
        assert_eq!(fields, known);
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
        // コメント行の後にキーが続く正しい frontmatter・空の frontmatter は誤読にしない
        assert_eq!(detect("---\n# メモ\ntitle: x\n---\n本文\n"), None);
        assert_eq!(detect("---\n\ntitle: x\n---\n本文\n"), None);
        assert_eq!(detect("---\n---\n本文\n"), None);
    }
}
