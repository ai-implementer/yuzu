//! base path 解決と `.md` 相互リンクの解決。
//!
//! 方針（凍結）: 相対 URL の深さ計算はせず、常に `baseUrl` 付きの
//! 絶対パスへ統一する（pretty URL と相対パスの組み合わせ事故を避ける）。
//!
//! route → URL のパーセントエンコードは `yuzu_core::urlpath::encode_path` が唯一の
//! 変換点。`routes` の HashMap は生 route をキーに持ち、`page_url` / `md_url` と
//! `rewrite` の返り値だけがエンコード済み（[`UrlResolver::base`] は設定値そのまま）。

use std::collections::HashMap;

use yuzu_core::urlpath::{
    encode_path, percent_decode, rel_to_slash, resolve_relative, split_suffix,
};
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
            .map(|p| (rel_to_slash(&p.rel), p.route.clone()))
            .collect();
        Self {
            base: base_url.to_string(),
            routes,
        }
    }

    pub fn base(&self) -> &str {
        &self.base
    }

    /// route → 配信 URL（例: `guide/` → `/docs/guide/`、`設計/` → `/docs/%E8%A8%AD%E8%A8%88/`）
    pub fn page_url(&self, route: &str) -> String {
        format!("{}{}", self.base, encode_path(route))
    }

    /// route → ページ単位 Markdown の配信 URL（例: `guide/intro/` → `/docs/guide/intro.md`）
    pub fn md_url(&self, route: &str) -> String {
        if route.is_empty() {
            format!("{}index.md", self.base)
        } else {
            format!(
                "{}{}.md",
                self.base,
                encode_path(route.trim_end_matches('/'))
            )
        }
    }

    /// テーマアセットのベース URL（末尾スラッシュ付き）
    pub fn asset_url(&self) -> String {
        format!("{}_assets/", self.base)
    }

    /// 設定で指定されたサイト内パス（ロゴ等）を配信 URL へ解決する。
    /// フル URL はそのまま、`/foo` と `foo` はともにサイトルート起点として base を前置する
    pub fn public_url(&self, path: &str) -> String {
        if path.contains("://") {
            return path.to_string();
        }
        format!("{}{}", self.base, path.trim_start_matches('/'))
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

        // `/foo` 始まり → public/ 資産等のサイト絶対参照。base を前置する。
        // 著者が書いた URL をそのまま使う（comrak の escape_href が非 ASCII と
        // 空白を `%XX` にし、`%XX` は素通しするので、ここで再エンコードしない）
        if let Some(rest) = path.strip_prefix('/') {
            return Some(format!("{}{}{}", self.base, rest, suffix));
        }

        // content 相対の参照は著者がエンコード済みで書いた形（`my%20page.md`）を
        // 生のファイル名へ戻してから照合する（linkcheck と同じ規則）
        let path = percent_decode(path);
        let path = path.as_str();

        // 相対 `.md` リンク → ページ route へ解決
        if path.ends_with(".md") {
            let dir = page.rel.parent().map(rel_to_slash).unwrap_or_default();
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
            return Some(format!("{}{}{}", self.base, encode_path(&route), suffix));
        }

        // その他の相対参照のうち拡張子付き（画像・添付等）は content の同伴
        // アセットとして dist の同じ相対パスへコピーされるため、base 付き絶対
        // URL へ解決する（ページは `guide/foo/index.html` に置かれるので相対の
        // ままでは階層がずれる）。判定はリンク検査（linkcheck）と同じ
        // 「末尾セグメントに `.` を含む」。ディレクトリ風リンクは配信形態
        // 依存のため従来どおり触らない
        let last = path.rsplit('/').next().unwrap_or(path);
        if last.contains('.') {
            let dir = page.rel.parent().map(rel_to_slash).unwrap_or_default();
            let resolved = resolve_relative(&dir, path);
            return Some(format!("{}{}{}", self.base, encode_path(&resolved), suffix));
        }
        None
    }
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
            labels: Vec::new(),
            crossref_offset: Default::default(),
            generated: None,
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
    fn 相対の同伴アセット参照は_content_相対パスの絶対_url_へ解決される() {
        let (r, p) = resolver("/docs/");
        // ページは guide/getting-started.md → 画像は content/guide/ 基準
        assert_eq!(
            r.rewrite(&p, "diagram.png").as_deref(),
            Some("/docs/guide/diagram.png")
        );
        assert_eq!(
            r.rewrite(&p, "./img/shot.png").as_deref(),
            Some("/docs/guide/img/shot.png")
        );
        assert_eq!(
            r.rewrite(&p, "../top.png").as_deref(),
            Some("/docs/top.png")
        );
        // 添付ファイル（画像以外）も同じ規則
        assert_eq!(
            r.rewrite(&p, "spec.pdf").as_deref(),
            Some("/docs/guide/spec.pdf")
        );
    }

    #[test]
    fn 拡張子のないディレクトリ風リンクは触らない() {
        let (r, p) = resolver("/docs/");
        assert!(r.rewrite(&p, "guide/").is_none());
        assert!(r.rewrite(&p, "some-page").is_none());
    }

    #[test]
    fn テンプレート用_url() {
        let (r, _) = resolver("/docs/");
        assert_eq!(r.page_url("guide/"), "/docs/guide/");
        assert_eq!(r.asset_url(), "/docs/_assets/");
    }

    #[test]
    fn page_url_と_md_url_は_route_をパーセントエンコードする() {
        let (r, _) = resolver("/docs/");
        assert_eq!(
            r.page_url("設計/概 要#1/"),
            "/docs/%E8%A8%AD%E8%A8%88/%E6%A6%82%20%E8%A6%81%231/"
        );
        assert_eq!(
            r.md_url("設計/概 要#1/"),
            "/docs/%E8%A8%AD%E8%A8%88/%E6%A6%82%20%E8%A6%81%231.md"
        );
        assert_eq!(r.md_url(""), "/docs/index.md");
        // `%` を含むファイル名は二重にならず `%25` へ（サーバのデコードで物理パスに一致）
        assert_eq!(r.page_url("a%23b/"), "/docs/a%2523b/");
        // base は設定値そのまま（エンコードの対象外）
        let (r, _) = resolver("https://example.com/設計/");
        assert_eq!(r.page_url("x/"), "https://example.com/設計/x/");
    }

    fn resolver_with(base: &str, pages: Vec<Page>, current: Page) -> (UrlResolver, Page) {
        let site = SiteModel {
            pages,
            nav: Vec::new(),
        };
        (UrlResolver::new(base, &site), current)
    }

    #[test]
    fn 本文リンクの_route_はエンコードして埋める() {
        let (r, p) = resolver_with(
            "/docs/",
            vec![
                page("index.md", ""),
                page("設計/概 要.md", "設計/概 要/"),
                page("設計/index.md", "設計/"),
            ],
            page("設計/index.md", "設計/"),
        );
        // 生のファイル名で書いたリンク
        assert_eq!(
            r.rewrite(&p, "概 要.md#見出し").as_deref(),
            Some("/docs/%E8%A8%AD%E8%A8%88/%E6%A6%82%20%E8%A6%81/#見出し"),
            "suffix はエンコードしない"
        );
        // 著者がエンコード済みで書いたリンクも同じページへ解決される
        assert_eq!(
            r.rewrite(&p, "%E6%A6%82%20%E8%A6%81.md").as_deref(),
            Some("/docs/%E8%A8%AD%E8%A8%88/%E6%A6%82%20%E8%A6%81/")
        );
        assert_eq!(r.rewrite(&p, "../index.md").as_deref(), Some("/docs/"));
    }

    #[test]
    fn 同伴アセットの相対参照はデコードしてから再エンコードする() {
        let (r, p) = resolver("/docs/");
        assert_eq!(
            r.rewrite(&p, "my%20image.png").as_deref(),
            Some("/docs/guide/my%20image.png")
        );
        assert_eq!(
            r.rewrite(&p, "my image.png").as_deref(),
            Some("/docs/guide/my%20image.png")
        );
        assert_eq!(
            r.rewrite(&p, "図 1.png").as_deref(),
            Some("/docs/guide/%E5%9B%B3%201.png")
        );
    }

    #[test]
    fn ルート絶対参照は著者の記述をそのまま通す() {
        // 再エンコードすると `%E8` が `%25E8` に化ける。comrak の escape_href が
        // 非 ASCII を処理し `%XX` は素通しするので、ここでは触らない
        let (r, p) = resolver("/docs/");
        assert_eq!(
            r.rewrite(&p, "/%E8%A8%AD%E8%A8%88/").as_deref(),
            Some("/docs/%E8%A8%AD%E8%A8%88/")
        );
        assert_eq!(r.rewrite(&p, "/設計/").as_deref(), Some("/docs/設計/"));
    }

    #[test]
    fn public_url_は_base_を前置しフル_url_はそのまま() {
        let (r, _) = resolver("/docs/");
        assert_eq!(r.public_url("/images/logo.svg"), "/docs/images/logo.svg");
        assert_eq!(r.public_url("images/logo.svg"), "/docs/images/logo.svg");
        assert_eq!(
            r.public_url("https://cdn.example.com/logo.png"),
            "https://cdn.example.com/logo.png"
        );
    }
}
