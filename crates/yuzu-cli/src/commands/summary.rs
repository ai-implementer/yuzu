//! build の最後の報告（v0.20 Phase 91）: 何を出したかの 1 行の要約・利用者のファイルが生成した
//! ページを上書きしたときの警告・トップページが無いときの警告。
//!
//! 件数は出力の記録（[`OutputRecord`]。今回書いたもの。内容が同じで書き込みを省いたものも含む）
//! から数える = 「生成した Markdown」と「実際に配信されるファイル」を混同しない。出どころは
//! 書く側が申告した [`OutputKind`] で、パスの文字列から分類しない（別の一覧を持つと
//! v0.19.1 の R1 と同じずれが起きる）

use std::path::Path;

use yuzu_config::ResolvedConfig;
use yuzu_core::{GeneratedKind, MarkdownCensus, OutputKind, OutputRecord, SiteModel, SiteOutput};

/// 警告と要約をログに出す（ビルドは成功のまま）
pub(crate) fn report(rc: &ResolvedConfig, site: &SiteModel, record: &OutputRecord) {
    let input = &rc.config.input.dir;
    if let Some(message) = page_overwrites(record, input) {
        tracing::warn!("{message}");
    }
    // ページが 0 件のときは warn_if_no_pages が知らせる（重ねて言わない）
    if site.pages.iter().any(|p| !p.is_generated())
        && let Some(message) = top_page_problem(rc, site, record)
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
    /// シンボリックリンク（走査はリンクを辿らないので読まない）
    Symlink,
}

/// トップページの原稿の状態（[`top_page_message`] の入力）
#[derive(Debug, Clone, PartialEq, Eq)]
struct TopPageSource {
    index_md: IndexMd,
    /// content 直下で、トップページのつもりで置かれやすい名前（`README.md`・大文字の
    /// `Index.md` 等）のままビルドしたページ（原稿のファイル名と route）
    misnamed: Vec<(String, String)>,
}

/// トップページの原稿の状態を、ページの読み込みと同じ走査（`census`）とビルドしたページから
/// 決める。ファイルシステムを別に調べない（走査の規則とずれて、シンボリックリンクの `index.md`
/// を draft と、ignore や draft で外れた `README.md` をページになると案内していた。PR #69 の
/// レビュー指摘）
fn top_page_source(census: &MarkdownCensus, site: &SiteModel) -> TopPageSource {
    let index = Path::new("index.md");
    let index_md = if census.ignored.iter().any(|rel| rel == index) {
        IndexMd::Ignored
    } else if census.symlinks.iter().any(|rel| rel == index) {
        IndexMd::Symlink
    } else if census.loaded.iter().any(|rel| rel == index) {
        // 読み込んだのにビルドしたページに無い = draft（build が外すのは draft だけ）
        IndexMd::Draft
    } else {
        IndexMd::Missing
    };
    let misnamed = site
        .pages
        .iter()
        .filter(|p| !p.is_generated() && p.rel.parent() == Some(Path::new("")))
        .filter_map(|p| {
            let name = p.rel.file_name()?.to_str()?;
            let looks_top =
                name.eq_ignore_ascii_case("index.md") || name.eq_ignore_ascii_case("readme.md");
            (looks_top && name != "index.md").then(|| (name.to_string(), p.route.clone()))
        })
        .collect();
    TopPageSource { index_md, misnamed }
}

/// トップページ（dist の `index.html`）が無いときの警告。出力の記録で判定するので、
/// `public/index.html` や content の同伴の `index.html` だけでトップページを作るサイトは
/// 警告しない（ページと `public/` だけで判定すると誤って警告する。PR #53 の 3 回目のレビュー）。
/// draft を含む check では判定がずれるので、build と dev だけが出す
fn top_page_problem(
    rc: &ResolvedConfig,
    site: &SiteModel,
    record: &OutputRecord,
) -> Option<String> {
    if record.files.contains_key("index.html") {
        return None;
    }
    // 走査が失敗するのは、直前のページの読み込みの後に原稿のディレクトリが変わったときだけ。
    // そのときは理由を添えない
    let source = yuzu_core::markdown_census(&rc.content_dir, &rc.config.input.ignore)
        .ok()
        .map(|census| top_page_source(&census, site));
    Some(top_page_message(source.as_ref(), &rc.config.input.dir))
}

