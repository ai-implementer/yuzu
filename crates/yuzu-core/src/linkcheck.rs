//! 内部リンク・アンカーの静的検査（`yuzu check`）。
//!
//! - 外部 URL（スキーム付き・mailto・tel）には触れない（決定的・オフライン）。
//!   ただし http / https の出現箇所は [`LinkReport::external`] で返す = 到達性の
//!   検査（ネットワーク I/O）は cli 層の opt-in（`yuzu check --external-links`）だけが
//!   行い、core は既定経路をオフラインに保つ
//! - URL の分類は yuzu-render の `UrlResolver::rewrite` と同じ `urlpath::classify_link`
//!   （分類の enum `LinkTarget` を両方が網羅 match で使う。10-12 レビューの R5）
//! - 著者が書いたパスは `%XX` を**デコードしてから**照合する（`my%20page.md` /
//!   `/%E8%A8%AD…/`）。ブラウザで辿れるリンクは検査も通る、が契約。
//!   `?` / `#` 以降の suffix はデコードの対象外（フラグメントは `has_anchor` が別途）
//! - アンカーは `Page.toc` の id（本文 HTML と同一の採番）で照合する。
//!   自前 slugify はしない

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use crate::MarkdownOptions;
use crate::diagnostics::Diagnostic;
use crate::error::CoreError;
use crate::markdown::{self, LinkRef};
use crate::model::{Page, SourceSpan};
use crate::rules;
use crate::site_output::{OutputSwitches, SiteOutput};
use crate::urlpath::{
    LinkTarget, classify_link, is_http_url, percent_decode, rel_to_slash, resolve_relative,
};

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

/// リンク先として照合する集合（ページ・public/・同伴アセット）
struct Targets<'a> {
    /// rel（/ 区切り）→ ページ。draft も引ける（専用メッセージを出すため）
    by_rel: HashMap<String, &'a Page>,
    /// route → ページ。有効ターゲットはビルド成果物に実在するページだけ
    by_route: HashMap<&'a str, &'a Page>,
    public_dir: Option<&'a Path>,
    /// build がコピーする同伴アセット（content 相対・/ 区切り）。`is_file()` で見ると
    /// 隠しファイル・`input.ignore` の対象・シンボリックリンクも「ある」になり、
    /// check は通るのに公開サイトで画像が切れていた。build のコピーと同じ列挙
    /// （`scan_content_assets`）を使う
    assets: HashSet<String>,
    /// draft も出力に含めるビルド（`--drafts`）か
    include_drafts: bool,
    /// 今回出す生成物（`/404.html`・`/llms.txt`・`/_search/` 等）。判断は SiteOutput が持つ
    outputs: &'a OutputSwitches,
}

pub(crate) fn check_links(
    pages: &[Page],
    public_dir: Option<&Path>,
    content_dir: &Path,
    ignore: &[String],
    include_drafts: bool,
    outputs: &OutputSwitches,
    opts: &MarkdownOptions,
) -> Result<LinkReport, CoreError> {
    let targets = Targets {
        by_rel: pages.iter().map(|p| (rel_to_slash(&p.rel), p)).collect(),
        by_route: pages
            .iter()
            .filter(|p| include_drafts || !p.frontmatter.draft)
            .map(|p| (p.route.as_str(), p))
            .collect(),
        public_dir,
        assets: crate::scan::scan_content_assets(content_dir, ignore)?
            .iter()
            .map(|f| rel_to_slash(&f.rel))
            .collect(),
        include_drafts,
        outputs,
    };

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
            check_one(page, &link, &targets, &mut out);
        }
    }
    Ok(LinkReport {
        diags: out,
        external,
    })
}

