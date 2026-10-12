//! ビルドパイプライン: clean → ページ HTML（rayon 並列） → テーマアセット →
//! syntect CSS → public パススルー → build_id（`build --watch` のときだけ）
//!
//! ページ生成はページ間に依存が無いため並列化している（Phase 32）。
//! 集約出力（nav は各ページに埋まるが構築は事前・llms / 404 / アセット）は
//! 直列のまま = インクリメンタルビルドの層構造は不変

use std::collections::BTreeSet;
use std::fs;

use minijinja::context;
use rayon::prelude::*;

use yuzu_config::ResolvedConfig;
use yuzu_core::{
    BuildCache, CachedBody, GeneratedKind, OutputSwitches, OutputTracker, SiteModel, SiteOutput,
};

use crate::assets;
use crate::context::{NavCtx, NavOrder, NavTrails, PageCtx, SiteCtx, build_breadcrumbs};
use crate::error::RenderError;
use crate::shared::RenderShared;
use crate::urls::UrlResolver;

/// サイト単位の生成物（[`SiteOutput`]）を今回出すかの入力を、設定と実行の文脈から作る。
///
/// 書き出し（[`render_site`]）・止めた生成物の掃除（[`unpublished_outputs`]）・
/// `yuzu check` のリンクの有効ターゲットが、すべてこの値と [`SiteOutput::emitted`] を通る
/// （10-12 レビューの R1。3 か所が別々の一覧を持っていて食い違っていた）
pub fn output_switches(rc: &ResolvedConfig, live_reload: LiveReloadMode) -> OutputSwitches {
    let cfg = &rc.config;
    OutputSwitches {
        llms: cfg.llms.enabled,
        llms_full: cfg.llms.full,
        sitemap: crate::urls::is_absolute_base(&rc.base_url),
        highlight: cfg.markdown.highlight.enabled,
        search: cfg.search.enabled,
        // build_id を読むのは `build --watch` の autorefresh.js だけ。通常の build で書くと
        // dist が毎回 1 ファイル変わり、ビルド時刻も公開される
        build_id: live_reload == LiveReloadMode::Poll,
    }
}

/// 設定やページの指定で今回**出さない**生成物（[`unpublished_outputs`] の結果）
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Unpublished {
    /// dist 相対のファイル（原稿の `.md`・llms.txt・sitemap.xml・syntect.css・build_id 等）
    pub files: BTreeSet<String>,
    /// 中のファイルを列挙して消すディレクトリ（検索インデックスの `_search`）。
    /// ディレクトリごとは消さない（public/ に置いた利用者のファイルを残すため）
    pub dirs: Vec<&'static str>,
}

/// 設定やページの指定で**出さないことにした**生成物（Phase 83。R1 で [`SiteOutput`] に一本化）。
///
/// 書き出し側と同じ判断を裏返して作る: 原稿の `.md`（`Page::emits_page_md`）と、
/// [`SiteOutput::emitted`] が false のサイト単位の生成物。cli はこれを前回の出力の代わりに
/// 孤児掃除へ渡し、今回書かなかったものを消す。
/// 出力マニフェストが無いとき（`--force`・`.yuzu` の削除の後）は通常の孤児掃除が働かず、
/// `output.clean = false` だと dist も作り直さないので、止めた原稿の `.md`（frontmatter 込み）や
/// syntect.css・build_id が残り続けていた（PR #42 のレビュー指摘・10-12 レビューの R1）。
/// public/ に同じパスのファイルを置いていれば今回書き出しているので消さない
/// （判定は呼び出し側の「今回書いたもの」との差）
pub fn unpublished_outputs(
    rc: &ResolvedConfig,
    site: &SiteModel,
    live_reload: LiveReloadMode,
) -> Unpublished {
    let cfg = &rc.config;
    let switches = output_switches(rc, live_reload);
    let mut files: BTreeSet<String> = site
        .pages
        .iter()
        .filter(|page| !page.emits_page_md(cfg.llms.page_md))
        .map(|page| page.md_rel_path())
        .collect();
    let mut dirs = Vec::new();
    for output in SiteOutput::ALL.iter().filter(|o| !o.emitted(&switches)) {
        if output.is_dir() {
            dirs.push(output.path());
        } else {
            files.insert(output.path().to_string());
        }
    }
    Unpublished { files, dirs }
}

