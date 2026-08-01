//! Markdown 断片のインクルード（` ```include file="snippets/note.md" `）。
//!
//! 共通の注意書き・免責文を複数ページで再利用する。断片は本文の AST へ
//! 展開されるので、リンクの URL 書き換え・断片内コードのハイライト・
//! 折りたたみ等は通常の本文と同じ経路を通る。
//!
//! **断片は散文専用**（v1）。見出し・図表キャプション行・脚注・frontmatter・
//! `file=` 付きフェンス（入れ子のインクルード含む）は [`violations`] が検出し、
//! `yuzu check` が `include-error` として報告する。描画は寛容にそのまま
//! 継続する（check がゲート・描画は止めない、という既存方針）。
//!
//! - 見出し禁止の理由: TOC・アンカー採番・meta キャッシュが断片に依存すると、
//!   3 経路（extract_meta / 本文 HTML / extract_plain_sections）の採番同期と
//!   キャッシュ無効化を断片まで広げる必要がある
//! - `file=` 入れ子禁止の理由: 検索の deps ハッシュ（yuzu-index）は入れ子の
//!   参照先を追わないため、「参照先を編集しても検索が古い」を再導入してしまう
//! - 脚注禁止の理由: 断片は独立してパースされるので、脚注セクションが
//!   取り込み先ページの途中に紛れ込む

use comrak::nodes::NodeValue;
use comrak::{Arena, parse_document};

use crate::MarkdownOptions;
use crate::markdown::{crossref, escape_html};

/// 断片インクルードのフェンス言語トークン。
///
/// この言語のフェンスは yuzu-render のフックに**届かない**（core が展開して
/// 消すため）。`is_special_render_lang` には意図的に入れない — あの集合は
/// render のディスパッチと同期する契約で、検索のゲートも正反対
/// （特別言語 = 索引除外、断片 = 常に索引）
pub const FRAGMENT_LANG: &str = "include";

/// 断片の読み込み失敗の表示（yuzu-render の `include_error_box` と同型。
/// ビルドは止めず、公開前の検出は `yuzu check` が担う）
pub(crate) fn error_box(message: &str, path: &str) -> String {
    format!(
        "<div class=\"markdown-alert markdown-alert-caution\">\n\
         <p class=\"markdown-alert-title\">断片の読み込みに失敗しました</p>\n\
         <p>{}</p>\n</div>\n<pre><code>file=\"{}\"</code></pre>\n",
        escape_html(message),
        escape_html(path),
    )
}

/// 断片テキストを散文専用の規約で検査する（`yuzu check` 用）。
///
/// `lines=` 切り出し**後**のテキストを渡すこと（範囲外の見出しで鳴らさない）。
/// 脚注定義を位置のまま見るため keep_footnotes 版オプションでパースする。
/// メッセージは「断片 {path} 」が前置される前提の述部で返す
pub(crate) fn violations(text: &str, opts: &MarkdownOptions) -> Vec<String> {
    let arena = Arena::new();
    let options = crate::markdown::comrak_options_keep_footnotes(opts);
    let root = parse_document(&arena, text, &options);

    let mut out = Vec::new();
    for node in root.descendants() {
        let data = node.data.borrow();
        match &data.value {
            NodeValue::FrontMatter(_) => out.push(
                "に frontmatter があります（断片では無視されます。削除してください）".to_string(),
            ),
            NodeValue::Heading(_) => out.push(
                "に見出しがあります（断片は散文専用です。見出しは取り込み先の目次・アンカー採番とずれます）"
                    .to_string(),
            ),
            NodeValue::Paragraph
                if crossref::parse_caption(&crate::markdown::collect_text(node)).is_some() =>
            {
                out.push(
                    "に図表キャプション行があります（断片は散文専用です。図表番号の採番がずれます）"
                        .to_string(),
                )
            }
            NodeValue::CodeBlock(cb) => {
                let (lang, meta) = crate::markdown::fence::parse_fence_info(&cb.info);
                if lang == Some(FRAGMENT_LANG) || meta.include.is_some() {
                    out.push(
                        "の中で file= 付きのフェンスは使えません（入れ子のインクルードは非対応です）"
                            .to_string(),
                    );
                }
            }
            NodeValue::FootnoteDefinition(_) | NodeValue::FootnoteReference(_) => out.push(
                "に脚注があります（脚注は取り込み先ページの脚注セクションと衝突します）"
                    .to_string(),
            ),
            _ => {}
        }
    }
    out
}

