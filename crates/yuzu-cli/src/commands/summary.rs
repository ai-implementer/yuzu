//! build の最後の報告（v0.20 Phase 91）: 何を出したかの 1 行の要約・利用者のファイルが生成した
//! ページを上書きしたときの警告・トップページが無いときの警告。
//!
//! 件数は出力の記録（[`OutputRecord`]。今回書いたもの。内容が同じで書き込みを省いたものも含む）
//! から数える = 「生成した Markdown」と「実際に配信されるファイル」を混同しない。出どころは
//! 書く側が申告した [`OutputKind`] で、パスの文字列から分類しない（別の一覧を持つと
//! v0.19.1 の R1 と同じずれが起きる）

use std::path::Path;

use yuzu_config::ResolvedConfig;
use yuzu_core::{GeneratedKind, IgnoreMatcher, OutputKind, OutputRecord, SiteModel, SiteOutput};

/// 警告と要約をログに出す（ビルドは成功のまま）
pub(crate) fn report(rc: &ResolvedConfig, site: &SiteModel, record: &OutputRecord) {
    let input = &rc.config.input.dir;
    if let Some(message) = page_overwrites(record, input) {
        tracing::warn!("{message}");
    }
    // ページが 0 件のときは warn_if_no_pages が知らせる（重ねて言わない）
    if site.pages.iter().any(|p| !p.is_generated())
        && let Some(message) = top_page_problem(
            record.files.contains_key("index.html"),
            &top_page_source(rc),
            input,
        )
    {
        tracing::warn!("{message}");
    }
    tracing::info!("{}", summary_line(site, record));
}

/// content 直下の `index.md`（`/` のページの原稿）が、ビルドしたページに無い理由
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum IndexMd {
    /// 置かれていない
    Missing,
    /// draft（`--drafts` のときだけビルドする）
    Draft,
    /// `input.ignore` で外れている
    Ignored,
}

/// トップページの原稿の状態（[`top_page_problem`] の入力）
#[derive(Debug, Clone, PartialEq, Eq)]
struct TopPageSource {
    index_md: IndexMd,
    /// content 直下の `README.md`（大文字小文字を問わない。実際のファイル名）
    readme: Option<String>,
}

/// ファイルシステムからトップページの原稿の状態を調べる。`index.md` があるのにビルドした
/// ページに無いなら、`input.ignore` に当たるか、当たらなければ draft
fn top_page_source(rc: &ResolvedConfig) -> TopPageSource {
    let index_md = if rc.content_dir.join("index.md").is_file() {
        let ignored = IgnoreMatcher::new(&rc.config.input.ignore)
            .is_ok_and(|m| m.is_match(Path::new("index.md")));
        if ignored {
            IndexMd::Ignored
        } else {
            IndexMd::Draft
        }
    } else {
        IndexMd::Missing
    };
    let readme = std::fs::read_dir(&rc.content_dir).ok().and_then(|entries| {
        entries
            .filter_map(Result::ok)
            .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .find(|name| name.eq_ignore_ascii_case("readme.md"))
    });
    TopPageSource { index_md, readme }
}

/// トップページ（dist の `index.html`）が無いときの警告。出力の記録で判定するので、
/// `public/index.html` や content の同伴の `index.html` だけでトップページを作るサイトは
/// 警告しない（ページと `public/` だけで判定すると誤って警告する。PR #53 の 3 回目のレビュー）。
/// draft を含む check では判定がずれるので、build と dev だけが出す
fn top_page_problem(has_index_html: bool, source: &TopPageSource, input: &str) -> Option<String> {
    if has_index_html {
        return None;
    }
    let reason = match source.index_md {
        IndexMd::Missing => format!("`{input}/index.md` がありません"),
        IndexMd::Draft => {
            format!("`{input}/index.md` は draft です（--drafts のときだけビルドします）")
        }
        IndexMd::Ignored => format!("`{input}/index.md` は input.ignore で外れています"),
    };
    let mut message =
        format!("トップページがありません（サイトの `/` を開くと 404 になります）。{reason}");
    if let Some(readme) = &source.readme {
        let stem = readme.strip_suffix(".md").unwrap_or(readme);
        message.push_str(&format!(
            "。`{input}/{readme}` は `/{stem}/` のページになります（トップページにするには \
             index.md へ名前を変えてください）"
        ));
    }
    Some(message)
}

