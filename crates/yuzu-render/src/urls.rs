//! base path 解決と `.md` 相互リンクの解決。
//!
//! 方針（凍結）: 相対 URL の深さ計算はせず、常に `baseUrl` 付きの
//! 絶対パスへ統一する（pretty URL と相対パスの組み合わせ事故を避ける）。

use std::collections::HashMap;
use std::path::Path;

use yuzu_core::{Page, SiteModel, UrlRewriter};

/// URL 解決器。本文リンクの書き換え（[`UrlRewriter`]）と、
/// テンプレートに渡すナビ・アセット URL の生成を担う
pub struct UrlResolver {
    /// 正規化済み baseUrl（常に末尾スラッシュ付き。例: `/` / `/docs/`）
    base: String,
    /// `content/` からの相対パス（`/` 区切り）→ route
    routes: HashMap<String, String>,
}

impl UrlResolver {
    pub fn new(base_url: &str, site: &SiteModel) -> Self {
        let routes = site
            .pages
            .iter()
            .map(|p| (rel_slash(&p.rel), p.route.clone()))
            .collect();
        Self {
            base: base_url.to_string(),
            routes,
        }
    }

    pub fn base(&self) -> &str {
        &self.base
    }

    /// route → 配信 URL（例: `guide/` → `/docs/guide/`）
    pub fn page_url(&self, route: &str) -> String {
        format!("{}{}", self.base, route)
    }

    /// テーマアセットのベース URL（末尾スラッシュ付き）
    pub fn asset_url(&self) -> String {
        format!("{}_assets/", self.base)
    }
}

impl UrlRewriter for UrlResolver {
    fn rewrite(&self, page: &Page, url: &str) -> Option<String> {
        // フラグメントのみ・スキーム付き（外部リンク等）は触らない
        if url.is_empty() || url.starts_with('#') {
            return None;
        }
        if url.contains("://") || url.starts_with("mailto:") || url.starts_with("tel:") {
            return None;
        }

        let (path, suffix) = split_suffix(url);

        // `/foo` 始まり → public/ 資産等のサイト絶対参照。base を前置する
        if let Some(rest) = path.strip_prefix('/') {
            return Some(format!("{}{}{}", self.base, rest, suffix));
        }

        // 相対 `.md` リンク → ページ route へ解決
        if path.ends_with(".md") {
            let dir = page.rel.parent().map(rel_slash).unwrap_or_default();
            let resolved = resolve_relative(&dir, path);
            let route = match self.routes.get(&resolved) {
                Some(route) => route.clone(),
                None => {
                    tracing::warn!(
                        from = %page.rel.display(),
                        to = url,
                        "リンク先の Markdown が見つかりません（URL は機械変換します）"
                    );
                    guess_route(&resolved)
                }
            };
            return Some(format!("{}{}{}", self.base, route, suffix));
        }

        // その他の相対参照（画像等）はそのまま
        None
    }
}

/// `?query` / `#fragment` を切り離す
fn split_suffix(url: &str) -> (&str, &str) {
    match url.find(['?', '#']) {
        Some(i) => (&url[..i], &url[i..]),
        None => (url, ""),
    }
}

/// 相対パスを `dir` 基準で解決し、`/` 区切りに正規化する
fn resolve_relative(dir: &str, target: &str) -> String {
    let mut parts: Vec<&str> = if dir.is_empty() {
        Vec::new()
    } else {
        dir.split('/').collect()
    };
    for seg in target.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            s => parts.push(s),
        }
    }
    parts.join("/")
}

/// リンク切れ時のフォールバック: `.md` パスを pretty URL へ機械変換する
fn guess_route(rel: &str) -> String {
    let stem = rel.strip_suffix(".md").unwrap_or(rel);
    match stem.strip_suffix("index") {
        Some(dir) => dir.trim_end_matches('/').to_string() + "/",
        None => format!("{stem}/"),
    }
    .trim_start_matches('/')
    .to_string()
}

/// 相対パスを `/` 区切り文字列へ（Windows 対策）
fn rel_slash(rel: &Path) -> String {
    rel.iter()
        .map(|c| c.to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use yuzu_core::{Frontmatter, SiteModel};

    fn page(rel: &str, route: &str) -> Page {
        Page {
            src: PathBuf::from("/tmp").join(rel),
            rel: PathBuf::from(rel),
            route: route.to_string(),
            frontmatter: Frontmatter::default(),
            title: rel.to_string(),
            toc: Vec::new(),
            source: String::new(),
        }
    }

    fn resolver(base: &str) -> (UrlResolver, Page) {
        let site = SiteModel {
            pages: vec![
                page("index.md", ""),
                page("guide/index.md", "guide/"),
                page("guide/getting-started.md", "guide/getting-started/"),
            ],
            nav: Vec::new(),
        };
        (
            UrlResolver::new(base, &site),
            page("guide/getting-started.md", "guide/getting-started/"),
        )
    }

    #[test]
    fn 絶対パスは_base_を前置する() {
        let (r, p) = resolver("/docs/");
        assert_eq!(
            r.rewrite(&p, "/images/logo.svg").as_deref(),
            Some("/docs/images/logo.svg")
        );
    }

    #[test]
    fn 相対_md_リンクは_route_へ解決される() {
        let (r, p) = resolver("/docs/");
        assert_eq!(r.rewrite(&p, "./index.md").as_deref(), Some("/docs/guide/"));
        assert_eq!(r.rewrite(&p, "../index.md").as_deref(), Some("/docs/"));
        assert_eq!(
            r.rewrite(&p, "getting-started.md#見出し").as_deref(),
            Some("/docs/guide/getting-started/#見出し")
        );
    }

    #[test]
    fn 外部リンクとフラグメントは触らない() {
        let (r, p) = resolver("/docs/");
        assert!(r.rewrite(&p, "https://example.com/a.md").is_none());
        assert!(r.rewrite(&p, "#section").is_none());
        assert!(r.rewrite(&p, "mailto:a@example.com").is_none());
    }

    #[test]
    fn リンク切れは機械変換で警告付きフォールバック() {
        let (r, p) = resolver("/");
        assert_eq!(
            r.rewrite(&p, "missing.md").as_deref(),
            Some("/guide/missing/")
        );
    }

    #[test]
    fn テンプレート用_url() {
        let (r, _) = resolver("/docs/");
        assert_eq!(r.page_url("guide/"), "/docs/guide/");
        assert_eq!(r.asset_url(), "/docs/_assets/");
    }
}