fn check_one(page: &Page, link: &LinkRef, targets: &Targets, out: &mut Vec<Diagnostic>) {
    let url = link.url.as_str();
    // 分類は描画（yuzu-render の `UrlResolver::rewrite`）と同じ `classify_link`（R5）
    match classify_link(url) {
        // 外部参照は検査しない（http / https は呼び出し側で external へ回し済み）。
        // ディレクトリ風の相対参照（`guide/`）は配信の形に依るので静的には検査しない
        LinkTarget::Empty | LinkTarget::External | LinkTarget::Other => {}
        // 同一ページ内アンカー
        LinkTarget::Fragment(frag) => {
            if !has_anchor(page, frag) {
                push(
                    out,
                    page,
                    link,
                    rules::BROKEN_ANCHOR,
                    format!("このページに見出し `#{frag}` がありません"),
                );
            }
        }
        // ルート絶対（`/foo`）→ public/・ページ route・ビルド生成物に照合。
        // 著者がエンコード済みで書いた `/%E8%A8%AD…/` を生の route へ戻す
        LinkTarget::Absolute { rest, suffix } => {
            let rest = percent_decode(rest);
            check_absolute(
                page,
                link,
                &rest,
                LinkTarget::fragment(suffix),
                targets,
                out,
            );
        }
        LinkTarget::Page { path, suffix } => {
            check_relative_page(
                page,
                link,
                &path,
                LinkTarget::fragment(suffix),
                targets,
                out,
            );
        }
        LinkTarget::Asset { path, .. } => check_relative_asset(page, link, &path, targets, out),
    }
}

/// 相対 `.md` リンク → ページに照合
fn check_relative_page(
    page: &Page,
    link: &LinkRef,
    path: &str,
    frag: Option<&str>,
    targets: &Targets,
    out: &mut Vec<Diagnostic>,
) {
    let url = link.url.as_str();
    let dir = page.rel.parent().map(rel_to_slash).unwrap_or_default();
    let Some(resolved) = resolve_relative(&dir, path) else {
        push_outside_content(out, page, link, url);
        return;
    };
    match targets.by_rel.get(&resolved) {
        None => push(
            out,
            page,
            link,
            rules::BROKEN_LINK,
            format!("リンク先 `{url}` が見つかりません"),
        ),
        Some(target) if target.frontmatter.draft && !targets.include_drafts => {
            push(
                out,
                page,
                link,
                rules::BROKEN_LINK,
                format!("リンク先 `{resolved}` は draft のため公開サイトに含まれません"),
            );
        }
        Some(target) => {
            if let Some(frag) = frag
                && !has_anchor(target, frag)
            {
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

/// 拡張子付きの相対参照（画像等）→ content/ 内の実在を確認する
fn check_relative_asset(
    page: &Page,
    link: &LinkRef,
    path: &str,
    targets: &Targets,
    out: &mut Vec<Diagnostic>,
) {
    let url = link.url.as_str();
    let dir = page.rel.parent().map(rel_to_slash).unwrap_or_default();
    let Some(resolved) = resolve_relative(&dir, path) else {
        push_outside_content(out, page, link, url);
        return;
    };
    if !targets.assets.contains(&resolved) {
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

/// ルート絶対パスの照合
fn check_absolute(
    page: &Page,
    link: &LinkRef,
    rest: &str,
    frag: Option<&str>,
    targets: &Targets,
    out: &mut Vec<Diagnostic>,
) {
    // ビルドの生成物（今回出すものだけ。止めた llms-full.txt へのリンクは通さない）
    if SiteOutput::is_emitted_target(rest, targets.outputs) {
        return;
    }
    // public/ のファイル
    if targets
        .public_dir
        .is_some_and(|dir| dir.join(rest).is_file())
    {
        return;
    }
    // ページ route（末尾スラッシュの省略は許容）
    let target = targets.by_route.get(rest).or_else(|| {
        if rest.ends_with('/') {
            None
        } else {
            targets.by_route.get(format!("{rest}/").as_str())
        }
    });
    if let Some(target) = target {
        if let Some(frag) = frag
            && !has_anchor(target, frag)
        {
            push(
                out,
                page,
                link,
                rules::BROKEN_ANCHOR,
                format!("リンク先 `/{rest}` に見出し `#{frag}` がありません"),
            );
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
    out.push(Diagnostic::new(
        rule,
        page.rel.clone(),
        Some(link.span),
        message,
    ));
}

/// content の外を指す相対リンク（`..` がルートより上へ出る）。描画は href を書いたまま残す
fn push_outside_content(out: &mut Vec<Diagnostic>, page: &Page, link: &LinkRef, url: &str) {
    push(
        out,
        page,
        link,
        rules::LINK_OUTSIDE_CONTENT,
        format!(
            "リンク先 `{url}` は content/ の外を指しています。content/ の外のファイルはサイトに出ないので、\
             リンクは書いたまま残します（フル URL で書くか、ファイルを content/ の下へ移してください）"
        ),
    );
}
