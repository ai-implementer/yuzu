//! 用語集・略語（`markdown.glossary`）。
//!
//! - **辞書は設定に置く**（`lint.terms` と同じ思想）。本文の Markdown は
//!   1 バイトも変わらないので、素の Markdown ビューアでも読める
//! - 本文中の**ページ内初出だけ**を `<abbr title="説明">略語</abbr>` にする
//!   （出現のたびに点線下線が付くと設計書では読みづらい）
//! - 用語集ページの Markdown 原文もここで組み立てる。生成物は通常ページと
//!   同じ経路（nav・検索・sitemap・llms・linkcheck）に乗る
//!
//! このモジュールは **comrak を触らない純関数だけ**を持つ（`crossref` /
//! `collapse` / `tabs` と同じ規律）。AST の書き換えは `markdown/mod.rs` 側。

use crate::GlossaryOptions;

use super::escape_html;

/// テキストを辞書で分割した結果の 1 片。
///
/// 元の `Text` ノードのリテラルは `RefCell` の借用越しにしか読めず、借用を抜けた
/// 後段（適用フェーズ）で使うので**所有文字列で返す**。割り当てが起きるのは実際に
/// マッチしたノードだけなので、大半のテキストでは `None` が返って何も起きない
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Piece {
    /// そのまま残すテキスト
    Text(String),
    /// `<abbr title="…">略語</abbr>` にする箇所
    Abbr { term: String, desc: String },
}

/// 辞書を「長い用語から順に」引けるように整えた検索表。
/// 走査のたびにソートし直さないよう、ページ本文の走査前に 1 回だけ作る
#[derive(Debug)]
pub(crate) struct Matcher<'a> {
    /// (略語, 説明文)。文字数の降順（最長一致優先）
    entries: Vec<(&'a str, &'a str)>,
}

impl<'a> Matcher<'a> {
    /// 有効な辞書エントリから検索表を作る。空辞書なら `None`
    pub(crate) fn new(opts: &'a GlossaryOptions) -> Option<Self> {
        if !opts.abbr {
            return None;
        }
        let mut entries: Vec<(&str, &str)> = opts
            .terms
            .iter()
            .filter(|(term, desc)| !term.is_empty() && !desc.is_empty())
            .map(|(term, desc)| (term.as_str(), desc.as_str()))
            .collect();
        if entries.is_empty() {
            return None;
        }
        // 最長一致優先。同長は辞書順（BTreeMap 由来）で決定的
        entries.sort_by(|a, b| b.0.len().cmp(&a.0.len()).then_with(|| a.0.cmp(b.0)));
        Some(Self { entries })
    }

    /// 1 つのテキストリテラルを分割する。
    ///
    /// `used` は「このページで既に `<abbr>` 化した用語」。初出だけを対象にするため、
    /// マッチした用語をここへ入れながら進む（呼び出し側がページ単位で持ち回る）。
    /// 置換が 1 つも起きなければ `None`（＝木を触らない）
    pub(crate) fn split(
        &self,
        text: &str,
        used: &mut std::collections::HashSet<String>,
    ) -> Option<Vec<Piece>> {
        // 全用語が出尽くしたページ後半では 1 文字ずつ走査する意味がない
        if self.entries.iter().all(|(term, _)| used.contains(*term)) {
            return None;
        }
        let mut pieces: Vec<Piece> = Vec::new();
        let mut cursor = 0usize; // 未出力テキストの先頭
        let mut pos = 0usize; // 走査位置
        while pos < text.len() {
            // 文字境界だけを走査する（バイト境界で slice すると panic する）
            if !text.is_char_boundary(pos) {
                pos += 1;
                continue;
            }
            let rest = &text[pos..];
            let hit = self.entries.iter().find(|(term, _)| {
                !used.contains(*term) && rest.starts_with(term) && has_boundary(text, pos, term)
            });
            match hit {
                Some((term, desc)) => {
                    if cursor < pos {
                        pieces.push(Piece::Text(text[cursor..pos].to_string()));
                    }
                    pieces.push(Piece::Abbr {
                        term: (*term).to_string(),
                        desc: (*desc).to_string(),
                    });
                    used.insert((*term).to_string());
                    pos += term.len();
                    cursor = pos;
                }
                None => pos += 1,
            }
        }
        if pieces.is_empty() {
            return None;
        }
        if cursor < text.len() {
            pieces.push(Piece::Text(text[cursor..].to_string()));
        }
        Some(pieces)
    }
}

/// 用語の**端が ASCII 英数字のときだけ**単語境界を要求する。
///
/// `API` が `RAPID` の中でマッチしては困る一方、日本語の用語（`分かち書き` 等）に
/// 単語境界の概念は無く、要求すると和文中で一度もマッチしなくなる。
/// 端の字種で判断を分けるのが、両方を 1 実装で満たす唯一の形
fn has_boundary(text: &str, start: usize, term: &str) -> bool {
    let ascii_word = |c: char| c.is_ascii_alphanumeric() || c == '_';
    let first = term
        .chars()
        .next()
        .expect("空の用語は Matcher が除外している");
    if ascii_word(first) && text[..start].chars().next_back().is_some_and(ascii_word) {
        return false;
    }
    let last = term.chars().next_back().expect("同上");
    let end = start + term.len();
    if ascii_word(last) && text[end..].chars().next().is_some_and(ascii_word) {
        return false;
    }
    true
}

