//! フェンス情報文字列（` ```rust title="src/main.rs" {2,4-6} showLineNumbers `）の解釈。
//!
//! 先頭トークンを言語、以降を表示メタとして解釈する。HTML 化（render 側フック）と
//! 検索抽出（`plain_sections`）で解釈を揃えるため、実装はここに 1 つだけ置く。
//! 未知のトークンは黙って無視する（他ツール由来の情報文字列を壊さない）。

/// フェンス情報文字列から解釈した表示メタ（言語の後ろのトークン）。
///
/// - `title="src/main.rs"` — キャプション（ファイル名など。無引用の `title=x` も可）
/// - `{2,4-6}` — ハイライトする行（1 始まり・両端含む。数値とレンジのカンマ区切り）
/// - `showLineNumbers` / `noLineNumbers` — 行番号表示のブロック単位上書き
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CodeBlockMeta {
    /// キャプション（`<figcaption>` に出す。エスケープは描画側の責務）
    pub title: Option<String>,
    /// ハイライト行の範囲リスト（1 始まり・両端含む・未ソート可）
    pub highlight_lines: Vec<(usize, usize)>,
    /// 行番号表示の上書き。`None` = サイト設定（`markdown.highlight.lineNumbers`）に従う
    pub line_numbers: Option<bool>,
}

impl CodeBlockMeta {
    /// メタ指定がひとつも無いか
    pub fn is_empty(&self) -> bool {
        self.title.is_none() && self.highlight_lines.is_empty() && self.line_numbers.is_none()
    }

    /// 1 始まりの行番号がハイライト対象か
    pub fn is_highlighted(&self, line: usize) -> bool {
        self.highlight_lines
            .iter()
            .any(|&(start, end)| start <= line && line <= end)
    }
}

/// 情報文字列を（言語, メタ）へ解釈する。
///
/// 先頭トークンは言語。ただし先頭からメタ形（`=` を含む・`{` 始まり・行番号キーワード）
/// なら言語なしとしてメタ解釈する。トークン分割は二重引用符の中の空白を保持する
/// （`title="a b"` が 1 トークン）。
pub(crate) fn parse_fence_info(info: &str) -> (Option<&str>, CodeBlockMeta) {
    let mut meta = CodeBlockMeta::default();
    let mut lang: Option<&str> = None;
    for (i, token) in tokenize(info).enumerate() {
        if i == 0 && !is_meta_token(token) {
            lang = Some(token);
            continue;
        }
        apply_token(token, &mut meta);
    }
    (lang, meta)
}

/// 空白区切り・二重引用符内の空白は保持するトークナイザ
fn tokenize(info: &str) -> impl Iterator<Item = &str> {
    let mut rest = info;
    std::iter::from_fn(move || {
        rest = rest.trim_start();
        if rest.is_empty() {
            return None;
        }
        let mut in_quotes = false;
        let end = rest
            .char_indices()
            .find(|&(_, c)| {
                if c == '"' {
                    in_quotes = !in_quotes;
                }
                c.is_whitespace() && !in_quotes
            })
            .map(|(i, _)| i)
            .unwrap_or(rest.len());
        let (token, tail) = rest.split_at(end);
        rest = tail;
        Some(token)
    })
}

fn is_meta_token(token: &str) -> bool {
    token.contains('=')
        || token.starts_with('{')
        || token == "showLineNumbers"
        || token == "noLineNumbers"
}

fn apply_token(token: &str, meta: &mut CodeBlockMeta) {
    if token == "showLineNumbers" {
        meta.line_numbers = Some(true);
    } else if token == "noLineNumbers" {
        meta.line_numbers = Some(false);
    } else if let Some(value) = token.strip_prefix("title=") {
        let value = value
            .strip_prefix('"')
            .and_then(|v| v.strip_suffix('"'))
            .unwrap_or(value);
        if !value.is_empty() {
            meta.title = Some(value.to_string());
        }
    } else if let Some(body) = token.strip_prefix('{').and_then(|t| t.strip_suffix('}')) {
        parse_line_ranges(body, &mut meta.highlight_lines);
    }
    // それ以外の未知トークンは無視
}