/// 利用者のファイル（content の同伴ファイル・`public/`）が、生成したページ（トップページを含む）・
/// リダイレクト・原稿の `.md` を上書きしたときの警告。docs が案内している 404.html・llms・
/// sitemap 等のサイト単位の生成物の置き換えは警告せず、要約に出す
fn page_overwrites(record: &OutputRecord, input: &str) -> Option<String> {
    let items: Vec<String> = record
        .overwrites
        .iter()
        .filter(|o| {
            matches!(
                o.generated,
                OutputKind::Page | OutputKind::Redirect | OutputKind::PageMd
            )
        })
        .map(|o| format!("{} ← {}", o.rel, user_file(o.by, &o.rel, input)))
        .collect();
    (!items.is_empty()).then(|| {
        format!(
            "利用者のファイルが、生成したページを上書きしました（配信されるのは利用者のファイルです）: {}",
            items.join("・")
        )
    })
}

/// 上書きした利用者のファイルのプロジェクトルート相対パス（`public/` はルート直下に固定）
fn user_file(kind: OutputKind, rel: &str, input: &str) -> String {
    match kind {
        OutputKind::Public => format!("public/{rel}"),
        OutputKind::ContentAsset => format!("{input}/{rel}"),
        _ => rel.to_string(),
    }
}

/// 何を出したかの 1 行。build のページ数・検索インデックスのページ数・check のページ数が
/// 食い違って見える理由（用語集と検索結果ページの合成ページ）の答えにもする
fn summary_line(site: &SiteModel, record: &OutputRecord) -> String {
    let mut parts = Vec::new();

    let sources = site.pages.iter().filter(|p| !p.is_generated()).count();
    let mut breakdown = vec![format!("原稿 {sources}")];
    for (kind, label) in [
        (GeneratedKind::Glossary, "用語集"),
        (GeneratedKind::Search, "検索結果ページ"),
    ] {
        if site.pages.iter().any(|p| p.generated == Some(kind)) {
            breakdown.push(label.to_string());
        }
    }
    // 上書きされたページは記録の上では利用者のファイルなので、ページ数が内訳より少なくなる
    // （警告の行を見落としても数が合うように添える）
    let overwritten = record
        .overwrites
        .iter()
        .filter(|o| o.generated == OutputKind::Page)
        .count();
    if overwritten > 0 {
        breakdown.push(format!("{overwritten} 件は利用者のファイルで上書き"));
    }
    parts.push(format!(
        "ページ {}（{}）",
        record.count(OutputKind::Page),
        breakdown.join("・")
    ));

    let redirects = record.count(OutputKind::Redirect);
    if redirects > 0 {
        parts.push(format!("リダイレクト {redirects}"));
    }
    let page_md = record.count(OutputKind::PageMd);
    if page_md > 0 {
        parts.push(format!("原稿の .md {page_md}"));
    }
    for output in [SiteOutput::LlmsTxt, SiteOutput::LlmsFullTxt] {
        if record.files.get(output.path()) == Some(&OutputKind::Site(output)) {
            parts.push(output.path().to_string());
        }
    }
    let search = OutputKind::Site(SiteOutput::SearchIndex);
    if record.files.values().any(|k| *k == search) {
        let indexed = site.pages.iter().filter(|p| p.in_search_index()).count();
        parts.push(format!("検索 {indexed} ページ"));
    }
    let assets = record.count(OutputKind::ContentAsset);
    if assets > 0 {
        parts.push(format!("同伴ファイル {assets}"));
    }
    let public = record.count(OutputKind::Public);
    if public > 0 {
        parts.push(format!("public {public}"));
    }

    let mut line = format!("出力: {}", parts.join("・"));
    // サイト単位の生成物を利用者のファイルで置き換えたもの（docs が案内している使い方）
    let replaced: Vec<&str> = record
        .overwrites
        .iter()
        .filter(|o| matches!(o.generated, OutputKind::Site(_)))
        .map(|o| o.rel.as_str())
        .collect();
    if !replaced.is_empty() {
        line.push_str(&format!(
            "（利用者のファイルを優先: {}）",
            replaced.join("・")
        ));
    }
    line
}

