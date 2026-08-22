//! minijinja テンプレートへ渡すコンテキスト型

use std::collections::HashMap;

use serde::Serialize;

use yuzu_core::{NavNode, Page, TocEntry};

use crate::urls::UrlResolver;

/// TOC に表示する既定の見出しレベル（h2〜h3）。`theme.toc.levels` で変更できる
pub(crate) const TOC_LEVELS: std::ops::RangeInclusive<u8> = 2..=3;

/// `theme.toc.levels`（`"2-3"` / `"4"`。インクルードの `lines=` と同じ範囲記法）を
/// 解析する。1..=6 の外は clamp、逆順・非数値は None（呼び出し側が警告して既定へ縮退）
pub(crate) fn parse_toc_levels(raw: &str) -> Option<std::ops::RangeInclusive<u8>> {
    let raw = raw.trim();
    let (lo, hi) = match raw.split_once('-') {
        Some((a, b)) => (a.trim().parse::<u8>().ok()?, b.trim().parse::<u8>().ok()?),
        None => {
            let n = raw.parse::<u8>().ok()?;
            (n, n)
        }
    };
    let (lo, hi) = (lo.clamp(1, 6), hi.clamp(1, 6));
    (lo <= hi).then_some(lo..=hi)
}

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
    /// より深いレベルの直後の見出し群（テンプレートが入れ子 `<ul>` に描く）
    pub children: Vec<TocCtx<'a>>,
}

/// フラットな見出し列（文書順・表示レベルで絞り込み済み）を入れ子ツリーへ積む。
/// 「次に自分以下のレベルが現れるまで」が自分のサブツリー。レベル飛び
/// （h2 直下の h4）は直近の浅い見出しの子になる = 構造上 1 段だけ降りる。
/// 先頭に深いレベルが来る場合（h2 より先に h3）はそのままトップレベルに置く
fn build_toc<'a>(entries: &[&'a TocEntry]) -> Vec<TocCtx<'a>> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < entries.len() {
        let level = entries[i].level;
        let mut j = i + 1;
        while j < entries.len() && entries[j].level > level {
            j += 1;
        }
        out.push(TocCtx {
            level,
            id: &entries[i].id,
            text: &entries[i].text,
            children: build_toc(&entries[i + 1..j]),
        });
        i = j;
    }
    out
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
    /// 最終コミット日（YYYY-MM-DD。git.last_updated 有効かつ追跡済みのときのみ）
    pub last_updated: Option<String>,
    /// 「このページを編集」リンク（git.edit_url の {path} 置換済み）
    pub edit_url: Option<String>,
    pub toc: Vec<TocCtx<'a>>,
}

impl<'a> PageCtx<'a> {
    pub fn new(
        page: &'a Page,
        body: &'a str,
        resolver: &UrlResolver,
        last_updated: Option<String>,
        edit_url: Option<String>,
        toc_levels: &std::ops::RangeInclusive<u8>,
    ) -> Self {
        let visible: Vec<&TocEntry> = page
            .toc
            .iter()
            .filter(|t| toc_levels.contains(&t.level))
            .collect();
        Self {
            title: &page.title,
            description: page.frontmatter.description.as_deref(),
            body,
            url: resolver.page_url(&page.route),
            md_url: resolver.md_url(&page.route),
            draft: page.frontmatter.draft,
            last_updated,
            edit_url,
            toc: build_toc(&visible),
        }
    }
}

/// route → nav 上の祖先チェーン（先頭 = トップレベル、末尾 = 当該ノード自身。
/// route を持たないラベルノードも祖先として含む）。
///
/// ページ並列ループの**外で 1 回だけ**全ツリーを DFS し、各ページは O(1) の
/// 参照で済ませる（従来はページごとに find_path の DFS とホームの線形探索を
/// 回していた = 規模が出ると効く）。ノードの同定は `ptr::eq` — ラベルノードは
/// route で特定できないため
pub(crate) struct NavTrails<'a> {
    trails: HashMap<&'a str, Vec<&'a NavNode>>,
    /// route "" のホームノード（パンくず前置用）
    home: Option<&'a NavNode>,
}

