//! 内部リンク・アンカーの静的検査（`yuzu check`）。
//!
//! - 外部 URL（スキーム付き・mailto・tel）には触れない（決定的・オフライン）
//! - URL の分類は yuzu-render の `UrlResolver::rewrite` と同じ規則
//!   （crates/yuzu-render/src/urls.rs — 変更時は両方を揃えること）
//! - アンカーは `Page.toc` の id（本文 HTML と同一の採番）で照合する。
//!   自前 slugify はしない

use std::collections::HashMap;
use std::path::Path;

use crate::MarkdownOptions;
use crate::diagnostics::{Diagnostic, Severity};
use crate::error::CoreError;
use crate::markdown::{self, LinkRef};
use crate::model::Page;
use crate::urlpath::{rel_to_slash, resolve_relative, split_suffix};

/// ビルドが生成する route 以外のパス（ルート絶対リンクの有効ターゲット）
const GENERATED: &[&str] = &["llms.txt", "llms-full.txt"];
const GENERATED_DIRS: &[&str] = &["_assets/", "_search/"];

pub(crate) fn check_links(
    pages: &[Page],
    public_dir: Option<&Path>,
    content_dir: &Path,
    opts: &MarkdownOptions,
) -> Result<Vec<Diagnostic>, CoreError> {
    // rel（/ 区切り）→ ページ。draft も引ける（専用メッセージを出すため）
    let by_rel: HashMap<String, &Page> = pages.iter().map(|p| (rel_to_slash(&p.rel), p)).collect();
    // route → ページ。有効ターゲットは非 draft のみ（ビルド成果物に実在するもの）
    let by_route: HashMap<&str, &Page> = pages
        .iter()
        .filter(|p| !p.frontmatter.draft)
        .map(|p| (p.route.as_str(), p))
        .collect();

    let mut out = Vec::new();
    for page in pages {
        for link in markdown::extract_link_refs(&page.source, opts) {
            check_one(
                page,
                &link,
                &by_rel,
                &by_route,
                public_dir,
                content_dir,
                &mut out,
            );
        }
    }
    Ok(out)
}

fn check_one(
    page: &Page,
    link: &LinkRef,
    by_rel: &HashMap<String, &Page>,
    by_route: &HashMap<&str, &Page>,
    public_dir: Option<&Path>,
    content_dir: &Path,
    out: &mut Vec<Diagnostic>,
) {
    let url = link.url.as_str();
    if url.is_empty() {
        return;
    }

    // 同一ページ内アンカー
    if let Some(frag) = url.strip_prefix('#') {
        if !has_anchor(page, frag) {
            push(
                out,
                page,
                link,
                "broken-anchor",
                format!("このページに見出し `#{frag}` がありません"),
            );
        }
        return;
    }

    // 外部参照は検査しない
    if url.contains("://") || url.starts_with("mailto:") || url.starts_with("tel:") {
        return;
    }

    let (path, suffix) = split_suffix(url);
    let frag = suffix.split_once('#').map(|(_, f)| f);

    // ルート絶対（`/foo`）→ public/・ページ route・ビルド生成物に照合
    if let Some(rest) = path.strip_prefix('/') {
        check_absolute(page, link, rest, frag, by_route, public_dir, out);
        return;
    }

    // 相対 `.md` リンク → ページに照合
    if path.ends_with(".md") {
        let dir = page.rel.parent().map(rel_to_slash).unwrap_or_default();
        let resolved = resolve_relative(&dir, path);
        match by_rel.get(&resolved) {
            None => push(
                out,
                page,
                link,
                "broken-link",
                format!("リンク先 `{url}` が見つかりません"),
            ),
            Some(target) if target.frontmatter.draft => {
                push(
                    out,
                    page,
                    link,
                    "broken-link",
                    format!("リンク先 `{resolved}` は draft のため公開サイトに含まれません"),
                );
            }
            Some(target) => {
                if let Some(frag) = frag {
                    if !has_anchor(target, frag) {
                        push(
                            out,
                            page,
                            link,
                            "broken-anchor",
                            format!("リンク先 `{resolved}` に見出し `#{frag}` がありません"),
                        );
                    }
                }
            }
        }
        return;
    }

    // その他の相対参照（画像等）: 拡張子付きのみ content/ 内の実在を確認する
    // （`guide/` のようなディレクトリ風リンクは配信形態依存のため静的検証しない）
    let last = path.rsplit('/').next().unwrap_or(path);
    if last.contains('.') {
        let dir = page.rel.parent().map(rel_to_slash).unwrap_or_default();
        let resolved = resolve_relative(&dir, path);
        if !content_dir.join(&resolved).is_file() {
            let kind = if link.is_image { "画像" } else { "参照先" };
            push(
                out,
                page,
                link,
                "broken-link",
                format!("{kind} `{url}` が content/ に見つかりません"),
            );
        }
    }
}