/// ページに注入するライブリロード方式
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LiveReloadMode {
    /// 注入なし（通常ビルド / `dev.live_reload: false`）
    #[default]
    None,
    /// build_id ポーリング（`build --watch`）
    Poll,
    /// WebSocket（`yuzu dev`）
    Ws,
}

/// インクリメンタルビルドの文脈（すべて None = 従来のフルビルドと同一動作）
#[derive(Default)]
pub struct RenderCtx<'a> {
    /// ページ派生物のキャッシュ（本文 HTML・llms 正規化 md）
    pub cache: Option<&'a BuildCache>,
    /// 出力トラッキング（孤児掃除マニフェスト用）。Some なら dist の clean をしない
    pub outputs: Option<&'a OutputTracker>,
    /// セッション共有の重い状態（テンプレート Env・ハイライタ・syntect CSS）
    pub shared: Option<&'a RenderShared>,
}

/// レンダリング一式の入力
pub struct RenderParams<'a> {
    pub config: &'a ResolvedConfig,
    pub site: &'a SiteModel,
    pub live_reload: LiveReloadMode,
    pub ctx: RenderCtx<'a>,
    /// ページ（content 相対 `/` 区切り）→ 最終コミット日（YYYY-MM-DD）。
    /// git の実行は cli 層の責務で、render はデータを受け取るだけ。
    /// None = 最終更新日を表示しない
    pub git_dates: Option<&'a std::collections::HashMap<String, String>>,
}

/// ページ URL とエイリアスの妥当性を検証する（書き出し前の門番）。
///
/// - 同じ route のページは同じ `index.html` へ並列に書き込むため勝者が非決定になる
/// - `#` 等を含むファイル名は nav・検索・llms のすべてで壊れたリンクになる
/// - 不正・衝突するエイリアスは実ページや他のリダイレクトを上書きする
///
/// **cli は破壊的な clean より前にこれを呼ぶこと**（検証エラーで既存の dist を
/// 失わないため）。`render_site` も冒頭で呼ぶので、呼び忘れても出力は壊れない
pub fn validate_pages(site: &SiteModel, rc: &ResolvedConfig) -> Result<(), RenderError> {
    let cfg = &rc.config;
    let md_opts = crate::markdown_options(cfg);

    let route_diags = yuzu_core::validate_routes(&site.pages);
    if let Some(first) = route_diags.first() {
        return Err(RenderError::InvalidRoutes {
            count: route_diags.len(),
            first: format!("{}: {}", first.rel.display(), first.message),
        });
    }

    let alias_diags = yuzu_core::validate_aliases(&site.pages, &md_opts);
    if let Some(first) = alias_diags.first() {
        return Err(RenderError::InvalidAliases {
            count: alias_diags.len(),
            first: format!("{}: {}", first.rel.display(), first.message),
        });
    }
    Ok(())
}