impl<'a> NavTrails<'a> {
    pub fn new(nav: &'a [NavNode]) -> Self {
        fn walk<'a>(
            nodes: &'a [NavNode],
            path: &mut Vec<&'a NavNode>,
            trails: &mut HashMap<&'a str, Vec<&'a NavNode>>,
        ) {
            for node in nodes {
                path.push(node);
                if let Some(route) = node.route.as_deref() {
                    trails.insert(route, path.clone());
                }
                walk(&node.children, path, trails);
                path.pop();
            }
        }
        let mut trails = HashMap::new();
        walk(nav, &mut Vec::new(), &mut trails);
        let home = trails
            .get("")
            .map(|t| *t.last().expect("チェーンは自分自身を必ず含む"));
        Self { trails, home }
    }

    /// 現在ページの祖先チェーン。nav に無い route（"404.html" 等）は空
    pub fn trail(&self, route: &str) -> &[&'a NavNode] {
        self.trails.get(route).map_or(&[], |v| v.as_slice())
    }
}

#[derive(Serialize)]
pub(crate) struct NavCtx<'a> {
    pub title: &'a str,
    pub url: Option<String>,
    /// 表示中のページ自身か（**完全一致のみ**。サイドバーのハイライトと
    /// `aria-current`。祖先には付かない — theme.css / base.jinja の
    /// `li.active > a` 系セレクタがこの意味に依存している）
    pub active: bool,
    /// 現在ページの祖先チェーン上か（自分自身も含む）。
    /// テンプレートが `<details open>` へそのまま写す
    pub open: bool,
    pub children: Vec<NavCtx<'a>>,
}

