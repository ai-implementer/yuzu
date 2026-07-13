//! llms.txt / llms-full.txt の生成（<https://llmstxt.org/> 仕様）。
//!
//! - `llms.txt` — リンク索引: H1（サイト名）＋ blockquote（要約）＋
//!   nav のトップレベル構造ごとの H2 セクション＋リンクリスト
//! - `llms-full.txt` — 全ページの正規化 Markdown を nav 順で連結（仕様外の事実上の慣行）
//!
//! frontmatter `llms: false` のページは両ファイルから除外する。

use std::collections::{HashMap, HashSet};
use std::path::Path;

use yuzu_config::ResolvedConfig;
use yuzu_core::{MarkdownOptions, NavNode, Page, SiteModel};

use crate::assets;
use crate::error::RenderError;
use crate::urls::UrlResolver;

/// トップレベルの葉ページ（ルート `index.md` 等）をまとめる先頭セクション名
const ROOT_SECTION_TITLE: &str = "Docs";

/// llms.txt 本文を生成する
pub fn generate_llms_txt(rc: &ResolvedConfig, site: &SiteModel) -> Result<String, RenderError> {
    let resolver = UrlResolver::new(&rc.base_url, site);

    let mut out = String::new();
    out.push_str(&format!("# {}\n", sanitize_line(&rc.config.site.title)));
    if let Some(desc) = &rc.config.site.description {
        out.push_str(&format!("\n> {}\n", sanitize_line(desc)));
    }

    for (title, pages) in sections(site) {
        out.push_str(&format!("\n## {}\n\n", sanitize_line(&title)));
        for page in pages {
            // リンク先はページ単位 Markdown（LLM が直接読める形式。Phase 14）
            let url = resolver.md_url(&page.route);
            let title = sanitize_line(&page.title);
            match page.frontmatter.description.as_deref() {
                Some(desc) => {
                    out.push_str(&format!("- [{title}]({url}): {}\n", sanitize_line(desc)));
                }
                None => out.push_str(&format!("- [{title}]({url})\n")),
            }
        }
    }
    Ok(out)
}

/// llms-full.txt 本文を生成する（nav 順で正規化 Markdown を連結）。
/// cache があれば未変更ページの正規化（comrak パース）をスキップする
pub fn generate_llms_full_txt(
    rc: &ResolvedConfig,
    site: &SiteModel,
    cache: Option<&yuzu_core::BuildCache>,
) -> Result<String, RenderError> {
    let resolver = UrlResolver::new(&rc.base_url, site);
    let md_opts = MarkdownOptions {
        gfm: rc.config.markdown.gfm,
        math: rc.config.markdown.math.enabled,
    };

    let mut out = String::new();
    out.push_str(&format!("# {}\n", sanitize_line(&rc.config.site.title)));
    if let Some(desc) = &rc.config.site.description {
        out.push_str(&format!("\n> {}\n", sanitize_line(desc)));
    }

    for (_, pages) in sections(site) {
        for page in pages {
            // ページ本文は自身の H1 を含むため、区切りは URL 行のみ
            out.push_str("\n---\n\n");
            out.push_str(&format!("URL: {}\n\n", resolver.page_url(&page.route)));
            let normalized = match cache.and_then(|c| c.llms(&page.rel, &page.source)) {
                Some(cached) => cached,
                None => {
                    let normalized = yuzu_core::normalize_markdown(page, &md_opts)?;
                    if let Some(c) = cache {
                        c.store_llms(&page.rel, &page.source, normalized.clone());
                    }
                    normalized
                }
            };
            out.push_str(normalized.trim_end());
            out.push('\n');
        }
    }
    Ok(out)
}

/// dist/ 直下へ書き出す（`llms.full=false` なら llms-full.txt は書かない）
pub(crate) fn write_llms_files(
    rc: &ResolvedConfig,
    site: &SiteModel,
    output_dir: &Path,
    ctx: &crate::pipeline::RenderCtx,
) -> Result<(), RenderError> {
    let llms_txt = generate_llms_txt(rc, site)?;
    assets::write_output(ctx.outputs, output_dir, "llms.txt", llms_txt.as_bytes())?;

    if rc.config.llms.full {
        let full = generate_llms_full_txt(rc, site, ctx.cache)?;
        assets::write_output(ctx.outputs, output_dir, "llms-full.txt", full.as_bytes())?;
    }
    Ok(())
}

/// nav 構造 → セクション列（見出し, ページ列）。両ファイルで同じ順序を使う。
///
/// - トップレベルの葉ページ → 先頭の [`ROOT_SECTION_TITLE`] セクション
/// - children を持つトップレベルノード → H2 セクション（子孫は平坦化）
/// - `llms: false` は除外。リンク 0 件のセクションは出さない
/// - nav に現れないページは防御的に先頭セクションへ追補（現状は発生しない）
fn sections(site: &SiteModel) -> Vec<(String, Vec<&Page>)> {
    let by_route: HashMap<&str, &Page> = site.pages.iter().map(|p| (p.route.as_str(), p)).collect();
    let mut visited: HashSet<&str> = HashSet::new();

    let mut root_pages: Vec<&Page> = Vec::new();
    let mut dir_sections: Vec<(String, Vec<&Page>)> = Vec::new();

    for node in &site.nav {
        if node.children.is_empty() {
            collect_pages(node, &by_route, &mut visited, &mut root_pages);
        } else {
            let mut pages = Vec::new();
            collect_pages(node, &by_route, &mut visited, &mut pages);
            if !pages.is_empty() {
                dir_sections.push((node.title.clone(), pages));
            }
        }
    }

    // 防御: nav に現れなかったページ（現状の nav 生成では起きない）
    for page in &site.pages {
        if !visited.contains(page.route.as_str()) && page.frontmatter.llms {
            root_pages.push(page);
        }
    }

    let mut sections = dir_sections;
    if !root_pages.is_empty() {
        sections.insert(0, (ROOT_SECTION_TITLE.to_string(), root_pages));
    }
    sections
}

/// ノード自身 → children の深さ優先でページを収集する（`llms: false` は除外）
fn collect_pages<'a>(
    node: &NavNode,
    by_route: &HashMap<&str, &'a Page>,
    visited: &mut HashSet<&'a str>,
    out: &mut Vec<&'a Page>,
) {
    if let Some(route) = node.route.as_deref() {
        if let Some(&page) = by_route.get(route) {
            if visited.insert(page.route.as_str()) && page.frontmatter.llms {
                out.push(page);
            }
        }
    }
    for child in &node.children {
        collect_pages(child, by_route, visited, out);
    }
}

/// 見出し・リンク行に改行が混ざると llms.txt のリスト形式が壊れるため空白へ潰す
fn sanitize_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}