/// [`top_page_problem`] の文言（`source` が無ければ理由を添えない）
fn top_page_message(source: Option<&TopPageSource>, input: &str) -> String {
    let mut message =
        "トップページがありません（サイトの `/` を開くと 404 になります）".to_string();
    let Some(source) = source else {
        return message;
    };
    let reason = match source.index_md {
        IndexMd::Missing => format!("`{input}/index.md` がありません"),
        IndexMd::Draft => {
            format!("`{input}/index.md` は draft です（--drafts のときだけビルドします）")
        }
        IndexMd::Ignored => format!("`{input}/index.md` は input.ignore で外れています"),
        IndexMd::Symlink => format!(
            "`{input}/index.md` はシンボリックリンクなので読みません（実体のファイルを置いてください）"
        ),
    };
    message.push_str(&format!("。{reason}"));
    if !source.misnamed.is_empty() {
        let pages: Vec<String> = source
            .misnamed
            .iter()
            .map(|(name, route)| format!("`{input}/{name}` は `/{route}`"))
            .collect();
        // index.md が draft・ignore 等で既にあるなら、名前を変えると重なるので案内しない
        let rename = if source.index_md == IndexMd::Missing {
            "（トップページにするには index.md へ名前を変えてください）"
        } else {
            ""
        };
        message.push_str(&format!(
            "。{} のページになります{rename}",
            pages.join("・")
        ));
    }
    message
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
        .map(|o| format!("{} ← {}", o.rel, user_file(o.by, &o.by_rel, input)))
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

    fn source(index_md: IndexMd, misnamed: &[(&str, &str)]) -> TopPageSource {
        TopPageSource {
            index_md,
            misnamed: misnamed
                .iter()
                .map(|(name, route)| (name.to_string(), route.to_string()))
                .collect(),
        }
    }

    /// PR #53 の協議で導入派が試した組み合わせ（0.19.1）のうち、トップページが無いもの。
    /// トップページがある組み合わせは下のファイルシステムを使うテスト
    #[test]
    fn トップページの警告に理由を添える() {
        let warn = |src: &TopPageSource| top_page_message(Some(src), "content");
        // 5. 何も無い
        let m = warn(&source(IndexMd::Missing, &[]));
        assert!(m.contains("`content/index.md` がありません"), "{m}");
        assert!(!m.contains("README"), "{m}");
        // 6. README.md だけ（`input.dir = "docs"` の既存のリポジトリ。`/README/` になる）
        let m = warn(&source(IndexMd::Missing, &[("README.md", "README/")]));
        assert!(
            m.contains("`content/README.md` は `/README/` のページ"),
            "{m}"
        );
        // 大文字の Index.md（`/Index/` になる）と README.md の両方
        let m = warn(&source(
            IndexMd::Missing,
            &[("Index.md", "Index/"), ("readme.md", "readme/")],
        ));
        assert!(
            m.contains(
                "`content/Index.md` は `/Index/`・`content/readme.md` は `/readme/` のページ"
            ),
            "{m}"
        );
        // 7. index.md が draft
        let m = warn(&source(IndexMd::Draft, &[]));
        assert!(m.contains("draft です"), "{m}");
        // 8. index.md が input.ignore で外れている
        let m = warn(&source(IndexMd::Ignored, &[("README.md", "README/")]));
        assert!(m.contains("input.ignore で外れています"), "{m}");
        assert!(
            m.contains("`content/README.md` は `/README/` のページになります"),
            "{m}"
        );
        // index.md は既にあるので、README.md の名前を変える案内はしない
        assert!(!m.contains("名前を変えて"), "{m}");
        // index.md がシンボリックリンク（走査はリンクを辿らない）
        let m = warn(&source(IndexMd::Symlink, &[]));
        assert!(m.contains("シンボリックリンクなので読みません"), "{m}");
        assert!(!m.contains("draft"), "{m}");
        // 原稿を調べられなければ理由を添えない
        assert_eq!(
            top_page_message(None, "content"),
            "トップページがありません（サイトの `/` を開くと 404 になります）"
        );
    }

    /// 理由と README の案内は、ページの読み込みと同じ走査とビルドしたページから決める。
    /// ファイルシステムを別に調べると、シンボリックリンクの index.md を draft と、外れた
    /// README.md をページになると案内していた（PR #69 のレビュー指摘）
    #[test]
    fn トップページの警告は走査とビルドしたページから決める() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().canonicalize().unwrap();
        let write = |rel: &str, text: &str| {
            let path = root.join(rel);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, text).unwrap();
        };
        let warn = |files: &[(&str, OutputKind)]| {
            let rc = yuzu_config::load(&root).unwrap();
            let md_opts = yuzu_render::markdown_options(&rc.config);
            let site =
                yuzu_core::build_site_model(&rc.content_dir, &rc.config.input.ignore, &md_opts)
                    .unwrap();
            top_page_problem(&rc, &site, &record(files, Vec::new()))
        };
        let page = [("guide/index.html", OutputKind::Page)];
        write("yuzu.toml", "[input]\nignore = [\"notes/**\"]\n");
        write("content/guide.md", "# ガイド\n");

        // README.md がページになる
        write("content/README.md", "# リポジトリ\n");
        let m = warn(&page).unwrap();
        assert!(m.contains("`content/index.md` がありません"), "{m}");
        assert!(
            m.contains("`content/README.md` は `/README/` のページ"),
            "{m}"
        );
        // README.md が draft なら、ページにならないので案内しない
        write(
            "content/README.md",
            "---\ndraft: true\n---\n\n# リポジトリ\n",
        );
        let m = warn(&page).unwrap();
        assert!(!m.contains("README"), "{m}");
        // index.md が draft
        write("content/index.md", "---\ndraft: true\n---\n\n# トップ\n");
        let m = warn(&page).unwrap();
        assert!(m.contains("`content/index.md` は draft です"), "{m}");
        // 1〜4. 記録に index.html があれば（index.md をビルドした・public/index.html・同伴の
        // content/index.html）警告しない
        assert_eq!(warn(&[("index.html", OutputKind::Page)]), None);
        assert_eq!(warn(&[("index.html", OutputKind::Public)]), None);
        assert_eq!(warn(&[("index.html", OutputKind::ContentAsset)]), None);

        // README.md・index.md が input.ignore で外れている
        std::fs::remove_file(root.join("content/index.md")).unwrap();
        write(
            "yuzu.toml",
            "[input]\nignore = [\"notes/**\", \"README.md\", \"index.md\"]\n",
        );
        write("content/index.md", "# トップ\n");
        write("content/README.md", "# リポジトリ\n");
        let m = warn(&page).unwrap();
        assert!(
            m.contains("`content/index.md` は input.ignore で外れています"),
            "{m}"
        );
        assert!(!m.contains("README"), "{m}");

        // index.md がリポジトリの README.md へのシンボリックリンク
        #[cfg(unix)]
        {
            write("yuzu.toml", "");
            std::fs::remove_file(root.join("content/index.md")).unwrap();
            std::fs::remove_file(root.join("content/README.md")).unwrap();
            write("README.md", "# リポジトリ\n");
            std::os::unix::fs::symlink(root.join("README.md"), root.join("content/index.md"))
                .unwrap();
            let m = warn(&page).unwrap();
            assert!(
                m.contains("`content/index.md` はシンボリックリンクなので読みません"),
                "{m}"
            );
            assert!(!m.contains("draft"), "{m}");
        }
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
                // 大文字小文字を区別しない FS では、大文字小文字だけ違う利用者のファイルが
                // 同じファイルに書かれる。警告には利用者のファイルの実際の名前を出す
                Overwrite {
                    rel: "index.html".into(),
                    generated: OutputKind::Page,
                    by: OutputKind::Public,
                    by_rel: "INDEX.html".into(),
                },
                Overwrite {
                    rel: "guide/index.html".into(),
                    generated: OutputKind::Page,
                    by: OutputKind::ContentAsset,
                    by_rel: "guide/index.html".into(),
                },
                Overwrite {
                    rel: "404.html".into(),
                    generated: OutputKind::Site(SiteOutput::NotFound),
                    by: OutputKind::Public,
                    by_rel: "404.html".into(),
                },
            ],
        );
        let m = page_overwrites(&rec, "docs").unwrap();
        assert!(
            m.ends_with("index.html ← public/INDEX.html・guide/index.html ← docs/guide/index.html"),
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
