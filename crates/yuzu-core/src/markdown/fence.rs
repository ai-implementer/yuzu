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

/// 情報文字列の解釈で無視された指定（lint の `code-block-meta` 警告用）
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum FenceMetaIssue {
    /// 認識されないトークン（`showLineNumbers` のタイポ等）
    UnknownToken(String),
    /// `{…}` 内の解釈できない要素（数値でない・逆順レンジ・0）
    InvalidRangePart(String),
}

/// 情報文字列を（言語, メタ）へ解釈する。
///
/// 先頭トークンは言語。ただし先頭からメタ形（`=` を含む・`{` 始まり・行番号キーワード）
/// なら言語なしとしてメタ解釈する。トークン分割は二重引用符の中の空白を保持する
/// （`title="a b"` が 1 トークン）。描画経路は寛容（無視された指定は捨てる）で、
/// タイポの検出は lint が [`parse_fence_info_detailed`] で行う
pub(crate) fn parse_fence_info(info: &str) -> (Option<&str>, CodeBlockMeta) {
    let (lang, meta, _) = parse_fence_info_detailed(info);
    (lang, meta)
}

/// [`parse_fence_info`] の詳細版: 無視された指定も返す（lint 用）
pub(crate) fn parse_fence_info_detailed(
    info: &str,
) -> (Option<&str>, CodeBlockMeta, Vec<FenceMetaIssue>) {
    let mut meta = CodeBlockMeta::default();
    let mut issues = Vec::new();
    let mut lang: Option<&str> = None;
    for (i, token) in tokenize(info).enumerate() {
        if i == 0 && !is_meta_token(token) {
            lang = Some(token);
            continue;
        }
        apply_token(token, &mut meta, &mut issues);
    }
    (lang, meta, issues)
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

fn apply_token(token: &str, meta: &mut CodeBlockMeta, issues: &mut Vec<FenceMetaIssue>) {
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
        parse_line_ranges(body, &mut meta.highlight_lines, issues);
    } else {
        // 未知トークン: 描画では無視するが lint が警告できるよう記録する
        issues.push(FenceMetaIssue::UnknownToken(token.to_string()));
    }
}

/// `2,4-6` 形式の行範囲リスト。不正な要素（数値でない・逆順レンジ・0）は
/// 個別に無視し、無視した事実を issues に記録する
fn parse_line_ranges(body: &str, out: &mut Vec<(usize, usize)>, issues: &mut Vec<FenceMetaIssue>) {
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
        match range {
            Some((start, end)) if start >= 1 && start <= end => out.push((start, end)),
            _ => issues.push(FenceMetaIssue::InvalidRangePart(part.to_string())),
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
    fn 詳細版は無視した指定を順に返す() {
        let (lang, meta, issues) =
            parse_fence_info_detailed("rust foo=bar {2,x,0,6-4} showLineNumber");
        assert_eq!(lang, Some("rust"));
        assert_eq!(meta.highlight_lines, vec![(2, 2)], "有効な範囲だけ採用");
        assert_eq!(
            issues,
            vec![
                FenceMetaIssue::UnknownToken("foo=bar".to_string()),
                FenceMetaIssue::InvalidRangePart("x".to_string()),
                FenceMetaIssue::InvalidRangePart("0".to_string()),
                FenceMetaIssue::InvalidRangePart("6-4".to_string()),
                FenceMetaIssue::UnknownToken("showLineNumber".to_string()),
            ]
        );
        // 正しい指定なら issues なし
        let (_, _, issues) =
            parse_fence_info_detailed(r#"rust title="src/main.rs" {2,4-6} showLineNumbers"#);
        assert!(issues.is_empty());
    }

    #[test]
    fn メタ全部入りと_is_empty() {
        let (lang, meta) = parse(r#"rust title="src/main.rs" {2,4-6} showLineNumbers"#);
        assert_eq!(lang, Some("rust"));
        assert!(!meta.is_empty());
        assert!(parse("rust").1.is_empty());
    }
}
