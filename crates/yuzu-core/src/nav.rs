//! ナビツリーの自動生成（ディレクトリ階層＝ナビ階層）

use std::collections::BTreeMap;

use crate::model::{NavNode, Page};
use crate::urlpath::rel_to_slash;

/// ディレクトリツリーの中間表現
#[derive(Default)]
struct DirNode<'a> {
    /// このディレクトリの `index.md`
    index: Option<&'a Page>,
    /// `index.md` 以外の直下ページ（キー: ファイル stem）
    pages: BTreeMap<String, &'a Page>,
    /// サブディレクトリ（キー: ディレクトリ名）
    dirs: BTreeMap<String, DirNode<'a>>,
}

/// ページ一覧からナビツリーを構築する。
///
/// - ディレクトリ階層がそのまま階層になる
/// - 表示名は frontmatter `title`（→ h1 → ファイル名）
/// - 並び順は `order` 昇順、未指定（`order` なし）は最後尾グループでファイル名順
/// - ディレクトリの表示名・並び順は配下 `index.md` のものを使う
pub(crate) fn build_nav(pages: &[Page]) -> Vec<NavNode> {
    let mut root = DirNode::default();

    // 検索結果ページ等はサイドバーに出さない（pager・パンくずも nav 由来なので
    // 連動して外れる）。用語集は載せる — 判定は Page::in_nav に集約
    for page in pages.iter().filter(|p| p.in_nav()) {
        let parts: Vec<String> = rel_to_slash(&page.rel)
            .split('/')
            .map(String::from)
            .collect();
        let (file, dirs) = parts.split_last().expect("相対パスは空にならない");
        let mut node = &mut root;
        for dir in dirs {
            node = node.dirs.entry(dir.clone()).or_default();
        }
        let stem = file.strip_suffix(".md").unwrap_or(file);
        if stem == "index" {
            node.index = Some(page);
        } else {
            node.pages.insert(stem.to_string(), page);
        }
    }

    to_nav_children(&root)
}

fn to_nav_children(dir: &DirNode) -> Vec<NavNode> {
    let mut children = Vec::new();

    // ルート（および各ディレクトリ）の index.md はそのディレクトリ直下の先頭候補として並べる
    if let Some(index) = dir.index {
        children.push((
            sort_key(index.frontmatter.order, "".to_string()),
            NavNode {
                title: index.title.clone(),
                route: Some(index.route.clone()),
                order: index.frontmatter.order,
                children: Vec::new(),
            },
        ));
    }

    for (stem, page) in &dir.pages {
        children.push((
            sort_key(page.frontmatter.order, stem.clone()),
            NavNode {
                title: page.title.clone(),
                route: Some(page.route.clone()),
                order: page.frontmatter.order,
                children: Vec::new(),
            },
        ));
    }

    for (name, sub) in &dir.dirs {
        let sub_children = to_nav_children(sub);
        // ディレクトリ自体のリンク・表示名・並び順は index.md から取る
        let (title, route, order) = match sub.index {
            Some(index) => (
                index.title.clone(),
                Some(index.route.clone()),
                index.frontmatter.order,
            ),
            None => (name.clone(), None, None),
        };
        // index.md はディレクトリノード自身として表現するので、子から重複を除く
        let sub_children: Vec<NavNode> = sub_children
            .into_iter()
            .filter(|c| c.route.as_deref() != route.as_deref())
            .collect();
        if sub_children.is_empty() && route.is_none() {
            // ページを 1 つも持たない空ディレクトリは出さない
            continue;
        }
        children.push((
            sort_key(order, name.clone()),
            NavNode {
                title,
                route,
                order,
                children: sub_children,
            },
        ));
    }

    children.sort_by(|(a, _), (b, _)| a.cmp(b));
    children.into_iter().map(|(_, node)| node).collect()
}

/// 並び順キー: `order` 昇順 → 未指定は最後尾グループ → 名前順
fn sort_key(order: Option<i64>, name: String) -> (i64, String) {
    (order.unwrap_or(i64::MAX), name)
}

/// 検索結果の絞り込み単位（ナビ第 1 階層）
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NavGroup {
    /// route の先頭セグメント（`guide/x/` → `guide`。ルート直下は `""`）
    pub key: String,
    /// 表示名（`<dir>/index.md` の title。無ければディレクトリ名）
    pub label: String,
}

