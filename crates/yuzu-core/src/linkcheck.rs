//! 内部リンク・アンカーの静的検査（`yuzu check`）。
//!
//! - 外部 URL（スキーム付き・mailto・tel）には触れない（決定的・オフライン）。
//!   ただし http / https の出現箇所は [`LinkReport::external`] で返す = 到達性の
//!   検査（ネットワーク I/O）は cli 層の opt-in（`yuzu check --external-links`）だけが
//!   行い、core は既定経路をオフラインに保つ
//! - URL の分類は yuzu-render の `UrlResolver::rewrite` と同じ規則
//!   （crates/yuzu-render/src/urls.rs — 変更時は両方を揃えること。外部参照の判定は
//!   `urlpath::is_external_url` の 1 実装を共有）
//! - 著者が書いたパスは `%XX` を**デコードしてから**照合する（`my%20page.md` /
//!   `/%E8%A8%AD…/`）。ブラウザで辿れるリンクは検査も通る、が契約。
//!   `?` / `#` 以降の suffix はデコードの対象外（フラグメントは `has_anchor` が別途）
//! - アンカーは `Page.toc` の id（本文 HTML と同一の採番）で照合する。
//!   自前 slugify はしない

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::MarkdownOptions;
use crate::diagnostics::{DiagBase, Diagnostic};
use crate::error::CoreError;
use crate::markdown::{self, LinkRef};
use crate::model::{Page, SourceSpan};
use crate::rules;
use crate::urlpath::{
    is_external_url, is_http_url, percent_decode, rel_to_slash, resolve_relative, split_suffix,
};

/// ビルドが生成する route 以外のパス（ルート絶対リンクの有効ターゲット）
const GENERATED: &[&str] = &["llms.txt", "llms-full.txt"];
const GENERATED_DIRS: &[&str] = &["_assets/", "_search/"];

/// 本文中の外部リンク（http / https）の出現箇所。
/// core はネットワークに触れないので、到達性の検査は呼び出し側（cli の opt-in）が行う
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalLink {
    /// リンク元ページの content 相対パス
    pub rel: PathBuf,
    /// リンクの位置（診断に付ける）
    pub span: SourceSpan,
    /// 書かれたとおりの URL
    pub url: String,
    pub is_image: bool,
}

/// [`check_links`] の結果。内部リンクの診断と、検査対象外だった外部リンクの一覧
#[derive(Debug, Default)]
pub struct LinkReport {
    pub diags: Vec<Diagnostic>,
    pub external: Vec<ExternalLink>,
}

pub(crate) fn check_links(
    pages: &[Page],
    public_dir: Option<&Path>,
    content_dir: &Path,
    opts: &MarkdownOptions,
) -> Result<LinkReport, CoreError> {
    // rel（/ 区切り）→ ページ。draft も引ける（専用メッセージを出すため）
    let by_rel: HashMap<String, &Page> = pages.iter().map(|p| (rel_to_slash(&p.rel), p)).collect();
    // route → ページ。有効ターゲットは非 draft のみ（ビルド成果物に実在するもの）
    let by_route: HashMap<&str, &Page> = pages
        .iter()
        .filter(|p| !p.frontmatter.draft)
        .map(|p| (p.route.as_str(), p))
        .collect();

    let mut out = Vec::new();
    let mut external = Vec::new();
    // 合成ページ（用語集）は**リンク先としてだけ**有効にする。上の by_rel / by_route
    // には入れて `[用語集](../glossary.md#api)` を解決可能にしつつ、リンク元としては
    // 見ない（辞書の説明文に書かれたリンクを実在しないファイルの診断として出さない）
    for page in pages.iter().filter(|p| !p.is_generated()) {
        for link in markdown::extract_link_refs(&page.source, opts) {
            // http / https は到達性検査（opt-in）へ回す。mailto / tel / 他スキームは
            // 検査しようがないので捨てる（従来どおり）
            if is_http_url(&link.url) {
                external.push(ExternalLink {
                    rel: page.rel.clone(),
                    span: link.span,
                    url: link.url.clone(),
                    is_image: link.is_image,
                });
                continue;
            }
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
    Ok(LinkReport {
        diags: out,
        external,
    })
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
                rules::BROKEN_ANCHOR,
                format!("このページに見出し `#{frag}` がありません"),
            );
        }
        return;
    }

    // 外部参照は検査しない（http / https は呼び出し側で external へ回し済み）
    if is_external_url(url) {
        return;
    }

    let (path, suffix) = split_suffix(url);
    let frag = suffix.split_once('#').map(|(_, f)| f);

    // 絶対・相対の分類は**デコード前**の文字列で行う（render 側の `rewrite` と同じ
    // 順序）。`%2Flogo.png` はデコードすると `/logo.png` だが相対参照として
    // `<dir>/logo.png` へ解決されるので、先にデコードすると分類が render とずれる
    // ルート絶対（`/foo`）→ public/・ページ route・ビルド生成物に照合
    if let Some(rest) = path.strip_prefix('/') {
        // 著者がエンコード済みで書いた `/%E8%A8%AD…/` を生の route へ戻す
        let rest = percent_decode(rest);
        check_absolute(page, link, &rest, frag, by_route, public_dir, out);
        return;
    }

    // content 相対の参照は生のファイル名へ戻してから照合する（suffix はデコードしない）
    let path = percent_decode(path);
    let path = path.as_str();

    // 相対 `.md` リンク → ページに照合
    if path.ends_with(".md") {
        let dir = page.rel.parent().map(rel_to_slash).unwrap_or_default();
        let resolved = resolve_relative(&dir, path);
        match by_rel.get(&resolved) {
            None => push(
                out,
                page,
                link,
                rules::BROKEN_LINK,
                format!("リンク先 `{url}` が見つかりません"),
            ),
            Some(target) if target.frontmatter.draft => {
                push(
                    out,
                    page,
                    link,
                    rules::BROKEN_LINK,
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
                            rules::BROKEN_ANCHOR,
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
                rules::BROKEN_LINK,
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
                    rules::BROKEN_ANCHOR,
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
        rules::BROKEN_LINK,
        format!("リンク先 `/{rest}` が見つかりません（public/ にもページ route にもありません）"),
    );
}

/// アンカーを `Page.toc` の id（本文 HTML と同一採番）で照合する。
/// percent エンコードされた日本語フラグメントはデコードしてから比較
fn has_anchor(page: &Page, frag: &str) -> bool {
    // 見出しアンカーと図表ラベル（`{#fig:deps}`）の両方が有効ターゲット
    let known =
        |id: &str| page.toc.iter().any(|t| t.id == id) || page.labels.iter().any(|l| l.id == id);
    if known(frag) {
        return true;
    }
    let decoded = percent_decode(frag);
    decoded != frag && known(&decoded)
}

fn push(
    out: &mut Vec<Diagnostic>,
    page: &Page,
    link: &LinkRef,
    rule: rules::Rule,
    message: String,
) {
    out.push(Diagnostic {
        rule: rule.id,
        severity: rule.severity,
        base: DiagBase::Content,
        rel: page.rel.clone(),
        span: Some(link.span),
        message,
        fix: None,
    });
}