impl<'a> NavCtx<'a> {
    /// ナビツリーを URL 解決しつつ、trail（[`NavTrails::trail`]）から
    /// active / open を立てる。trail が空（404 等）なら全ノード false = 全閉じ
    pub fn build(
        nav: &'a [NavNode],
        trail: &[&NavNode],
        resolver: &UrlResolver,
    ) -> Vec<NavCtx<'a>> {
        nav.iter()
            .map(|node| NavCtx {
                title: &node.title,
                url: node.route.as_deref().map(|r| resolver.page_url(r)),
                active: trail.last().is_some_and(|last| std::ptr::eq(*last, node)),
                open: trail.iter().any(|n| std::ptr::eq(*n, node)),
                children: Self::build(&node.children, trail, resolver),
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

/// 階層パンくず「ホーム > セクション > 現在ページ」を組み立てる。
/// ルート index.md（route ""）は nav 上トップレベルの葉で祖先にならないため
/// 手動で前置する。遡る先のないページ（ホーム自身・階層なし）は空 = 非表示。
/// 祖先チェーンは [`NavTrails`]（ループ外の前計算）から引く
pub(crate) fn build_breadcrumbs<'a>(
    trails: &NavTrails<'a>,
    current_route: &str,
    resolver: &UrlResolver,
) -> Vec<BreadcrumbCtx<'a>> {
    if current_route.is_empty() {
        return Vec::new(); // ホーム自身には出さない
    }
    let path = trails.trail(current_route);
    if path.is_empty() {
        return Vec::new();
    }

    let mut items = Vec::new();
    if let Some(home) = trails.home {
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
    fn navtrails_は祖先チェーンとホームを前計算する() {
        let nav = sample_nav();
        let trails = NavTrails::new(&nav);

        // ラベルノード（guide）も祖先として入る
        let trail = trails.trail("guide/getting-started/");
        let titles: Vec<&str> = trail.iter().map(|n| n.title.as_str()).collect();
        assert_eq!(titles, ["guide", "はじめに"]);
        // 末尾は自分自身
        assert_eq!(
            trail.last().unwrap().route.as_deref(),
            Some("guide/getting-started/")
        );

        // ディレクトリ自身（index.md 持ち）のチェーンは自分だけ
        let titles: Vec<&str> = trails
            .trail("manual/")
            .iter()
            .map(|n| n.title.as_str())
            .collect();
        assert_eq!(titles, ["マニュアル"]);

        // 未知 route（404 等）は空・ホームは検出される
        assert!(trails.trail("404.html").is_empty());
        assert_eq!(trails.home.unwrap().title, "ホーム");
    }

    #[test]
    fn navctx_の_active_は完全一致のみで祖先には_open_が立つ() {
        let nav = sample_nav();
        let trails = NavTrails::new(&nav);
        let ctx = NavCtx::build(&nav, trails.trail("manual/config/"), &resolver());

        // ホーム: active も open も付かない
        assert!(!ctx[0].active && !ctx[0].open);
        // ラベルセクション guide: チェーン外なので閉じる
        assert!(!ctx[1].active && !ctx[1].open);
        // マニュアル（祖先）: open だけ立つ（active の意味は完全一致のまま）
        assert!(!ctx[2].active && ctx[2].open);
        // 現在ページ: 両方立つ
        assert!(ctx[2].children[0].active && ctx[2].children[0].open);

        // ラベルノードも祖先なら open（guide 配下のページ）
        let ctx = NavCtx::build(&nav, trails.trail("guide/advanced/"), &resolver());
        assert!(
            !ctx[1].active && ctx[1].open,
            "ラベルノードに open が立たない"
        );

        // 空 trail（404 相当）は全ノード false = 全閉じ
        let ctx = NavCtx::build(&nav, &[], &resolver());
        fn all_closed(nodes: &[NavCtx]) -> bool {
            nodes
                .iter()
                .all(|n| !n.active && !n.open && all_closed(&n.children))
        }
        assert!(all_closed(&ctx));
    }

    fn toc_entry(level: u8, id: &str) -> yuzu_core::TocEntry {
        yuzu_core::TocEntry {
            level,
            id: id.to_string(),
            text: id.to_string(),
            span: yuzu_core::SourceSpan {
                start_line: 1,
                start_col: 1,
                end_line: 1,
                end_col: 1,
            },
        }
    }

    #[test]
    fn build_toc_はレベルで入れ子を積む() {
        // h2 > h3 の素直な入れ子
        let entries = [
            toc_entry(2, "a"),
            toc_entry(3, "a1"),
            toc_entry(3, "a2"),
            toc_entry(2, "b"),
        ];
        let refs: Vec<&yuzu_core::TocEntry> = entries.iter().collect();
        let toc = build_toc(&refs);
        assert_eq!(toc.len(), 2);
        assert_eq!(toc[0].id, "a");
        assert_eq!(
            toc[0].children.iter().map(|t| t.id).collect::<Vec<_>>(),
            ["a1", "a2"]
        );
        assert!(toc[1].children.is_empty());

        // レベル飛び（h2 直下の h4）は 1 段だけ降りて h2 の子になる。
        // 後続の h3 も同じ h2 の子（h4 の子にはならない）
        let entries = [toc_entry(2, "a"), toc_entry(4, "deep"), toc_entry(3, "a1")];
        let refs: Vec<&yuzu_core::TocEntry> = entries.iter().collect();
        let toc = build_toc(&refs);
        assert_eq!(
            toc[0].children.iter().map(|t| t.id).collect::<Vec<_>>(),
            ["deep", "a1"]
        );

        // 先頭に深いレベル（h2 より先の h3）はトップレベルに置く
        let entries = [toc_entry(3, "lead"), toc_entry(2, "a")];
        let refs: Vec<&yuzu_core::TocEntry> = entries.iter().collect();
        let toc = build_toc(&refs);
        assert_eq!(toc.iter().map(|t| t.id).collect::<Vec<_>>(), ["lead", "a"]);
    }

    #[test]
    fn parse_toc_levels_は範囲記法を受け付け不正は_none() {
        assert_eq!(parse_toc_levels("2-3"), Some(2..=3));
        assert_eq!(parse_toc_levels("4"), Some(4..=4));
        assert_eq!(parse_toc_levels(" 2 - 4 "), Some(2..=4));
        // 1..=6 の外は clamp
        assert_eq!(parse_toc_levels("0-9"), Some(1..=6));
        // 逆順・非数値・空は None（呼び出し側が既定へ縮退）
        assert_eq!(parse_toc_levels("3-2"), None);
        assert_eq!(parse_toc_levels("abc"), None);
        assert_eq!(parse_toc_levels(""), None);
    }

    #[test]
    fn パンくずはホームを前置し中間ラベルは_url_なし() {
        let nav = sample_nav();
        let items = build_breadcrumbs(&NavTrails::new(&nav), "guide/getting-started/", &resolver());
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
        let items = build_breadcrumbs(&NavTrails::new(&nav), "manual/config/", &resolver());
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
        assert!(build_breadcrumbs(&NavTrails::new(&nav), "", &resolver()).is_empty());

        // ホームが nav に無く、トップレベル葉ページ単独 → 遡る先なし
        let nav = vec![node("単独", Some("alone/"), vec![])];
        assert!(build_breadcrumbs(&NavTrails::new(&nav), "alone/", &resolver()).is_empty());
    }

    #[test]
    fn ホームが_nav_に無ければ前置しない() {
        let nav = vec![node(
            "guide",
            None,
            vec![node("はじめに", Some("guide/getting-started/"), vec![])],
        )];
        let items = build_breadcrumbs(&NavTrails::new(&nav), "guide/getting-started/", &resolver());
        let view: Vec<(&str, Option<&str>)> =
            items.iter().map(|b| (b.title, b.url.as_deref())).collect();
        assert_eq!(view, [("guide", None), ("はじめに", None)]);
    }
}