/// `<abbr title="説明">略語</abbr>`。title は属性文脈なので必ずエスケープする
pub(crate) fn abbr_open_tag(desc: &str) -> String {
    format!("<abbr title=\"{}\">", escape_html(desc))
}

pub(crate) const ABBR_CLOSE_TAG: &str = "</abbr>";

/// 用語集ページの content 相対パス（`glossary` → `glossary.md`）。
///
/// 辞書が空、または route が不正（[`crate::urlpath::synth_page_rel`] 参照）なら
/// `None` ＝ページを作らない
pub(crate) fn page_rel(opts: &GlossaryOptions) -> Option<std::path::PathBuf> {
    if opts.terms.is_empty() {
        return None;
    }
    crate::urlpath::synth_page_rel(&opts.page)
}

/// 用語集ページの Markdown 原文。
///
/// **`yuzu fmt` の正規形と一致させる**（`format_commonmark` が ATX 見出し・
/// ブロック間 1 行空け・末尾改行で出すのと同じ形）。一致していれば
/// llms-full.txt の `normalize_markdown` 出力も原文と揃う。
/// frontmatter は付けない — タイトルは h1 から解決され、
/// 付けると `frontmatter-unknown-key` lint やバイト温存規約と余計に絡む
pub(crate) fn page_markdown(
    title: &str,
    terms: &std::collections::BTreeMap<String, String>,
) -> String {
    let mut out = format!("# {title}\n");
    // BTreeMap のキー順 = 決定的（rayon の並列化があっても出力バイトは同一）
    for (term, desc) in terms {
        if term.is_empty() || desc.is_empty() {
            continue;
        }
        out.push_str(&format!("\n## {term}\n\n{}\n", desc.trim()));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{BTreeMap, HashSet};

    fn opts(pairs: &[(&str, &str)]) -> GlossaryOptions {
        GlossaryOptions {
            terms: pairs
                .iter()
                .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
                .collect::<BTreeMap<_, _>>(),
            ..GlossaryOptions::default()
        }
    }

    fn split_once_page(o: &GlossaryOptions, text: &str) -> Option<Vec<Piece>> {
        let m = Matcher::new(o)?;
        m.split(text, &mut HashSet::new())
    }

    #[test]
    fn 略語を分割する() {
        let o = opts(&[("API", "Application Programming Interface")]);
        let pieces = split_once_page(&o, "この API を使う").unwrap();
        assert_eq!(
            pieces,
            vec![
                Piece::Text("この ".into()),
                Piece::Abbr {
                    term: "API".into(),
                    desc: "Application Programming Interface".into()
                },
                Piece::Text(" を使う".into()),
            ]
        );
    }

    #[test]
    fn 単語の一部にはマッチしない() {
        let o = opts(&[("API", "Application Programming Interface")]);
        assert!(split_once_page(&o, "RAPID な開発").is_none());
        assert!(split_once_page(&o, "APIs は複数形").is_none());
        assert!(split_once_page(&o, "x_API_y").is_none());
    }

    #[test]
    fn 日本語の用語は境界を要求しない() {
        let o = opts(&[("分かち書き", "テキストを単語へ切る処理")]);
        let pieces = split_once_page(&o, "日本語の分かち書きは難しい").unwrap();
        assert_eq!(
            pieces,
            vec![
                Piece::Text("日本語の".into()),
                Piece::Abbr {
                    term: "分かち書き".into(),
                    desc: "テキストを単語へ切る処理".into()
                },
                Piece::Text("は難しい".into()),
            ]
        );
    }

    #[test]
    fn 最長一致を優先する() {
        let o = opts(&[("API", "インターフェース"), ("API キー", "認証用の文字列")]);
        let pieces = split_once_page(&o, "API キーを渡す").unwrap();
        assert_eq!(
            pieces,
            vec![
                Piece::Abbr {
                    term: "API キー".into(),
                    desc: "認証用の文字列".into()
                },
                Piece::Text("を渡す".into()),
            ]
        );
    }

    #[test]
    fn 初出だけを置換する() {
        let o = opts(&[("API", "インターフェース")]);
        let m = Matcher::new(&o).unwrap();
        let mut used = HashSet::new();
        assert!(m.split("最初の API", &mut used).is_some());
        // 同じページの 2 回目以降は触らない（used を持ち回るのが呼び出し側の責務）
        assert!(m.split("2 回目の API", &mut used).is_none());
    }

    #[test]
    fn 空のエントリと無効化を無視する() {
        assert!(Matcher::new(&opts(&[])).is_none());
        assert!(Matcher::new(&opts(&[("", "説明")])).is_none());
        assert!(Matcher::new(&opts(&[("API", "")])).is_none());
        let disabled = GlossaryOptions {
            abbr: false,
            ..opts(&[("API", "インターフェース")])
        };
        assert!(Matcher::new(&disabled).is_none());
    }

    #[test]
    fn title_はエスケープする() {
        assert_eq!(
            abbr_open_tag(r#"<b>"x"</b>"#),
            "<abbr title=\"&lt;b&gt;&quot;x&quot;&lt;/b&gt;\">"
        );
    }
}
