//! minijinja テンプレートへ渡すコンテキスト型

use std::collections::HashMap;

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
    /// ページ単位 Markdown の配信 URL（コピーボタンの fetch 先）
    pub md_url: String,
    /// draft ページか（`--drafts` プレビュー時のバナー表示用。通常ビルドでは常に false）
    pub draft: bool,
    pub toc: Vec<TocCtx<'a>>,
}

impl<'a> PageCtx<'a> {
    pub fn new(page: &'a Page, body: &'a str, resolver: &UrlResolver) -> Self {
        Self {
            title: &page.title,
            description: page.frontmatter.description.as_deref(),
            body,
            url: resolver.page_url(&page.route),
            md_url: resolver.md_url(&page.route),
            draft: page.frontmatter.draft,
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

#[derive(Serialize)]
pub(crate) struct PagerLinkCtx<'a> {
    pub title: &'a str,
    /// 配信 URL（base 付き）
    pub url: String,
}

#[derive(Serialize)]
pub(crate) struct PagerCtx<'a> {
    pub prev: Option<PagerLinkCtx<'a>>,
    pub next: Option<PagerLinkCtx<'a>>,
}

/// nav 順の深さ優先走査でページを 1 列に並べたもの（前/次リンクの導出元）。
/// ノード自身 → children の順。route を持たないラベルノードは飛ばして子へ降りる。
///
/// 注意: llms.txt はトップレベルの葉ページを先頭セクションへ前寄せするため
/// （llms.rs の sections()）、葉がディレクトリより後ろに並ぶ構成では順序が
/// 一致しない。前/次は「サイドバー表示順」を正とする（設計判断）
pub(crate) struct NavOrder<'a> {
    /// (title, route)。route は nav 構築時点で一意（nav.rs が index 重複を除去済み）
    entries: Vec<(&'a str, &'a str)>,
    /// route → entries の位置
    index: HashMap<&'a str, usize>,
}

impl<'a> NavOrder<'a> {
    pub fn new(nav: &'a [NavNode]) -> Self {
        fn collect<'a>(nodes: &'a [NavNode], out: &mut Vec<(&'a str, &'a str)>) {
            for node in nodes {
                if let Some(route) = node.route.as_deref() {
                    out.push((&node.title, route));
                }
                collect(&node.children, out);
            }
        }
        let mut entries = Vec::new();
        collect(nav, &mut entries);
        let index = entries
            .iter()
            .enumerate()
            .map(|(i, (_, route))| (*route, i))
            .collect();
        Self { entries, index }
    }

    /// 現在ページの前後リンクを引く。route が見つからない場合は両側 None
    /// （draft はサイトモデルから除外済みなので実際には起きない防御）
    pub fn pager(&self, current_route: &str, resolver: &UrlResolver) -> PagerCtx<'a> {
        let Some(&i) = self.index.get(current_route) else {
            return PagerCtx {
                prev: None,
                next: None,
            };
        };
        let link = |j: usize| {
            let (title, route) = self.entries[j];
            PagerLinkCtx {
                title,
                url: resolver.page_url(route),
            }
        };
        PagerCtx {
            prev: i.checked_sub(1).map(link),
            next: (i + 1 < self.entries.len()).then(|| link(i + 1)),
        }
    }
}

#[derive(Serialize)]
pub(crate) struct BreadcrumbCtx<'a> {
    pub title: &'a str,
    /// None = リンクなし（index.md のないディレクトリ、および末尾の現在ページ）
    pub url: Option<String>,
}

/// nav ツリーから現在ページへの祖先チェーンを探す（深さ優先。見つけたら true）
fn find_path<'a>(nodes: &'a [NavNode], route: &str, path: &mut Vec<&'a NavNode>) -> bool {
    for node in nodes {
        path.push(node);
        if node.route.as_deref() == Some(route) {
            return true;
        }
        if find_path(&node.children, route, path) {
            return true;
        }
        path.pop();
    }
    false
}

/// 階層パンくず「ホーム > セクション > 現在ページ」を組み立てる。
/// ルート index.md（route ""）は nav 上トップレベルの葉で祖先にならないため
/// 手動で前置する。遡る先のないページ（ホーム自身・階層なし）は空 = 非表示
pub(crate) fn build_breadcrumbs<'a>(
    nav: &'a [NavNode],
    current_route: &str,
    resolver: &UrlResolver,
) -> Vec<BreadcrumbCtx<'a>> {
    if current_route.is_empty() {
        return Vec::new(); // ホーム自身には出さない
    }
    let mut path = Vec::new();
    if !find_path(nav, current_route, &mut path) {
        return Vec::new();
    }

    let mut items = Vec::new();
    if let Some(home) = nav.iter().find(|n| n.route.as_deref() == Some("")) {
        items.push(BreadcrumbCtx {
            title: &home.title,
            url: Some(resolver.page_url("")),
        });
    }
    let last = path.len() - 1;
    for (i, node) in path.iter().enumerate() {
        items.push(BreadcrumbCtx {
            title: &node.title,
            // 末尾（現在ページ）は常にリンクなし
            url: (i != last)
                .then(|| node.route.as_deref().map(|r| resolver.page_url(r)))
                .flatten(),
        });
    }
    if items.len() <= 1 {
        return Vec::new(); // 遡る先がない（ホーム無しプロジェクトの最上位ページ等）
    }
    items
}