/// ルート絶対パスの照合
fn check_absolute(
    page: &Page,
    link: &LinkRef,
    rest: &str,
    frag: Option<&str>,
    by_route: &HashMap<&str, &Page>,
    public_dir: Option<&Path>,
    out: &mut Vec<Diagnostic>,
) {
    // ビルド生成物
    if GENERATED.contains(&rest) || GENERATED_DIRS.iter().any(|d| rest.starts_with(d)) {
        return;
    }
    // public/ のファイル
    if public_dir.is_some_and(|dir| dir.join(rest).is_file()) {
        return;
    }
    // ページ route（末尾スラッシュの省略は許容）
    let target = by_route.get(rest).or_else(|| {
        if rest.ends_with('/') {
            None
        } else {
            by_route.get(format!("{rest}/").as_str())
        }
    });
    if let Some(target) = target {
        if let Some(frag) = frag {
            if !has_anchor(target, frag) {
                push(
                    out,
                    page,
                    link,
                    "broken-anchor",
                    format!("リンク先 `/{rest}` に見出し `#{frag}` がありません"),
                );
            }
        }
        return;
    }
    push(
        out,
        page,
        link,
        "broken-link",
        format!("リンク先 `/{rest}` が見つかりません（public/ にもページ route にもありません）"),
    );
}

/// アンカーを `Page.toc` の id（本文 HTML と同一採番）で照合する。
/// percent エンコードされた日本語フラグメントはデコードしてから比較
fn has_anchor(page: &Page, frag: &str) -> bool {
    if page.toc.iter().any(|t| t.id == frag) {
        return true;
    }
    let decoded = percent_decode(frag);
    decoded != frag && page.toc.iter().any(|t| t.id == decoded)
}

/// `%XX` の最小限デコード（不正な並びはそのまま残す。新規依存を避ける）
fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hi = (bytes[i + 1] as char).to_digit(16);
            let lo = (bytes[i + 2] as char).to_digit(16);
            if let (Some(hi), Some(lo)) = (hi, lo) {
                out.push((hi * 16 + lo) as u8);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn push(
    out: &mut Vec<Diagnostic>,
    page: &Page,
    link: &LinkRef,
    rule: &'static str,
    message: String,
) {
    out.push(Diagnostic {
        rule,
        severity: Severity::Error,
        rel: page.rel.clone(),
        span: Some(link.span),
        message,
        fix: None,
    });
}

#[cfg(test)]
mod tests {
    use super::percent_decode;

    #[test]
    fn percent_decode_の基本() {
        assert_eq!(percent_decode("%E8%A6%8B%E5%87%BA%E3%81%97"), "見出し");
        assert_eq!(percent_decode("plain"), "plain");
        assert_eq!(percent_decode("a%2Gb"), "a%2Gb", "不正な 16 進はそのまま");
        assert_eq!(percent_decode("%"), "%", "末尾の % もそのまま");
    }
}
