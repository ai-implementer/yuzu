//! 診断の収集（`yuzu check` と、`yuzu build` の件数の報告が通る唯一の入口）。
//!
//! 以前は check が検査を 11 系統並べ、build はそのうち 3 系統を別に数えたうえで、
//! 大文字小文字だけが違う URL（`route-case-conflict`）を `lint.rules` を直接読んで判定していた。
//! そのため frontmatter の `lintDisable` が build では効かなかった（10-12 レビューの R2）。
//! ここでは検査の並びを 1 つにして、どこまで見るかだけを [`Scope`] で選ぶ。
//! 集めた診断は [`Collected::suppress`]（core の `apply_suppressions`）を通した後でしか読めない

use yuzu_config::ResolvedConfig;
use yuzu_core::{Diagnostic, LintOptions, MarkdownOptions, Page, SuppressionOutcome};
use yuzu_render::LiveReloadMode;

use super::diag;

/// 収集の範囲
pub(crate) enum Scope {
    /// `yuzu check`: すべての検査。`fmt = false` は `--no-fmt`、`external_links` は
    /// `--external-links`（ここだけがネットワークに触れる）
    Check { fmt: bool, external_links: bool },
    /// `yuzu build` の件数の報告: ビルドしたページの壊れたリンク・取り込みの失敗と、
    /// 大文字小文字だけが違う URL。`live_reload` はリンク先として有効な生成物
    /// （`build --watch` の build_id）を決める
    Build {
        include_drafts: bool,
        live_reload: LiveReloadMode,
    },
}

/// 抑制を適用する前の診断。中身は [`Collected::suppress`] を通してからしか読めない
/// （抑制を通さずに数えると、check と build で件数が食い違う）
pub(crate) struct Collected {
    diags: Vec<Diagnostic>,
    lint: LintOptions,
    /// `--external-links` で到達性を判定できなかった URL の数
    skipped: usize,
}

impl Collected {
    /// 外部リンク検査で判定できなかった URL の数（集計行の「スキップ」）
    pub(crate) fn skipped(&self) -> usize {
        self.skipped
    }

    /// frontmatter `lintDisable`・行コメント・`lint.rules` を適用する
    pub(crate) fn suppress(self, pages: &[Page], opts: &MarkdownOptions) -> SuppressionOutcome {
        yuzu_core::apply_suppressions(self.diags, pages, opts, &self.lint)
    }
}

/// `pages` を検査して診断を集める。check は draft 込みの全ページ（`build_source_pages`）、
/// build はビルドしたページ（`--drafts` なら draft も）を渡す
pub(crate) fn collect(
    rc: &ResolvedConfig,
    pages: &[Page],
    opts: &MarkdownOptions,
    scope: &Scope,
) -> anyhow::Result<Collected> {
    let root = &rc.root;
    let (check, fmt, external_links, include_drafts, live_reload) = match *scope {
        Scope::Check {
            fmt,
            external_links,
        } => (true, fmt, external_links, false, LiveReloadMode::None),
        Scope::Build {
            include_drafts,
            live_reload,
        } => (false, false, false, include_drafts, live_reload),
    };
    let mut lint = diag::lint_options(rc, external_links);
    let mut diags = Vec::new();

    if check {
        for page in pages {
            // 文書規約
            diags.extend(yuzu_core::lint_page(page, opts, &lint)?);
            // fmt 差分（ファイル単位・位置なし）。合成ページは `yuzu fmt` の
            // 対象外なので差分を報告しても直しようがない。`--no-fmt` なら見ない
            // （fmt は error のまま外せないので、既存の原稿を持ち込むと全ファイルが赤になる）
            if fmt && !page.is_generated() && yuzu_core::format_document(page, opts)? != page.source
            {
                // 差分の中身は診断に載せない（github 形式は改行を %0A にするため
                // 巨大な 1 行注釈になる）。見たい人を `fmt --diff` へ案内する
                diags.push(Diagnostic::new(
                    yuzu_core::rules::FMT,
                    page.rel.clone(),
                    None,
                    "整形差分があります（`yuzu fmt` で修正、`yuzu fmt --diff` で内容を確認できます）",
                ));
            }
        }
        // プロジェクト横断ルール（長音符ゆれの混在等）
        diags.extend(yuzu_core::lint_project(pages, opts)?);
        // エイリアス（frontmatter aliases）の形式・衝突。
        // draft 込みの全ソースで検証する（公開前に矛盾を検出する）
        diags.extend(yuzu_core::validate_aliases(pages, opts));
        // ページ URL の一意性（`x.md` と `x/index.md` は同じ URL になる）。
        // alias と同じく draft 込みで検証する（公開前に矛盾を検出する）
        diags.extend(yuzu_core::validate_routes(pages));
    }
    // 大文字小文字だけが違う URL（warning。区別しない FS で出力が 1 つにまとまる）
    diags.extend(yuzu_core::validate_url_case(pages, opts));
    if check {
        diags.extend(diag::config_diagnostics(rc));
    }
    // コンテンツインクルード（file=）の参照切れ・ルート外・行範囲外
    diags.extend(yuzu_core::validate_includes(pages, root, opts));
    // openapi / jsonschema の file: 参照の切れ・ルート外（記法の解釈は core が持つ）
    diags.extend(yuzu_core::validate_spec_refs(pages, root, opts));
    if check {
        // 仕様の中身（パース失敗・未対応バージョン・$ref 先）。参照が解決できた
        // ブロックだけを見る。描画は失敗してもエラーボックスで継続するため、
        // 公開前に気づける場所はここだけ（build の件数には入れない = Phase 85 の判断。
        // 描画のたびに全仕様をパースし直すことになる）
        diags.extend(yuzu_render::validate_api_specs(pages, root, opts));
    }
    // 内部リンク・アンカー（外部リンクの出現箇所は捨てずに受け取る）。
    // ビルドの生成物へのリンクは、build と同じ設定から作った「今回出すもの」で照合する
    let yuzu_core::LinkReport {
        diags: link_diags,
        external,
    } = yuzu_core::check_links(
        pages,
        rc.public_dir.as_deref(),
        &rc.content_dir,
        &rc.config.input.ignore,
        include_drafts,
        &yuzu_render::output_switches(rc, live_reload),
        opts,
    )?;
    diags.extend(link_diags);
    // 外部リンクの到達性（opt-in。ここだけがネットワークに触れる）。
    // 診断は抑制の漏斗を通す = lintDisable / 行コメント / lint.rules が効く
    let mut skipped = 0;
    if external_links {
        let outcome = super::extlink::check(&external)?;
        diags.extend(outcome.diags);
        // 到達性を判定できなかった出現箇所は抑制の unused 判定から外す
        // （環境要因で CI を落とさない契約。診断が無いので suppressed にも数えない）
        lint.unevaluated_occurrences = outcome
            .skipped_links
            .iter()
            .map(|l| {
                (
                    l.rel.clone(),
                    l.span.start_line,
                    yuzu_core::rules::EXTERNAL_LINK_BROKEN.id.to_string(),
                )
            })
            .collect();
        skipped = outcome.skipped;
    }
    Ok(Collected {
        diags,
        lint,
        skipped,
    })
}
