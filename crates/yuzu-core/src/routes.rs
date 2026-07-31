//! ページ route（出力 URL）の一意性と妥当性の検証。
//!
//! - `route-conflict` — `foo.md` と `foo/index.md` はどちらも route `foo/` になり、
//!   同じ `foo/index.html` を出力する。レンダリングはページ並列（rayon）なので
//!   検出しないと**勝者が実行ごとに変わる**（出力マニフェストは `BTreeSet` で
//!   同じ rel を 1 件に潰すため痕跡も残らない）
//! - `unsafe-page-path` — yuzu は slug 化をせずファイル名をそのまま route にするため、
//!   `#` や `?` を含むファイル名は URL にするとフラグメント・クエリとして解釈され、
//!   **リンクが必ず壊れる**（`a#b.md` の `/a#b/` は `/a` ＋ フラグメント `b/`）
//!
//! [`crate::validate_aliases`] と対になる「出力 URL」の担当で、
//! check（診断一覧）と render（書き出し前の中断）の両方から呼ぶ。

use std::collections::HashMap;

use crate::diagnostics::{DiagBase, Diagnostic, Severity};
use crate::model::Page;

/// route が壊れる文字。
///
/// - `#` `?` は URL 構文として解釈されてリンクが切れる
/// - `%` は**パーセントエンコード済みに見える**ため、`a%23b.md` は
///   `dist/a%23b/index.html` へ出力されるのに URL `/a%23b/` はサーバ側で
///   `a#b/` へデコードされ、物理パスと食い違って 404 になる
/// - 引用符・山括弧は生成 HTML の属性や `<script>` の文脈を壊す
///   （テンプレートの `| url` フィルタがエスケープするが、リンクの切れは直らない）
///
/// 半角スペースは含めない（ブラウザが `%20` へ補正するため実害が薄く、
/// 日本語プロジェクトでは誤検知が多い）
const UNSAFE_PATH_CHARS: &[char] = &['"', '\'', '<', '>', '#', '?', '%', '\\', '`'];

/// 全ページの route（出力 URL）の一意性と妥当性を検証する。
///
/// いずれも `Severity::Error`（check は失敗・build は書き出し前に中断）。
/// ファイル配置そのものの問題なので位置情報は持たない
/// （`directory-too-deep` と同じ扱いで `span` は None）。
pub fn validate_routes(pages: &[Page]) -> Vec<Diagnostic> {
    let mut diags = Vec::new();
    // 先着（走査はパス順で安定するので、どちらを衝突側として報告するかは決定的）
    let mut claimed: HashMap<&str, &Page> = HashMap::new();

    for page in pages {
        check_page_path(page, &mut diags);
        match claimed.get(page.route.as_str()) {
            Some(first) => diags.push(Diagnostic {
                rule: "route-conflict",
                severity: Severity::Error,
                base: DiagBase::Content,
                rel: page.rel.clone(),
                span: None,
                message: format!(
                    "URL /{} がページ {} と衝突しています（`x.md` と `x/index.md` は同じ URL になります）。どちらかのファイル名を変えてください",
                    page.route,
                    first.rel.display()
                ),
                fix: None,
            }),
            None => {
                claimed.insert(page.route.as_str(), page);
            }
        }
    }
    diags
}