/// route の先頭セグメント（純関数）。`guide/` も `guide/x/` も `guide`、
/// サイトのトップ（`""`）は `""`。
///
/// これ単体では「ディレクトリ配下か、ルート直下の単独ページか」を区別できない
/// （`about.md` の route も `about/` になる）。区別は [`nav_groups`] が返すキー集合との
/// 照合で行う ＝ **判定規則を 1 つに保つ**ための分担
pub fn route_group_key(route: &str) -> &str {
    route.split_once('/').map_or(route, |(head, _)| head)
}

/// ナビ第 1 階層のうち**ディレクトリ**（＝子を持つノード）をグループ列にする。
///
/// 検索の絞り込みチップに使うので、並びはサイドバーと一致させる
/// （パス順とナビ順は `order` の指定で食い違う）。
/// 子を持たない第 1 階層ノード（`index.md` や `about.md` のような単独ページ）は
/// グループにしない ＝ チップが「ページ 1 枚」にならない
pub fn nav_groups(nodes: &[NavNode]) -> Vec<NavGroup> {
    let mut out: Vec<NavGroup> = Vec::new();
    for node in nodes {
        if node.children.is_empty() {
            continue;
        }
        // index.md を持たないディレクトリノードは route が None なので、
        // 最初の子孫 route からキーを拾う
        let route = node
            .route
            .as_deref()
            .or_else(|| first_route(&node.children));
        let Some(route) = route else { continue };
        let key = route_group_key(route);
        if key.is_empty() || out.iter().any(|g| g.key == key) {
            continue;
        }
        out.push(NavGroup {
            key: key.to_string(),
            label: node.title.clone(),
        });
    }
    out
}

/// 深さ優先で最初に見つかる route
fn first_route(nodes: &[NavNode]) -> Option<&str> {
    for node in nodes {
        if let Some(route) = node.route.as_deref() {
            return Some(route);
        }
        if let Some(route) = first_route(&node.children) {
            return Some(route);
        }
    }
    None
}

#[cfg(test)]
mod group_tests {
    use super::*;

    #[test]
    fn route_からグループキーを取り出す() {
        assert_eq!(route_group_key(""), "");
        assert_eq!(route_group_key("guide/"), "guide");
        assert_eq!(route_group_key("guide/start/"), "guide");
        assert_eq!(route_group_key("guide/a/b/"), "guide");
        // ルート直下の単独ページも形は同じ。除外は nav_groups のキー集合が担う
        assert_eq!(route_group_key("about/"), "about");
    }

    fn node(title: &str, route: Option<&str>, children: Vec<NavNode>) -> NavNode {
        NavNode {
            title: title.to_string(),
            route: route.map(str::to_string),
            order: None,
            children,
        }
    }

    #[test]
    fn ナビ第一階層をナビ順で返す() {
        // order で並べ替わった後のナビを想定（パス順とは違う）
        let nav = vec![
            node("ホーム", Some(""), vec![]),
            node(
                "ガイド",
                Some("guide/"),
                vec![node("はじめに", Some("guide/start/"), vec![])],
            ),
            node(
                "開発",
                Some("development/"),
                vec![node("内部", Some("development/internals/"), vec![])],
            ),
        ];
        assert_eq!(
            nav_groups(&nav),
            vec![
                NavGroup {
                    key: "guide".to_string(),
                    label: "ガイド".to_string()
                },
                NavGroup {
                    key: "development".to_string(),
                    label: "開発".to_string()
                },
            ]
        );
    }

    #[test]
    fn index_の無いディレクトリはディレクトリ名へフォールバックする() {
        // build_nav は index.md が無いディレクトリの title にディレクトリ名を入れる
        let nav = vec![node(
            "guide",
            None,
            vec![node("はじめに", Some("guide/start/"), vec![])],
        )];
        assert_eq!(
            nav_groups(&nav),
            vec![NavGroup {
                key: "guide".to_string(),
                label: "guide".to_string()
            }]
        );
    }

    #[test]
    fn 子を持たない第一階層はグループにしない() {
        // トップページと、ディレクトリを作っていない単独ページ
        let nav = vec![
            node("ホーム", Some(""), vec![]),
            node("概要", Some("about/"), vec![]),
        ];
        assert!(nav_groups(&nav).is_empty());
    }
}