/// サイト全体を `dist/` に書き出す
pub fn render_site(params: &RenderParams) -> Result<(), RenderError> {
    let rc = params.config;
    let cfg = &rc.config;
    let output_dir = &rc.output_dir;
    let ctx = &params.ctx;

    let md_opts = crate::markdown_options(cfg);
    // サイト単位の生成物を出すかの判断（SiteOutput が唯一の定義。掃除と check も同じ値を使う）
    let switches = output_switches(rc, params.live_reload);

    // ページ URL とエイリアスの妥当性（cli は破壊的な clean より前に呼ぶが、
    // ライブラリ直接利用と呼び忘れに備えてここでも自衛する）
    validate_pages(params.site, rc)?;
    // ページ内 TOC の表示レベル。cli は設定の読み込み直後に検証済み。不正な値で
    // 既定へ黙って縮退しない（clean より前に止める = 既存の dist を失わない）
    let toc_levels = crate::context::parse_toc_levels(&cfg.theme.toc.levels).ok_or_else(|| {
        RenderError::InvalidTocLevels {
            value: cfg.theme.toc.levels.clone(),
            reason: crate::context::toc_levels_problem(&cfg.theme.toc.levels).unwrap_or_default(),
        }
    })?;

    // 出力先までの経路にシンボリックリンクが無いことを**毎ビルド**確認する。
    // 削除経路（下の remove_dir_all_under）は clean 無効・インクリメンタルでは
    // 走らないため、ここを通さないと書き込みだけがリンク先へ素通りする
    yuzu_core::output::ensure_no_symlink_under(&rc.root, output_dir)
        .map_err(RenderError::io(output_dir))?;

    // インクリメンタル時（outputs あり）は dist を残し、孤児掃除は cli 側が行う
    if cfg.output.clean && ctx.outputs.is_none() {
        yuzu_core::output::remove_dir_all_under(&rc.root, output_dir)
            .map_err(RenderError::io(output_dir))?;
    }
    fs::create_dir_all(output_dir).map_err(RenderError::io(output_dir))?;

    // セッション共有があれば再利用、なければこのビルド限りで構築（従来コスト）
    let local_shared = match ctx.shared {
        Some(_) => None,
        None => Some(RenderShared::new(rc)?),
    };
    let shared = ctx.shared.unwrap_or_else(|| {
        local_shared
            .as_ref()
            .expect("shared 未指定時は local_shared を構築済み")
    });
    let template = shared.env.get_template("page.jinja")?;
    // 検索結果ページ（`search.page`）は専用テンプレートで描く。
    // 居るときだけ解決する = 機能未使用のプロジェクトはテンプレート追加の影響を受けない
    let search_template = params
        .site
        .pages
        .iter()
        .any(|p| p.generated == Some(GeneratedKind::Search))
        .then(|| shared.env.get_template("search.jinja"))
        .transpose()?;
    let resolver = UrlResolver::new(&rc.base_url, params.site);
    // ドロップダウンの「すべての結果を見る」行の遷移先（base.jinja が全ページに配る）。
    // 未設定なら None = data-search-page 属性ごと出ない（既存出力とバイト同一）
    let search_page_url = params
        .site
        .pages
        .iter()
        .find(|p| p.generated == Some(GeneratedKind::Search))
        .map(|p| resolver.page_url(&p.route));
    // 結果ページで 1 回に表示する件数。0 は 1 へ丸める（shard 設定と同じ流儀）
    let search_page_size = cfg.search.page_size.max(1);
    // 前/次リンクの導出元（サイドバー表示順のフラット列）。全ページで共通
    let nav_order = NavOrder::new(&params.site.nav);
    // route → 祖先チェーンの前計算（サイドバーの open 判定とパンくずが共有。
    // ページごとの全ツリー DFS を無くす）
    let nav_trails = NavTrails::new(&params.site.nav);
    // og:image は絶対 URL が必須。フル URL 指定はそのまま、パス指定は base がフル URL の
    // ときだけ解決する（ローカルビルドでは出ないのが普通なので debug ログに留める）
    let image_url = cfg.site.image.as_deref().and_then(|p| {
        if p.contains("://") {
            Some(p.to_string())
        } else if resolver.is_absolute_base() {
            Some(resolver.public_url(p))
        } else {
            tracing::debug!("baseUrl がフル URL ではないため og:image（site.image）は出力しない");
            None
        }
    });
    let site_ctx = SiteCtx {
        title: &cfg.site.title,
        description: cfg.site.description.as_deref(),
        lang: &cfg.site.lang,
        logo_url: cfg.site.logo.as_deref().map(|p| resolver.public_url(p)),
        image_url,
        locale: crate::context::og_locale(&cfg.site.lang),
        generator: crate::context::GENERATOR,
    };
    // theme.css_vars / css_vars_dark → head に注入する CSS 変数上書き（全ページ共通）
    let theme_css_vars =
        crate::css::generate_theme_var_overrides(&cfg.theme.css_vars, &cfg.theme.css_vars_dark);

    // ページ生成の並列ループ。共有物は読み取り専用（Env / syntect / resolver /
    // nav / 日付マップ）か内部 Mutex の `&self` API（BuildCache / OutputTracker）。
    // ページ内状態は PageCodeRenderer がページローカルに持つ（`Cell` が `!Sync`
    // なので誤って共有するとコンパイルエラーになる）。出力はページごとに別
    // ファイルで、マニフェスト記録は BTreeSet のため書き込み順に依らず決定的
    params
        .site
        .pages
        .par_iter()
        .try_for_each(|page| -> Result<(), RenderError> {
            // 本文 HTML はキャッシュヒットなら comrak パースごとスキップする。
            // 進捗の 1 ページ 1 行はここで出す（並列ループ内なので行順は非決定。
            // 成果物のバイト同一性には関与しない）
            let (body, mermaid_fallback, reading) =
                match ctx.cache.and_then(|c| c.body(&page.rel, &page.source)) {
                    Some(cached) => {
                        tracing::info!(page = %page.rel.display(), "レンダ（キャッシュ）");
                        (cached.html, cached.mermaid_fallback, cached.reading)
                    }
                    None => {
                        tracing::info!(page = %page.rel.display(), "レンダ");
                        let renderer = shared.highlighter.page_renderer().for_page(&page.rel);
                        let rendered = yuzu_core::render_body_html(
                            page,
                            &md_opts,
                            &renderer,
                            &resolver,
                            Some(&rc.root),
                        )?;
                        let fallback = renderer.mermaid_fallback_occurred();
                        // 外部ファイル参照を使ったページはキャッシュしない: ページ source が
                        // 変わらなくても参照先の変更を次ビルドで反映するため、毎回レンダリング
                        // する。コード引用・仕様の file: は renderer 側のフラグ、Markdown 断片
                        // （```include）は core 展開なので used_fragment 側が担う（片方だけ
                        // 見ると v15 の「誤ってキャッシュに載る」事故が再演する）
                        let cacheable = !renderer.external_deps_used() && !rendered.used_fragment;
                        if let (Some(cache), true) = (ctx.cache, cacheable) {
                            cache.store_body(
                                &page.rel,
                                &page.source,
                                CachedBody {
                                    html: rendered.html.clone(),
                                    mermaid_fallback: fallback,
                                    reading: rendered.reading,
                                },
                            );
                        }
                        (rendered.html, fallback, rendered.reading)
                    }
                };
            // 「このページで mermaid.js を読み込むか」。client は従来どおり常に読み、
            // ssr はフォールバック（未対応図種等）が発生したページだけ読む
            let mermaid_js_needed = cfg.markdown.mermaid.enabled
                && match cfg.markdown.mermaid.backend {
                    yuzu_config::MermaidBackend::Client => true,
                    yuzu_config::MermaidBackend::Ssr => mermaid_fallback,
                };
            // 「このページで KaTeX を読み込むか」。comrak の数式出力は必ず
            // data-math-style="…" 属性を持つ。本文テキスト中の同じ文字列は comrak が
            // `"` を &quot; にエスケープするため、引用符込みのこの判定は誤検出しない
            let math_needed = cfg.markdown.math.enabled && body.contains("data-math-style=\"");
            // git 連携メタ（rel を / 区切りへ正規化してキーにする）
            let rel_key = page
                .rel
                .iter()
                .map(|c| c.to_string_lossy())
                .collect::<Vec<_>>()
                .join("/");
            let last_updated = params
                .git_dates
                .and_then(|dates| dates.get(&rel_key))
                .cloned();
            // 合成ページ（用語集）は実ファイルが無いので編集リンクを出さない
            // （last_updated は git_dates に無いので自動的に None になる）。
            // `{path}` は外部 URL のパスに埋まるので route と同じ規則でエンコードする
            let edit_url = cfg
                .git
                .edit_url
                .as_ref()
                .filter(|_| !page.is_generated())
                .map(|tpl| tpl.replace("{path}", &yuzu_core::urlpath::encode_path(&rel_key)));
            // 検索結果ページだけ専用テンプレート。コンテキストは共通の 1 個
            // （search_page_size は search.jinja だけが使い、page.jinja は無視する）
            let tpl = match page.generated {
                Some(GeneratedKind::Search) => search_template
                    .as_ref()
                    .expect("Search ページが pages に居るなら解決済み"),
                _ => &template,
            };
            let html = tpl.render(context! {
                site => site_ctx,
                page => PageCtx::new(
                    page,
                    &body,
                    &resolver,
                    last_updated,
                    edit_url,
                    &toc_levels,
                    cfg.theme.reading_time.then_some(reading),
                )
                .with_page_md(page.emits_page_md(cfg.llms.page_md), page, &resolver),
                nav => NavCtx::build(&params.site.nav, nav_trails.trail(&page.route), &resolver),
                nav_collapse => cfg.nav.collapse,
                pager => nav_order.pager(&page.route, &resolver),
                breadcrumbs => build_breadcrumbs(&nav_trails, &page.route, &resolver),
                base_url => resolver.base(),
                asset_url => resolver.asset_url(),
                live_reload_poll => params.live_reload == LiveReloadMode::Poll,
                live_reload_ws => params.live_reload == LiveReloadMode::Ws,
                mermaid_enabled => mermaid_js_needed,
                math_enabled => math_needed,
                highlight_enabled => cfg.markdown.highlight.enabled,
                dark_enabled => cfg.theme.dark == yuzu_config::DarkMode::Toggle,
                theme_mode => cfg.theme.dark.as_str(),
                search_enabled => cfg.search.enabled,
                search_page_url => search_page_url.as_deref(),
                search_page_size => search_page_size,
                theme_css_vars => theme_css_vars,
            })?;
            let out_rel = page.output_rel_path(); // route + "index.html"（/ 区切り）
            assets::write_output(ctx.outputs, output_dir, &out_rel, html.as_bytes())?;
            // ページ単位 Markdown（原文バイトそのまま）。コピーボタンと llms.txt の
            // .md リンクの実体。`yuzu fmt` 運用なら正規形と一致する。
            // 検索結果ページと、`llms.page_md = false` / frontmatter `pageMd: false` では
            // 出さない（既存の dist に残った分は出力マニフェストの孤児掃除で消える）
            if page.emits_page_md(cfg.llms.page_md) {
                assets::write_output(
                    ctx.outputs,
                    output_dir,
                    &page.md_rel_path(),
                    page.source.as_bytes(),
                )?;
            }
            tracing::debug!(page = %page.rel.display(), out = %out_rel, "ページ出力");
            Ok(())
        })?;

    // エイリアス（frontmatter aliases）→ 旧 URL へのリダイレクト HTML（直列部）。
    // 出力はマニフェストに記録されるため、エイリアス削除時は孤児掃除で自動的に消える。
    // 衝突は冒頭の validate_aliases で除外済み = ここで実ページを上書きすることはない
    let redirect_template = shared.env.get_template("redirect.jinja")?;
    for page in &params.site.pages {
        for alias_route in yuzu_core::alias_routes(page) {
            let html = redirect_template.render(context! {
                site => site_ctx,
                target_url => resolver.page_url(&page.route),
                target_title => page.title,
            })?;
            assets::write_output(
                ctx.outputs,
                output_dir,
                &format!("{alias_route}index.html"),
                html.as_bytes(),
            )?;
            tracing::debug!(page = %page.rel.display(), alias = %alias_route, "リダイレクト出力");
        }
    }

    // 404 ページ（GitHub Pages 等の静的ホスティングは 404.html を自動で使う）。
    // copy_public より前に書く = public/404.html を置いたプロジェクトは
    // そちらが上書きして優先される（テーマ上書きと同じ思想）
    let not_found = shared.env.get_template("404.jinja")?.render(context! {
        site => site_ctx,
        page => context! {
            title => "ページが見つかりません",
            description => Option::<&str>::None,
            // 404 は URL を持たない（どの route にも対応しない）ので canonical を出さない
            canonical_url => Option::<&str>::None,
            toc => Vec::<String>::new(),
        },
        // route "404.html" はどのページ route とも一致しない = trail が空で
        // サイドバーはハイライトなし・全セクション閉じ（nav.collapse 時）
        nav => NavCtx::build(&params.site.nav, &[], &resolver),
        nav_collapse => cfg.nav.collapse,
        base_url => resolver.base(),
        asset_url => resolver.asset_url(),
        live_reload_poll => params.live_reload == LiveReloadMode::Poll,
        live_reload_ws => params.live_reload == LiveReloadMode::Ws,
        mermaid_enabled => false,
        math_enabled => false,
        highlight_enabled => cfg.markdown.highlight.enabled,
        dark_enabled => cfg.theme.dark == yuzu_config::DarkMode::Toggle,
        theme_mode => cfg.theme.dark.as_str(),
        search_enabled => cfg.search.enabled,
        search_page_url => search_page_url.as_deref(),
        theme_css_vars => theme_css_vars,
    })?;
    if SiteOutput::NotFound.emitted(&switches) {
        assets::write_output(
            ctx.outputs,
            output_dir,
            SiteOutput::NotFound.path(),
            not_found.as_bytes(),
        )?;
    }

    if SiteOutput::ThemeAssets.emitted(&switches) {
        assets::write_theme_assets(output_dir, rc.theme_dir.as_deref(), ctx.outputs)?;
    }
    // syntect.css はハイライト有効時だけ書く（base.jinja の <link> も
    // `highlight_enabled` で同じ条件。止めたら掃除が消す）。CSS の中身は
    // ハイライト有効時だけ RenderShared が作っている
    if let (true, Some(css)) = (
        SiteOutput::SyntectCss.emitted(&switches),
        &shared.syntect_css,
    ) {
        assets::write_output(
            ctx.outputs,
            output_dir,
            SiteOutput::SyntectCss.path(),
            css.as_bytes(),
        )?;
    }

    // llms.txt / llms-full.txt（copy_public より前 = ユーザが public/llms.txt を
    // 置いた場合はそちらが上書きして優先される。テーマ上書きと同じ思想）
    crate::llms::write_llms_files(rc, params.site, output_dir, ctx, &switches)?;

    // sitemap.xml（copy_public より前 = `public/sitemap.xml` を置けばそちらが優先）。
    // `<loc>` は絶対 URL が仕様のため、baseUrl がフル URL のときだけ生成する。
    // 対象は非 draft の実ページ＋用語集のみ（エイリアス・404・ページ単位 .md・
    // 検索結果ページは載せない — 判定は Page::in_sitemap）
    if SiteOutput::Sitemap.emitted(&switches) {
        let mut xml = String::from(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
             <urlset xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\">\n",
        );
        for page in params.site.pages.iter().filter(|p| p.in_sitemap()) {
            let rel_key = page
                .rel
                .iter()
                .map(|c| c.to_string_lossy())
                .collect::<Vec<_>>()
                .join("/");
            xml.push_str("  <url><loc>");
            xml.push_str(&crate::highlight::escape_html(
                &resolver.page_url(&page.route),
            ));
            xml.push_str("</loc>");
            // 最終コミット日があるページだけ lastmod を付ける（git 連携メタと同じ供給源）
            if let Some(date) = params.git_dates.and_then(|dates| dates.get(&rel_key)) {
                xml.push_str("<lastmod>");
                xml.push_str(&crate::highlight::escape_html(date));
                xml.push_str("</lastmod>");
            }
            xml.push_str("</url>\n");
        }
        xml.push_str("</urlset>\n");
        assets::write_output(
            ctx.outputs,
            output_dir,
            SiteOutput::Sitemap.path(),
            xml.as_bytes(),
        )?;
    } else {
        tracing::debug!("baseUrl がフル URL ではないため sitemap.xml は生成しない");
    }

    // content の同伴アセット（ページ横の画像等）。copy_public より前 = 同じパスに
    // public/ のファイルがあればそちらが上書きして優先される（テーマ上書きと同じ思想）
    assets::copy_content_assets(&rc.content_dir, &cfg.input.ignore, output_dir, ctx.outputs)?;
    assets::copy_public(rc.public_dir.as_deref(), output_dir, ctx.outputs)?;
    // build_id は `build --watch` のときだけ（判断は output_switches）
    if SiteOutput::BuildId.emitted(&switches) {
        assets::write_build_id(output_dir, ctx.outputs)?;
    }

    tracing::info!(
        pages = params.site.pages.len(),
        out = %output_dir.display(),
        "ビルド完了"
    );
    Ok(())
}