/// ファイル名に URL を壊す文字が入っていないか。
///
/// **警告ではなくエラーにする**。route → URL の変換はテンプレートだけでなく
/// 検索インデックス・llms.txt・sitemap にも波及し、テンプレート段階では
/// 「パスの一部の `#`」と「URL 構文の `#`」を区別できないため、
/// 生成物のあちこちに壊れたリンクが出る。書き出す前に止めるのが唯一の整合策
fn check_page_path(page: &Page, out: &mut Vec<Diagnostic>) {
    let rel = crate::urlpath::rel_to_slash(&page.rel);
    let found: Vec<char> = UNSAFE_PATH_CHARS
        .iter()
        .copied()
        .filter(|c| rel.contains(*c))
        .chain(rel.chars().filter(|c| c.is_control()))
        .collect();
    if found.is_empty() {
        return;
    }
    let list = found
        .iter()
        .map(|c| format!("`{c}`"))
        .collect::<Vec<_>>()
        .join(" ");
    out.push(Diagnostic {
        rule: "unsafe-page-path",
        severity: Severity::Error,
        base: DiagBase::Content,
        rel: page.rel.clone(),
        span: None,
        message: format!(
            "ファイル名に URL で意味を持つ文字（{list}）が含まれています。yuzu はファイル名をそのまま URL にするため、このページへのリンクが壊れます。ファイル名を変えてください"
        ),
        fix: None,
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Frontmatter;
    use std::path::PathBuf;

    fn page(rel: &str, route: &str) -> Page {
        Page {
            src: PathBuf::from("/content").join(rel),
            rel: PathBuf::from(rel),
            route: route.to_string(),
            frontmatter: Frontmatter::default(),
            title: "t".to_string(),
            toc: Vec::new(),
            labels: Vec::new(),
            crossref_offset: Default::default(),
            source: "# t\n".to_string(),
        }
    }

    #[test]
    fn 衝突がなければ診断ゼロ() {
        let pages = [
            page("index.md", ""),
            page("guide/index.md", "guide/"),
            page("guide/start.md", "guide/start/"),
        ];
        assert!(validate_routes(&pages).is_empty());
    }

    #[test]
    fn 同じ_url_になるページを検出する() {
        // 走査順（sort_by_file_name）では guide/index.md が先に来る
        let pages = [page("guide/index.md", "guide/"), page("guide.md", "guide/")];
        let diags = validate_routes(&pages);

        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].rule, "route-conflict");
        assert_eq!(diags[0].severity, Severity::Error);
        assert_eq!(
            diags[0].rel,
            PathBuf::from("guide.md"),
            "報告は後着のページ"
        );
        assert!(
            diags[0].message.contains("guide/index.md"),
            "先着のページも文面に出す: {}",
            diags[0].message
        );
    }

    #[test]
    fn 診断は位置情報を持たない() {
        let pages = [page("a/index.md", "a/"), page("a.md", "a/")];
        assert!(validate_routes(&pages)[0].span.is_none());
    }

    #[test]
    fn 三つ以上の衝突は先着以外をすべて報告する() {
        let pages = [
            page("a/index.md", "a/"),
            page("a.md", "a/"),
            page("b.md", "a/"),
        ];
        assert_eq!(
            validate_routes(&pages)
                .iter()
                .filter(|d| d.rule == "route-conflict")
                .count(),
            2
        );
    }

    #[test]
    fn url_で意味を持つ文字を含むファイル名はエラーになる() {
        // `#` `?` はリンクが必ず壊れる。警告では生成物に壊れた URL が残るため
        // `a%23b.md` は「エンコード済みに見える」ため物理パスと URL が食い違う
        for rel in [r#"a"b.md"#, "a#b.md", "a?b.md", "a%23b.md", "guide/x<y>.md"] {
            let route = rel.trim_end_matches(".md").to_string() + "/";
            let diags = validate_routes(&[page(rel, &route)]);
            let hits: Vec<_> = diags
                .iter()
                .filter(|d| d.rule == "unsafe-page-path")
                .collect();
            assert_eq!(hits.len(), 1, "{rel:?}: {diags:?}");
            assert_eq!(hits[0].severity, Severity::Error);
            assert!(hits[0].span.is_none(), "ファイル配置の問題なので span なし");
        }
    }

    #[test]
    fn 通常のファイル名は報告しない() {
        // 日本語・ハイフン・アンダースコア・半角スペースは対象外
        for (rel, route) in [
            ("index.md", ""),
            ("guide/getting-started.md", "guide/getting-started/"),
            ("設計/概要.md", "設計/概要/"),
            ("a b.md", "a b/"),
        ] {
            let diags = validate_routes(&[page(rel, route)]);
            assert!(diags.is_empty(), "{rel:?}: {diags:?}");
        }
    }
}
