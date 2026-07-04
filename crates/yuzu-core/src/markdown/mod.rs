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
use comrak::{Anchorizer, Arena, Options, format_html, parse_document};

use crate::MarkdownOptions;
use crate::error::CoreError;
use crate::frontmatter::parse_frontmatter;
use crate::model::{Frontmatter, Page, SourceSpan, TocEntry};
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
    }
    options.extension.front_matter_delimiter = Some("---".to_string());
    options.extension.header_id_prefix = Some(String::new());
    options.render.r#unsafe = true;
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

/// 本文のプレーンテキストを抽出する（検索インデックス用）。
///
/// - 収集: `Text` / インライン `Code`（API 名検索のため含める）。
///   `SoftBreak` / `LineBreak` は空白、ブロック要素の末尾で改行 1 つ
/// - 除外: frontmatter・生 HTML・**フェンスコードブロック**
///   （mermaid ソースや長いコード片が BM25 を汚すため。
///   将来 `search.indexCode` のような opt-in を足す余地はある）
pub(crate) fn extract_plain_text(
    source: &str,
    opts: &MarkdownOptions,
) -> Result<String, CoreError> {
    let arena = Arena::new();
    let options = comrak_options(opts);
    let root = parse_document(&arena, source, &options);

    let mut out = String::new();
    collect_plain_text(root, &mut out);
    Ok(out.trim().to_string())
}

fn collect_plain_text<'a>(node: &'a AstNode<'a>, out: &mut String) {
    {
        let data = node.data.borrow();
        match &data.value {
            NodeValue::FrontMatter(_)
            | NodeValue::HtmlBlock(_)
            | NodeValue::HtmlInline(_)
            | NodeValue::CodeBlock(_) => return,
            NodeValue::Text(literal) => out.push_str(literal),
            NodeValue::Code(code) => out.push_str(&code.literal),
            NodeValue::LineBreak | NodeValue::SoftBreak => out.push(' '),
            _ => {}
        }
    }

    for child in node.children() {
        collect_plain_text(child, out);
    }

    // 段落・見出し・リスト項目等の区切りで改行を入れる（トークナイズの文脈を切る）
    if node.data.borrow().value.block() && !out.is_empty() && !out.ends_with('\n') {
        out.push('\n');
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