/// 断片テキストのプレーンテキスト抽出（検索用）。
///
/// 記法（強調・リンク等）を落とし、ブロック境界で改行を入れる。
/// 検索 UI の抜粋にそのまま出るため、生 Markdown を返さない。
/// Anchorizer は**通さない**（断片に見出しは来ない契約。来ても check が
/// エラーにする。ここで見出し境界を作ると 3 経路のアンカー同期が壊れる）。
/// コードブロック・HTML・frontmatter は含めない（`file=` 入れ子禁止と整合）
pub(crate) fn collect_plain_text(text: &str, opts: &MarkdownOptions) -> String {
    let arena = Arena::new();
    let options = crate::markdown::comrak_options(opts);
    let root = parse_document(&arena, text, &options);

    let mut out = String::new();
    for node in root.descendants() {
        let data = node.data.borrow();
        match &data.value {
            NodeValue::Text(t) => out.push_str(t),
            NodeValue::Code(c) => out.push_str(&c.literal),
            NodeValue::Math(m) => out.push_str(&m.literal),
            NodeValue::SoftBreak | NodeValue::LineBreak => out.push(' '),
            // ブロックの開始で改行を入れる（トークナイズの文脈を切る）
            NodeValue::Paragraph | NodeValue::Item(_)
                if !out.is_empty() && !out.ends_with('\n') =>
            {
                out.push('\n');
            }
            _ => {}
        }
    }
    if !out.is_empty() && !out.ends_with('\n') {
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn エラーボックスはメッセージとパスをエスケープする() {
        let html = error_box("<msg> & \"x\"", "a\"b.md");
        assert!(html.contains("&lt;msg&gt; &amp; &quot;x&quot;"), "{html}");
        assert!(html.contains("file=&quot;a&quot;b.md&quot;") || !html.contains("file=\"a\"b"));
        assert!(!html.contains("<msg>"));
    }

    #[test]
    fn プレーンテキスト抽出は記法を落とす() {
        let text = collect_plain_text(
            "これは**強調**と[リンクラベル](https://example.com)です。\n\n次の段落。\n",
            &MarkdownOptions::default(),
        );
        assert!(text.contains("これは強調とリンクラベルです。"), "{text}");
        assert!(!text.contains("**"), "{text}");
        assert!(!text.contains("example.com"), "URL は索引しない: {text}");
        assert!(text.contains("次の段落。"));
    }

    #[test]
    fn 散文違反を種類ごとに検出する() {
        let opts = MarkdownOptions::default();
        let cases: &[(&str, &str)] = &[
            ("# 見出し\n", "見出し"),
            ("Figure: 図の説明 {#fig:x}\n", "キャプション"),
            ("```include file=\"other.md\"\n```\n", "入れ子"),
            ("```rust file=\"src/a.rs\"\n```\n", "入れ子"),
            ("本文[^a]。\n\n[^a]: 脚注。\n", "脚注"),
            ("---\ntitle: t\n---\n\n本文。\n", "frontmatter"),
        ];
        for (src, expect) in cases {
            let vs = violations(src, &opts);
            assert!(!vs.is_empty(), "{expect} を検出するべき: {src:?} -> {vs:?}");
        }
    }

    #[test]
    fn 違反のない散文は空を返す() {
        let src = "共通の注意書きです。**強調**と[リンク](/guide/)も使えます。\n\n\
                   - リスト\n- も入る\n\n> [!NOTE]\n> 注記も散文の一部。\n\n\
                   ```rust\nfn main() {}\n```\n";
        assert!(violations(src, &MarkdownOptions::default()).is_empty());
    }
}
