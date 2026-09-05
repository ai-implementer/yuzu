//! `yuzu check`: lint ＋ リンク切れ検査 ＋ fmt 差分検出の統合チェック（CI 用）。
//! 1 件でも診断があれば終了コード 1。
//! `--external-links` を付けたときだけ外部リンクの到達性も見る（`extlink`。
//! 既定経路はネットワークに触れない）

use std::process::ExitCode;

use yuzu_core::{DiagBase, Diagnostic, MarkdownOptions};

use super::diag;

pub fn run(format: diag::Format, external_links: bool) -> anyhow::Result<ExitCode> {
    let (root, rc) = super::load_project()?;
    let opts = MarkdownOptions {
        gfm: rc.config.markdown.gfm,
        math: rc.config.markdown.math.enabled,
        mermaid: rc.config.markdown.mermaid.enabled,
        crossref_site_numbering: matches!(
            rc.config.markdown.crossref.numbering,
            yuzu_config::CrossrefNumbering::Site
        ),
        glossary: yuzu_render::glossary_options(&rc.config),
        search_page: yuzu_render::search_page_options(&rc.config),
    };
    let lint_opts = diag::lint_options(&rc);

    let pages = yuzu_core::build_source_pages(&rc.content_dir, &rc.config.input.ignore, &opts)?;

    let mut diags = Vec::new();
    for page in &pages {
        // 文書規約
        diags.extend(yuzu_core::lint_page(page, &opts, &lint_opts)?);
        // fmt 差分（ファイル単位・位置なし）。合成ページは `yuzu fmt` の
        // 対象外なので差分を報告しても直しようがない
        if !page.is_generated() && yuzu_core::format_document(page, &opts)? != page.source {
            diags.push(Diagnostic {
                rule: yuzu_core::rules::FMT.id,
                severity: yuzu_core::rules::FMT.severity,
                base: DiagBase::Content,
                rel: page.rel.clone(),
                span: None,
                // 差分の中身は診断に載せない（github 形式は改行を %0A にするため
                // 巨大な 1 行注釈になる）。見たい人を `fmt --diff` へ案内する
                message: "整形差分があります（`yuzu fmt` で修正、`yuzu fmt --diff` で内容を確認できます）"
                    .to_string(),
                fix: None,
            });
        }
    }
    // プロジェクト横断ルール（長音符ゆれの混在等）
    diags.extend(yuzu_core::lint_project(&pages, &opts)?);
    // エイリアス（frontmatter aliases）の形式・衝突。
    // draft 込みの全ソースで検証する（公開前に矛盾を検出する）
    diags.extend(yuzu_core::validate_aliases(&pages, &opts));
    // ページ URL の一意性（`x.md` と `x/index.md` は同じ URL になる）。
    // alias と同じく draft 込みで検証する（公開前に矛盾を検出する）
    diags.extend(yuzu_core::validate_routes(&pages));
    // コンテンツインクルード（file=）の参照切れ・ルート外・行範囲外
    diags.extend(super::diag::config_diagnostics(&rc));
    diags.extend(yuzu_core::validate_includes(&pages, &root, &opts));
    // openapi / jsonschema の file: 参照の切れ・ルート外（記法の解釈は core が持つ）
    diags.extend(yuzu_core::validate_spec_refs(&pages, &root, &opts));
    // 仕様の中身（パース失敗・未対応バージョン・$ref 先）。参照が解決できた
    // ブロックだけを見る。描画は失敗してもエラーボックスで継続するため、
    // 公開前に気づける場所はこの 2 つだけ
    diags.extend(yuzu_render::validate_api_specs(&pages, &root, &opts));
    // 内部リンク・アンカー（外部リンクの出現箇所は捨てずに受け取る）
    let yuzu_core::LinkReport {
        diags: link_diags,
        external,
    } = yuzu_core::check_links(&pages, rc.public_dir.as_deref(), &rc.content_dir, &opts)?;
    diags.extend(link_diags);
    // 外部リンクの到達性（opt-in。ここだけがネットワークに触れる）。
    // 診断は下の抑制の漏斗を通す = lintDisable / 行コメント / lint.rules が効く
    let skipped = if external_links {
        let outcome = super::extlink::check(&external)?;
        diags.extend(outcome.diags);
        outcome.skipped
    } else {
        0
    };

    // frontmatter `lintDisable` のページ単位抑制。全検査の診断が
    // この漏斗を通ってから報告される（config-* は ProjectRoot なので素通り）
    let yuzu_core::SuppressionOutcome {
        diags,
        suppressed,
        disabled,
    } = yuzu_core::apply_suppressions(diags, &pages, &opts, &lint_opts);

    diag::report(
        format,
        diags,
        &diag::Context {
            root: &root,
            content_dir: &rc.content_dir,
            pages: pages.iter().filter(|p| !p.is_generated()).count(),
            suppressed,
            disabled,
            skipped,
        },
    )
}
