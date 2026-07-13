//! comrak を使う唯一のモジュール（外部へは型を漏らさない）。
//!
//! - パス1: [`extract_meta`] — frontmatter / 先頭 h1 / TOC（全見出し）を抽出
//! - パス2: [`render_body_html`] — コードブロック差し替え・URL 書き換えを
//!   AST 上で行ってから HTML 化
//!
//! ⚠️ アンカー ID の同期: comrak の `header_ids` 拡張は HTML 化時に内部の
//! `Anchorizer` で ID を採番する。TOC 側も**全見出しを文書順で**採番することで
//! 重複サフィックス（`-1` 等）を一致させている。片方だけ見出しを飛ばすとずれる。

use std::path::Path;

use comrak::nodes::{AstNode, NodeHtmlBlock, NodeValue};
use comrak::{Anchorizer, Arena, Options, format_commonmark, format_html, parse_document};

use crate::MarkdownOptions;
use crate::error::CoreError;
use crate::frontmatter::parse_frontmatter;
use crate::model::{Frontmatter, Page, PlainSection, SourceSpan, TocEntry};
use crate::traits::{CodeBlockRenderer, UrlRewriter};

/// comrak のオプションを組み立てる（凍結: GFM 拡張＋YAML frontmatter＋header_ids）。
///
/// - AST ノードの sourcepos は常に記録されるため `render.sourcepos` は不要
///   （HTML に `data-sourcepos` 属性を撒かない）
/// - `render.unsafe_ = true` はコードブロック差し替え（HtmlBlock の素通し）と
///   著者の生 HTML のため。docs は信頼できる入力という前提
fn comrak_options(opts: &MarkdownOptions) -> Options<'static> {
    let mut options = Options::default();
    if opts.gfm {
        options.extension.table = true;
        options.extension.strikethrough = true;
        options.extension.autolink = true;
        options.extension.tasklist = true;
        options.extension.alerts = true; // > [!NOTE] 等の Admonition（GitHub 互換 5 種）
        options.extension.footnotes = true; // [^name] 脚注
    }
    if opts.math {
        options.extension.math_dollars = true; // $...$ / $$...$$（通貨 $100 等は弾かれる）
        options.extension.math_code = true; // $`...`$（```math フェンスは CodeBlock のまま）
    }
    options.extension.front_matter_delimiter = Some("---".to_string());
    options.extension.header_id_prefix = Some(String::new());
    options.render.r#unsafe = true;
    options
}

/// fmt / normalize / linkcheck 用: 脚注定義を**ソース位置のまま**温存する。
///
/// comrak は既定でパース終端に「参照済み定義を文書末尾へ移動・未参照定義を削除」
/// する（process_footnotes）。fmt のバイト尊重方針と衝突するため、整形・正規化・
/// リンク検査は定義を動かさないこのオプションでパースする。
///
/// ⚠️ HTML レンダに使ってはならない: `<section class="footnotes">` ラッパが
/// 最初の定義位置で 1 回しか開かれず HTML が壊れる。
/// ⚠️ `extract_meta` にも使わない: 見出しのアンカー採番順が render とずれる
fn comrak_options_keep_footnotes(opts: &MarkdownOptions) -> Options<'static> {
    let mut options = comrak_options(opts);
    options.parse.leave_footnote_definitions = true;
    options
}

/// パス1 の結果
pub(crate) struct ExtractedMeta {
    pub frontmatter: Frontmatter,
    pub first_h1: Option<String>,
    pub toc: Vec<TocEntry>,
}

/// frontmatter・先頭 h1・TOC（h1〜h6 全見出し＋アンカー ID）を抽出する
pub(crate) fn extract_meta(
    source: &str,
    opts: &MarkdownOptions,
    src_path: &Path,
) -> Result<ExtractedMeta, CoreError> {
    let arena = Arena::new();
    let options = comrak_options(opts);
    let root = parse_document(&arena, source, &options);

    let mut frontmatter = Frontmatter::default();
    let mut first_h1 = None;
    let mut toc = Vec::new();
    // HTML 化時の header_ids 拡張と同じ採番になるよう、全見出しを文書順で anchorize
    let mut anchorizer = Anchorizer::new();

    for node in root.descendants() {
        let data = node.data.borrow();
        match &data.value {
            NodeValue::FrontMatter(raw) => {
                frontmatter = parse_frontmatter(raw).map_err(|message| CoreError::Frontmatter {
                    path: src_path.to_path_buf(),
                    message,
                })?;
            }
            NodeValue::Heading(heading) => {
                let text = collect_text(node);
                let id = anchorizer.anchorize(&text);
                if heading.level == 1 && first_h1.is_none() {
                    first_h1 = Some(text.clone());
                }
                toc.push(TocEntry {
                    level: heading.level,
                    id,
                    text,
                    span: span_of(&data.sourcepos),
                });
            }
            _ => {}
        }
    }

    Ok(ExtractedMeta {
        frontmatter,
        first_h1,
        toc,
    })
}