/// `2,4-6` 形式の行範囲リスト。不正な要素（数値でない・逆順レンジ・0）は個別に無視
fn parse_line_ranges(body: &str, out: &mut Vec<(usize, usize)>) {
    for part in body.split(',') {
        let part = part.trim();
        let range = match part.split_once('-') {
            Some((start, end)) => start
                .trim()
                .parse::<usize>()
                .ok()
                .zip(end.trim().parse::<usize>().ok()),
            None => part.parse::<usize>().ok().map(|n| (n, n)),
        };
        if let Some((start, end)) = range {
            if start >= 1 && start <= end {
                out.push((start, end));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(info: &str) -> (Option<&str>, CodeBlockMeta) {
        parse_fence_info(info)
    }

    #[test]
    fn 言語のみは従来どおり() {
        assert_eq!(parse("rust"), (Some("rust"), CodeBlockMeta::default()));
        assert_eq!(parse(""), (None, CodeBlockMeta::default()));
        assert_eq!(parse("  "), (None, CodeBlockMeta::default()));
    }

    #[test]
    fn title_は引用符付きで空白を含められる() {
        let (lang, meta) = parse(r#"rust title="src/main.rs""#);
        assert_eq!(lang, Some("rust"));
        assert_eq!(meta.title.as_deref(), Some("src/main.rs"));

        let (_, meta) = parse(r#"rust title="読み 方""#);
        assert_eq!(meta.title.as_deref(), Some("読み 方"));

        // 無引用も受理
        let (_, meta) = parse("rust title=main.rs");
        assert_eq!(meta.title.as_deref(), Some("main.rs"));

        // 空値は無視
        let (_, meta) = parse(r#"rust title="""#);
        assert_eq!(meta.title, None);
    }

    #[test]
    fn 行ハイライトは番号とレンジのカンマ区切り() {
        let (lang, meta) = parse("rust {2,4-6}");
        assert_eq!(lang, Some("rust"));
        assert_eq!(meta.highlight_lines, vec![(2, 2), (4, 6)]);
        assert!(meta.is_highlighted(2));
        assert!(!meta.is_highlighted(3));
        assert!(meta.is_highlighted(5));
        assert!(!meta.is_highlighted(7));
    }

    #[test]
    fn 不正なレンジ要素は個別に無視される() {
        let (_, meta) = parse("rust {0,2,x,6-4,3-3}");
        assert_eq!(meta.highlight_lines, vec![(2, 2), (3, 3)]);
    }

    #[test]
    fn 行番号の上書き() {
        assert_eq!(parse("rust showLineNumbers").1.line_numbers, Some(true));
        assert_eq!(parse("rust noLineNumbers").1.line_numbers, Some(false));
        assert_eq!(parse("rust").1.line_numbers, None);
    }

    #[test]
    fn 先頭トークンがメタ形なら言語なし() {
        let (lang, meta) = parse(r#"title="メモ" {1}"#);
        assert_eq!(lang, None);
        assert_eq!(meta.title.as_deref(), Some("メモ"));
        assert_eq!(meta.highlight_lines, vec![(1, 1)]);

        let (lang, meta) = parse("showLineNumbers");
        assert_eq!(lang, None);
        assert_eq!(meta.line_numbers, Some(true));
    }

    #[test]
    fn 未知トークンは無視して言語とメタだけ拾う() {
        let (lang, meta) = parse("rust foo=bar baz {2} showLineNumbers");
        assert_eq!(lang, Some("rust"));
        assert_eq!(meta.highlight_lines, vec![(2, 2)]);
        assert_eq!(meta.line_numbers, Some(true));
        assert_eq!(meta.title, None);
    }

    #[test]
    fn メタ全部入りと_is_empty() {
        let (lang, meta) = parse(r#"rust title="src/main.rs" {2,4-6} showLineNumbers"#);
        assert_eq!(lang, Some("rust"));
        assert!(!meta.is_empty());
        assert!(parse("rust").1.is_empty());
    }
}
