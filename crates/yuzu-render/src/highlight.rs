//! syntect によるビルド時シンタックスハイライト（CSS クラス出力）と
//! ` ```mermaid ` ブロックの変換。
//!
//! 凍結事項: インラインスタイルは使わない。配色はテーマ CSS
//! （ビルド時生成の `syntect.css`、ライト/ダーク両対応）が担う。

use syntect::html::{ClassStyle, ClassedHTMLGenerator};
use syntect::parsing::SyntaxSet;
use syntect::util::LinesWithEndings;

use yuzu_core::CodeBlockRenderer;

/// syntect のクラス名接頭辞。テーマ CSS のクラスと衝突しないようにする。
/// `css.rs` の CSS 生成と必ず同じ値を使うこと
pub(crate) const CLASS_STYLE: ClassStyle = ClassStyle::SpacedPrefixed { prefix: "yz-" };

/// [`CodeBlockRenderer`] の実装。
/// - `lang == "mermaid"` → `<pre class="mermaid">`（クライアント描画）
/// - 既知の言語 → syntect ハイライト（CSS クラス）
/// - 言語なし・未知の言語 → `None`（パーサ既定のエスケープ済み `<pre><code>`）
pub struct SyntectCodeRenderer {
    syntax_set: SyntaxSet,
    highlight_enabled: bool,
    mermaid_enabled: bool,
}

impl SyntectCodeRenderer {
    pub fn new(highlight_enabled: bool, mermaid_enabled: bool) -> Self {
        Self {
            // ClassedHTMLGenerator の行単位 API は newlines 版とセットで使う
            syntax_set: SyntaxSet::load_defaults_newlines(),
            highlight_enabled,
            mermaid_enabled,
        }
    }

    fn highlight(&self, lang: &str, code: &str) -> Option<String> {
        if !self.highlight_enabled {
            return None;
        }
        let syntax = self.syntax_set.find_syntax_by_token(lang)?;
        let mut generator =
            ClassedHTMLGenerator::new_with_class_style(syntax, &self.syntax_set, CLASS_STYLE);
        for line in LinesWithEndings::from(code) {
            if let Err(e) = generator.parse_html_for_line_which_includes_newline(line) {
                // ハイライト失敗でビルドは止めず、プレーン表示へフォールバック
                tracing::warn!(lang, error = %e, "ハイライトに失敗したためプレーン表示にする");
                return None;
            }
        }
        Some(generator.finalize())
    }
}

impl CodeBlockRenderer for SyntectCodeRenderer {
    fn render(&self, lang: Option<&str>, code: &str) -> Option<String> {
        let lang = lang?;
        if lang == "mermaid" {
            if !self.mermaid_enabled {
                return None;
            }
            return Some(format!(
                "<pre class=\"mermaid\">{}</pre>\n",
                escape_html(code)
            ));
        }
        let inner = self.highlight(lang, code)?;
        Some(format!(
            "<pre class=\"highlight\"><code class=\"language-{}\">{}</code></pre>\n",
            escape_html(lang),
            inner
        ))
    }
}

/// HTML エスケープ（テキストノード・属性値用の最小集合）
pub(crate) fn escape_html(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mermaid_はエスケープ済み_pre_になる() {
        let r = SyntectCodeRenderer::new(true, true);
        let html = r.render(Some("mermaid"), "A->>B: <hello>\n").unwrap();
        assert_eq!(
            html,
            "<pre class=\"mermaid\">A-&gt;&gt;B: &lt;hello&gt;\n</pre>\n"
        );
    }

    #[test]
    fn mermaid_無効なら_none() {
        let r = SyntectCodeRenderer::new(true, false);
        assert!(r.render(Some("mermaid"), "graph TD;").is_none());
    }

    #[test]
    fn rust_はハイライトされ_css_クラスが付く() {
        let r = SyntectCodeRenderer::new(true, true);
        let html = r.render(Some("rust"), "fn main() {}\n").unwrap();
        assert!(html.starts_with("<pre class=\"highlight\">"));
        assert!(
            html.contains("class=\"yz-"),
            "yz- 接頭辞のクラス出力: {html}"
        );
        assert!(!html.contains("style="), "インラインスタイル禁止: {html}");
    }

    #[test]
    fn 未知の言語と言語なしは_none() {
        let r = SyntectCodeRenderer::new(true, true);
        assert!(r.render(Some("unknown-lang-xyz"), "x").is_none());
        assert!(r.render(None, "x").is_none());
    }

    #[test]
    fn ハイライト無効なら_none() {
        let r = SyntectCodeRenderer::new(false, true);
        assert!(r.render(Some("rust"), "fn main() {}").is_none());
    }
}