/// 本文を HTML 化する（コードブロック差し替え・URL 書き換えつき）
pub(crate) fn render_body_html(
    page: &Page,
    opts: &MarkdownOptions,
    code: &dyn CodeBlockRenderer,
    urls: &dyn UrlRewriter,
) -> Result<String, CoreError> {
    let arena = Arena::new();
    let options = comrak_options(opts);
    let root = parse_document(&arena, &page.source, &options);

    for node in root.descendants() {
        // コードブロック → フックが返した HTML（HtmlBlock）へ差し替え
        let replacement = {
            let data = node.data.borrow();
            if let NodeValue::CodeBlock(cb) = &data.value {
                let lang = cb.info.split_whitespace().next().filter(|s| !s.is_empty());
                code.render(lang, &cb.literal)
            } else {
                None
            }
        };
        if let Some(html) = replacement {
            node.data.borrow_mut().value = NodeValue::HtmlBlock(NodeHtmlBlock {
                block_type: 6,
                literal: html,
            });
            continue;
        }

        // リンク・画像の URL 書き換え
        let mut data = node.data.borrow_mut();
        if let NodeValue::Link(link) | NodeValue::Image(link) = &mut data.value {
            if let Some(rewritten) = urls.rewrite(page, &link.url) {
                link.url = rewritten;
            }
        }
    }

    let mut out = String::new();
    format_html(root, &options, &mut out)?;
    Ok(out)
}

/// 本文を正規化 Markdown として出力する（frontmatter は含めない）。
///
/// comrak の `format_commonmark` による正規化（見出し ATX 化・箇条書き `-` 統一・
/// 裸 URL の `<url>` 化等）。llms-full.txt と将来の `yuzu fmt`（Phase 6）の共通基盤。
///
/// ⚠️ `render_body_html` 後の AST（コードブロックが HtmlBlock 化済み）を
/// 流用しないこと。必ず新規パースした AST に対して行う
pub(crate) fn normalize_markdown(
    source: &str,
    opts: &MarkdownOptions,
) -> Result<String, CoreError> {
    let arena = Arena::new();
    // 脚注定義の位置・未参照定義を温存する（llms-full は原文に忠実な正規形を出す）
    let options = comrak_options_keep_footnotes(opts);
    let root = parse_document(&arena, source, &options);

    // format_commonmark は FrontMatter ノードを（区切り行込みの生テキストごと）
    // 再出力するため、AST から外す。FrontMatter は常に Document の第一子
    if let Some(first) = root.first_child() {
        if matches!(first.data.borrow().value, NodeValue::FrontMatter(_)) {
            first.detach();
        }
    }

    let mut out = String::new();
    format_commonmark(root, &options, &mut out)?;
    Ok(out)
}

/// 本文中のリンク・画像参照（linkcheck 用）
pub(crate) struct LinkRef {
    pub url: String,
    pub is_image: bool,
    pub span: SourceSpan,
}

/// 本文中のリンク・画像の URL を sourcepos 付きで列挙する（`yuzu check` 用）。
/// autolink（GFM）もリンクとして現れる
pub(crate) fn extract_link_refs(source: &str, opts: &MarkdownOptions) -> Vec<LinkRef> {
    let arena = Arena::new();
    // 既定オプションだと未参照の脚注定義が AST から消え、その中の壊れリンクが
    // 検査をすり抜ける。fmt が未参照定義を温存する以上、検査も同じ AST を見る
    let options = comrak_options_keep_footnotes(opts);
    let root = parse_document(&arena, source, &options);

    let mut refs = Vec::new();
    for node in root.descendants() {
        let data = node.data.borrow();
        let (url, is_image) = match &data.value {
            NodeValue::Link(link) => (link.url.clone(), false),
            NodeValue::Image(link) => (link.url.clone(), true),
            _ => continue,
        };
        refs.push(LinkRef {
            url,
            is_image,
            span: span_of(&data.sourcepos),
        });
    }
    refs
}

