//! minijinja テンプレートへ渡すコンテキスト型

use serde::Serialize;

use yuzu_core::{NavNode, Page};

use crate::urls::UrlResolver;

/// TOC に表示する見出しレベル（h2〜h3）
const TOC_LEVELS: std::ops::RangeInclusive<u8> = 2..=3;

#[derive(Serialize)]
pub(crate) struct SiteCtx<'a> {
    pub title: &'a str,
    pub description: Option<&'a str>,
    pub lang: &'a str,
    /// ヘッダーロゴの配信 URL（`site.logo` 由来。base 前置済み。None ならテーマ既定ロゴ）
    pub logo_url: Option<String>,
}

#[derive(Serialize)]
pub(crate) struct TocCtx<'a> {
    pub level: u8,
    pub id: &'a str,
    pub text: &'a str,
}

#[derive(Serialize)]
pub(crate) struct PageCtx<'a> {
    pub title: &'a str,
    pub description: Option<&'a str>,
    /// 本文 HTML（テンプレート側で `| safe` を通す）
    pub body: &'a str,
    /// 配信 URL（base 付き）
    pub url: String,
    pub toc: Vec<TocCtx<'a>>,
}

impl<'a> PageCtx<'a> {
    pub fn new(page: &'a Page, body: &'a str, resolver: &UrlResolver) -> Self {
        Self {
            title: &page.title,
            description: page.frontmatter.description.as_deref(),
            body,
            url: resolver.page_url(&page.route),
            toc: page
                .toc
                .iter()
                .filter(|t| TOC_LEVELS.contains(&t.level))
                .map(|t| TocCtx {
                    level: t.level,
                    id: &t.id,
                    text: &t.text,
                })
                .collect(),
        }
    }
}

#[derive(Serialize)]
pub(crate) struct NavCtx {
    pub title: String,
    pub url: Option<String>,
    /// 表示中のページかどうか（サイドバーのハイライト用）
    pub active: bool,
    pub children: Vec<NavCtx>,
}

impl NavCtx {
    /// ナビツリーを URL 解決しつつ、現在ページに active を立てる
    pub fn build(nav: &[NavNode], current_route: &str, resolver: &UrlResolver) -> Vec<NavCtx> {
        nav.iter()
            .map(|node| NavCtx {
                title: node.title.clone(),
                url: node.route.as_deref().map(|r| resolver.page_url(r)),
                active: node.route.as_deref() == Some(current_route),
                children: Self::build(&node.children, current_route, resolver),
            })
            .collect()
    }
}