#[cfg(test)]
mod tests {
    use super::*;
    use yuzu_core::SiteModel;

    fn node(title: &str, route: Option<&str>, children: Vec<NavNode>) -> NavNode {
        NavNode {
            title: title.to_string(),
            route: route.map(String::from),
            order: None,
            children,
        }
    }

    fn resolver() -> UrlResolver {
        UrlResolver::new(
            "/docs/",
            &SiteModel {
                pages: vec![],
                nav: vec![],
            },
        )
    }

    /// fixture 相当: ホーム（葉）＋ guide（index なしラベル）＋ manual（index あり）
    fn sample_nav() -> Vec<NavNode> {
        vec![
            node("ホーム", Some(""), vec![]),
            node(
                "guide",
                None,
                vec![
                    node("はじめに", Some("guide/getting-started/"), vec![]),
                    node("応用", Some("guide/advanced/"), vec![]),
                ],
            ),
            node(
                "マニュアル",
                Some("manual/"),
                vec![node("設定", Some("manual/config/"), vec![])],
            ),
        ]
    }

    #[test]
    fn フラット化は_nav_順の深さ優先でラベルノードを飛ばす() {
        let nav = sample_nav();
        let order = NavOrder::new(&nav);
        let routes: Vec<&str> = order.entries.iter().map(|(_, r)| *r).collect();
        assert_eq!(
            routes,
            [
                "",
                "guide/getting-started/",
                "guide/advanced/",
                "manual/",
                "manual/config/"
            ]
        );
    }

    #[test]
    fn pager_は前後ページを返し先頭末尾は片側_none() {
        let nav = sample_nav();
        let order = NavOrder::new(&nav);
        let r = resolver();

        let mid = order.pager("guide/advanced/", &r);
        assert_eq!(mid.prev.as_ref().unwrap().title, "はじめに");
        assert_eq!(
            mid.prev.as_ref().unwrap().url,
            "/docs/guide/getting-started/"
        );
        assert_eq!(mid.next.as_ref().unwrap().title, "マニュアル");
        assert_eq!(mid.next.as_ref().unwrap().url, "/docs/manual/");

        let first = order.pager("", &r);
        assert!(first.prev.is_none());
        assert_eq!(first.next.as_ref().unwrap().title, "はじめに");

        let last = order.pager("manual/config/", &r);
        assert!(last.next.is_none());
        assert_eq!(last.prev.as_ref().unwrap().title, "マニュアル");

        let unknown = order.pager("nowhere/", &r);
        assert!(unknown.prev.is_none() && unknown.next.is_none());
    }

    #[test]
    fn パンくずはホームを前置し中間ラベルは_url_なし() {
        let nav = sample_nav();
        let items = build_breadcrumbs(&nav, "guide/getting-started/", &resolver());
        let view: Vec<(&str, Option<&str>)> =
            items.iter().map(|b| (b.title, b.url.as_deref())).collect();
        assert_eq!(
            view,
            [
                ("ホーム", Some("/docs/")),
                ("guide", None),    // index.md なし → ラベル
                ("はじめに", None), // 現在ページ → リンクなし
            ]
        );

        // index.md ありディレクトリは中間でリンクになる
        let items = build_breadcrumbs(&nav, "manual/config/", &resolver());
        let view: Vec<(&str, Option<&str>)> =
            items.iter().map(|b| (b.title, b.url.as_deref())).collect();
        assert_eq!(
            view,
            [
                ("ホーム", Some("/docs/")),
                ("マニュアル", Some("/docs/manual/")),
                ("設定", None),
            ]
        );
    }

    #[test]
    fn ホーム自身と遡る先のないページはパンくずが空() {
        let nav = sample_nav();
        assert!(build_breadcrumbs(&nav, "", &resolver()).is_empty());

        // ホームが nav に無く、トップレベル葉ページ単独 → 遡る先なし
        let nav = vec![node("単独", Some("alone/"), vec![])];
        assert!(build_breadcrumbs(&nav, "alone/", &resolver()).is_empty());
    }

    #[test]
    fn ホームが_nav_に無ければ前置しない() {
        let nav = vec![node(
            "guide",
            None,
            vec![node("はじめに", Some("guide/getting-started/"), vec![])],
        )];
        let items = build_breadcrumbs(&nav, "guide/getting-started/", &resolver());
        let view: Vec<(&str, Option<&str>)> =
            items.iter().map(|b| (b.title, b.url.as_deref())).collect();
        assert_eq!(view, [("guide", None), ("はじめに", None)]);
    }
}