/// 本文のテキストノードを span 付きで列挙する（用語 lint 用）。
/// コードブロック・インラインコード・HTML・数式・リンク URL は Text ノードに
/// ならないため対象外になる（見出し・リンクラベル・強調中のテキストは含む）
pub(crate) fn extract_text_spans(
    source: &str,
    opts: &MarkdownOptions,
) -> Vec<(String, SourceSpan)> {
    let arena = Arena::new();
    let root = parse_document(&arena, source, &comrak_options(opts));
    let mut out = Vec::new();
    for node in root.descendants() {
        let data = node.data.borrow();
        if let NodeValue::Text(text) = &data.value {
            out.push((text.to_string(), span_of(&data.sourcepos)));
        }
    }
    out
}

/// frontmatter の生テキスト（`---` 区切り行込み）とソース上の位置を返す。
/// frontmatter がなければ None（lint の未知キー検出用）
pub(crate) fn frontmatter_raw(
    source: &str,
    opts: &MarkdownOptions,
) -> Option<(String, SourceSpan)> {
    let arena = Arena::new();
    let options = comrak_options(opts);
    let root = parse_document(&arena, source, &options);

    let first = root.first_child()?;
    let data = first.data.borrow();
    match &data.value {
        NodeValue::FrontMatter(raw) => Some((raw.clone(), span_of(&data.sourcepos))),
        _ => None,
    }
}

/// 全文を整形した Markdown を返す（`yuzu fmt` 用）。
///
/// 本文は [`normalize_markdown`] と同じ `format_commonmark` の正規形。
/// frontmatter は YAML を再シリアライズせず**生テキストをバイト温存**して
/// 再結合する（コメント・キー順・クォートを壊さない）。
/// 末尾改行は常に 1 個、frontmatter と本文の間は空行 1 つに正規化する
pub(crate) fn format_document(source: &str, opts: &MarkdownOptions) -> Result<String, CoreError> {
    let arena = Arena::new();
    // 脚注定義の位置・未参照定義を温存する（fmt は書き手の構成を動かさない）
    let options = comrak_options_keep_footnotes(opts);
    let root = parse_document(&arena, source, &options);

    // frontmatter の生テキスト（区切り行込み）を退避して detach
    // （format_commonmark が生テキストごと再出力してしまうため。normalize と同じ）
    let mut fm_raw: Option<String> = None;
    if let Some(first) = root.first_child() {
        if let NodeValue::FrontMatter(raw) = &first.data.borrow().value {
            fm_raw = Some(raw.clone());
        }
        if fm_raw.is_some() {
            first.detach();
        }
    }

    let mut body = String::new();
    format_commonmark(root, &options, &mut body)?;
    let body = body.trim_end();

    Ok(match (fm_raw, body.is_empty()) {
        (Some(raw), true) => format!("{}\n", raw.trim_end()),
        (Some(raw), false) => format!("{}\n\n{body}\n", raw.trim_end()),
        (None, true) => String::new(),
        (None, false) => format!("{body}\n"),
    })
}

/// 本文のプレーンテキストを抽出する（検索インデックス用）。
/// [`extract_plain_sections`] の結合として実装する（除外ルールの単一実装化）
pub(crate) fn extract_plain_text(
    source: &str,
    opts: &MarkdownOptions,
) -> Result<String, CoreError> {
    let mut out = String::new();
    for section in extract_plain_sections(source, opts)? {
        if let Some(heading) = &section.heading {
            out.push_str(heading);
            out.push('\n');
        }
        if !section.body.is_empty() {
            out.push_str(&section.body);
            out.push('\n');
        }
    }
    Ok(out.trim().to_string())
}

/// 本文を h2/h3 見出し境界で分割したプレーンテキストセクションを返す（検索インデックス用）。
///
/// - 先頭は常にリード文セクション（anchor/heading = None。本文が無くても返す）
/// - h4〜h6 と h1 は境界にせず、見出しテキストを現セクションの本文に含める
/// - 収集: `Text` / インライン `Code`（API 名検索のため含める）。
///   `SoftBreak` / `LineBreak` は空白、ブロック要素の末尾で改行 1 つ
/// - 除外: frontmatter・生 HTML・**フェンスコードブロック**
///   （mermaid ソースや長いコード片が BM25 を汚すため。
///   将来 `search.indexCode` のような opt-in を足す余地はある）
///
/// ⚠️ アンカー同期: [`extract_meta`]・HTML 化と同じく Anchorizer を
/// **全見出し（h1〜h6）文書順**で回す。境界にしない見出しも必ず anchorize する。
/// keep_footnotes 版オプションは使わない（採番が render とずれるため）
pub(crate) fn extract_plain_sections(
    source: &str,
    opts: &MarkdownOptions,
) -> Result<Vec<PlainSection>, CoreError> {
    let arena = Arena::new();
    let options = comrak_options(opts);
    let root = parse_document(&arena, source, &options);

    let mut anchorizer = Anchorizer::new();
    let mut sections = vec![PlainSection {
        anchor: None,
        heading: None,
        body: String::new(),
    }];
    collect_sections(root, &mut anchorizer, &mut sections);
    for section in &mut sections {
        section.body = section.body.trim().to_string();
    }
    Ok(sections)
}

