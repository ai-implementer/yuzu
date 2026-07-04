//! ナビツリーの自動生成（ディレクトリ階層＝ナビ階層）

use std::collections::BTreeMap;

use crate::model::{NavNode, Page};
use crate::scan::rel_to_slash;

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

    for page in pages {
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