#[cfg(test)]
mod tests {
    use super::*;
    use yuzu_core::Overwrite;

    fn source(index_md: IndexMd, readme: Option<&str>) -> TopPageSource {
        TopPageSource {
            index_md,
            readme: readme.map(str::to_string),
        }
    }

    /// PR #53 の協議で導入派が試した組み合わせ（0.19.1）と、同伴の `index.html`
    #[test]
    fn トップページの警告は出力の記録で判定する() {
        let warn = |has, src: &TopPageSource| top_page_problem(has, src, "content");
        // 1. index.md をビルドした
        assert_eq!(warn(true, &source(IndexMd::Missing, None)), None);
        // 2. public/index.html だけ・3. 同伴の content/index.html だけ（記録に index.html がある）
        assert_eq!(warn(true, &source(IndexMd::Missing, None)), None);
        // 4. index.md と README.md の両方があってビルドした
        assert_eq!(
            warn(true, &source(IndexMd::Missing, Some("README.md"))),
            None
        );

        // 5. 何も無い
        let m = warn(false, &source(IndexMd::Missing, None)).unwrap();
        assert!(m.contains("`content/index.md` がありません"), "{m}");
        assert!(!m.contains("README"), "{m}");
        // 6. README.md だけ（`input.dir = "docs"` の既存のリポジトリ。`/README/` になる）
        let m = warn(false, &source(IndexMd::Missing, Some("README.md"))).unwrap();
        assert!(
            m.contains("`content/README.md` は `/README/` のページ"),
            "{m}"
        );
        // 小文字のファイル名もそのまま案内する
        let m = warn(false, &source(IndexMd::Missing, Some("readme.md"))).unwrap();
        assert!(m.contains("`/readme/`"), "{m}");
        // 7. index.md が draft
        let m = warn(false, &source(IndexMd::Draft, None)).unwrap();
        assert!(m.contains("draft です"), "{m}");
        // 8. index.md が input.ignore で外れている
        let m = warn(false, &source(IndexMd::Ignored, Some("README.md"))).unwrap();
        assert!(m.contains("input.ignore で外れています"), "{m}");
        assert!(m.contains("README.md"), "{m}");
    }

    fn record(files: &[(&str, OutputKind)], overwrites: Vec<Overwrite>) -> OutputRecord {
        OutputRecord {
            files: files.iter().map(|(r, k)| (r.to_string(), *k)).collect(),
            overwrites,
        }
    }

    #[test]
    fn ページの上書きだけを警告し_サイト単位の置き換えは要約に出す() {
        let rec = record(
            &[
                ("index.html", OutputKind::Public),
                ("guide/index.html", OutputKind::ContentAsset),
                ("404.html", OutputKind::Public),
            ],
            vec![
                Overwrite {
                    rel: "index.html".into(),
                    generated: OutputKind::Page,
                    by: OutputKind::Public,
                },
                Overwrite {
                    rel: "guide/index.html".into(),
                    generated: OutputKind::Page,
                    by: OutputKind::ContentAsset,
                },
                Overwrite {
                    rel: "404.html".into(),
                    generated: OutputKind::Site(SiteOutput::NotFound),
                    by: OutputKind::Public,
                },
            ],
        );
        let m = page_overwrites(&rec, "docs").unwrap();
        assert!(
            m.ends_with("index.html ← public/index.html・guide/index.html ← docs/guide/index.html"),
            "{m}"
        );
        assert!(!m.contains("404"), "{m}");
        let site = SiteModel {
            pages: Vec::new(),
            nav: Vec::new(),
        };
        let line = summary_line(&site, &rec);
        assert!(
            line.ends_with("（利用者のファイルを優先: 404.html）"),
            "{line}"
        );
        // 上書きされた 2 ページは記録の上では利用者のファイル（ページ 0）。内訳に添えて数を合わせる
        assert!(
            line.starts_with("出力: ページ 0（原稿 0・2 件は利用者のファイルで上書き）"),
            "{line}"
        );

        assert_eq!(page_overwrites(&record(&[], Vec::new()), "docs"), None);
    }
}