fn collect_sections<'a>(
    node: &'a AstNode<'a>,
    anchorizer: &mut Anchorizer,
    sections: &mut Vec<PlainSection>,
) {
    {
        let data = node.data.borrow();
        match &data.value {
            NodeValue::FrontMatter(_)
            | NodeValue::HtmlBlock(_)
            | NodeValue::HtmlInline(_)
            | NodeValue::CodeBlock(_) => return,
            NodeValue::Heading(heading) => {
                let text = collect_text(node);
                let id = anchorizer.anchorize(&text);
                if heading.level == 2 || heading.level == 3 {
                    // 境界: 新しいセクションを開始（自見出しは body に含めず、
                    // builder が heading フィールドへ重み付きで別計上する）
                    sections.push(PlainSection {
                        anchor: Some(id),
                        heading: Some(text),
                        body: String::new(),
                    });
                } else {
                    // h1・h4〜h6 は境界にしない（テキストは検索対象として本文に残す）
                    let body = &mut sections.last_mut().expect("先頭セクションが常にある").body;
                    body.push_str(&text);
                    body.push('\n');
                }
                return; // 見出し配下は collect_text で回収済み
            }
            NodeValue::Text(literal) => sections.last_mut().unwrap().body.push_str(literal),
            NodeValue::Code(code) => sections.last_mut().unwrap().body.push_str(&code.literal),
            NodeValue::LineBreak | NodeValue::SoftBreak => {
                sections.last_mut().unwrap().body.push(' ')
            }
            _ => {}
        }
    }

    for child in node.children() {
        collect_sections(child, anchorizer, sections);
    }

    // 段落・リスト項目等の区切りで改行を入れる（トークナイズの文脈を切る）
    if node.data.borrow().value.block() {
        let body = &mut sections.last_mut().unwrap().body;
        if !body.is_empty() && !body.ends_with('\n') {
            body.push('\n');
        }
    }
}

/// 見出しノード配下のプレーンテキストを収集する。
/// comrak の header_ids 拡張と同じ規則（Text/Code はリテラル、改行は空白）に合わせる
fn collect_text<'a>(node: &'a AstNode<'a>) -> String {
    let mut out = String::new();
    collect_text_into(node, &mut out);
    out
}

fn collect_text_into<'a>(node: &'a AstNode<'a>, out: &mut String) {
    match &node.data.borrow().value {
        NodeValue::Text(literal) => out.push_str(literal),
        NodeValue::Code(code) => out.push_str(&code.literal),
        // comrak の header_ids（Anchorizer）は見出し内数式の literal を採番に含める。
        // ここで落とすと TOC・リンク検査のアンカーが本文とずれる
        NodeValue::Math(math) => out.push_str(&math.literal),
        NodeValue::LineBreak | NodeValue::SoftBreak => out.push(' '),
        _ => {
            for child in node.children() {
                collect_text_into(child, out);
            }
        }
    }
}

fn span_of(sourcepos: &comrak::nodes::Sourcepos) -> SourceSpan {
    SourceSpan {
        start_line: sourcepos.start.line,
        start_col: sourcepos.start.column,
        end_line: sourcepos.end.line,
        end_col: sourcepos.end.column,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// インラインノード（リンク）の sourcepos が行・列とも正確なことの実測
    /// （linkcheck の診断位置の前提。ずれるようなら表示を行番号のみに落とす）
    #[test]
    fn リンクの_sourcepos_は行と列を正しく指す() {
        let source = "# 見出し\n\n本文 [リンク](target.md) と ![画像](img.png)。\n\n- 項目の [中のリンク](other.md#frag)\n";
        let refs = extract_link_refs(source, &MarkdownOptions::default());
        assert_eq!(refs.len(), 3);

        assert_eq!(refs[0].url, "target.md");
        assert!(!refs[0].is_image);
        assert_eq!(refs[0].span.start_line, 3);
        // 「本文 」= 本文(6 バイト) + 空白(1) の次 = 8 バイト目（col は 1 始まりバイト位置）
        assert_eq!(refs[0].span.start_col, 8);

        assert_eq!(refs[1].url, "img.png");
        assert!(refs[1].is_image);
        assert_eq!(refs[1].span.start_line, 3);

        assert_eq!(refs[2].url, "other.md#frag");
        assert_eq!(refs[2].span.start_line, 5);
    }
}
