//! 本文の分量（読了時間・文字数）を数える。
//!
//! 数えるのは本文の文章（`Text` と行内コード）だけで、コードブロック・図（mermaid は
//! コードブロック）・数式・生 HTML・frontmatter はノードの種類で自然に外れる。
//! 画像の代替テキストは本文として読まれないので、画像の配下は数えない。
//!
//! 読了時間は、日本語（かな・漢字・全角の約物など）は 1 分 500 字、英数字は
//! 1 分 200 語として足し合わせ、切り上げる。速度は固定（設定キーを持たない）

use comrak::nodes::{AstNode, NodeValue};

use crate::model::ReadingStats;

/// 日本語の文字の読了速度（字/分）
const CJK_CHARS_PER_MINUTE: f64 = 500.0;
/// 英数字の語の読了速度（語/分）
const WORDS_PER_MINUTE: f64 = 200.0;

/// 文書全体（`root`）の分量を数える
pub(crate) fn count<'a>(root: &'a AstNode<'a>) -> ReadingStats {
    let mut tally = Tally::default();
    for node in root.descendants() {
        if inside_image(node) {
            continue;
        }
        match &node.data.borrow().value {
            NodeValue::Text(text) => tally.add(text),
            NodeValue::Code(code) => tally.add(&code.literal),
            _ => {}
        }
    }
    tally.finish()
}

#[derive(Default)]
struct Tally {
    chars: usize,
    cjk_chars: usize,
    words: usize,
}

impl Tally {
    fn add(&mut self, text: &str) {
        let mut in_word = false;
        for c in text.chars() {
            if c.is_whitespace() {
                in_word = false;
                continue;
            }
            self.chars += 1;
            if is_cjk(c) {
                self.cjk_chars += 1;
                in_word = false;
            } else if c.is_alphanumeric() {
                if !in_word {
                    self.words += 1;
                    in_word = true;
                }
            } else {
                // 記号・約物（半角）は語の区切り
                in_word = false;
            }
        }
    }

    fn finish(self) -> ReadingStats {
        let minutes = if self.chars == 0 {
            0
        } else {
            let exact =
                self.cjk_chars as f64 / CJK_CHARS_PER_MINUTE + self.words as f64 / WORDS_PER_MINUTE;
            (exact.ceil() as usize).max(1)
        };
        ReadingStats {
            chars: self.chars,
            minutes,
        }
    }
}

/// 日本語・中国語・韓国語の文字か（字数で読む側）。
/// かな・漢字に加え、和文の約物（。、「」）と全角の英数字・記号も含める
fn is_cjk(c: char) -> bool {
    matches!(c,
        '\u{3000}'..='\u{303F}' // 和文の約物・記号
        | '\u{3040}'..='\u{309F}' // ひらがな
        | '\u{30A0}'..='\u{30FF}' // カタカナ
        | '\u{3400}'..='\u{4DBF}' // 漢字（拡張 A）
        | '\u{4E00}'..='\u{9FFF}' // 漢字
        | '\u{AC00}'..='\u{D7AF}' // ハングル
        | '\u{F900}'..='\u{FAFF}' // 漢字（互換）
        | '\u{FF01}'..='\u{FF60}' // 全角の英数字・記号
        | '\u{FF66}'..='\u{FF9F}' // 半角カナ
    )
}

/// 画像の配下（代替テキスト）か
fn inside_image<'a>(node: &'a AstNode<'a>) -> bool {
    node.ancestors()
        .skip(1)
        .any(|n| matches!(n.data.borrow().value, NodeValue::Image(_)))
}

#[cfg(test)]
mod tests {
    use comrak::{Arena, Options, parse_document};

    use super::count;
    use crate::model::ReadingStats;

    fn stats(source: &str) -> ReadingStats {
        let arena = Arena::new();
        let mut options = Options::default();
        options.extension.front_matter_delimiter = Some("---".to_string());
        options.extension.table = true;
        options.extension.math_dollars = true;
        count(parse_document(&arena, source, &options))
    }

    #[test]
    fn 日本語は字数_英数字は語数で数える() {
        // 「本文です。」= 5 字。空白は数えない
        assert_eq!(
            stats("本文です。\n"),
            ReadingStats {
                chars: 5,
                minutes: 1
            }
        );
        // 500 字ちょうどで 1 分、501 字で 2 分（切り上げ）
        assert_eq!(stats(&"あ".repeat(500)).minutes, 1);
        assert_eq!(stats(&"あ".repeat(501)).minutes, 2);
        // 英語は語数: 200 語で 1 分、201 語で 2 分
        assert_eq!(stats(&"word ".repeat(200)).minutes, 1);
        assert_eq!(stats(&"word ".repeat(201)).minutes, 2);
        // 混在は足し合わせる（250 字 = 0.5 分 + 100 語 = 0.5 分 → 1 分）
        let mixed = format!("{}\n\n{}", "あ".repeat(250), "word ".repeat(100));
        assert_eq!(stats(&mixed).minutes, 1);
    }

    #[test]
    fn 本文の文章以外は数えない() {
        let source = "---\ntitle: とても長いタイトル\n---\n\n本文\n\n```rust\nfn main() {}\n```\n\n```mermaid\ngraph TD; A-->B\n```\n\n$x^2$\n\n<div>生 HTML</div>\n\n![代替テキスト](a.png)\n";
        // 数えるのは「本文」の 2 字だけ
        assert_eq!(
            stats(source),
            ReadingStats {
                chars: 2,
                minutes: 1
            }
        );
    }

    #[test]
    fn 見出し_表_行内コードは本文として数える() {
        let source = "# 見出し\n\n| 列 |\n| --- |\n| 値 |\n\n`code` を使う\n";
        // 見出し 3 + 列 1 + 値 1 + code 4 + を使う 3 = 12 字
        assert_eq!(stats(source).chars, 12);
    }

    #[test]
    fn 文章が無ければ_0_分() {
        assert_eq!(stats(""), ReadingStats::default());
        assert_eq!(stats("```\ncode\n```\n"), ReadingStats::default());
    }
}
