//! `yuzu check`: lint ＋ リンク切れ検査 ＋ fmt 差分検出の統合チェック（CI 用）。
//! 1 件でも診断があれば終了コード 1。
//! `--external-links` を付けたときだけ外部リンクの到達性も見る（`extlink`。
//! 既定経路はネットワークに触れない）。
//! 検査の並びは `collect`（build の件数の報告と共有）が持つ

use std::process::ExitCode;

use super::collect::{self, Scope};
use super::diag;

pub fn run(
    cx: &crate::cx::Cx,
    format: diag::Format,
    external_links: bool,
    no_fmt: bool,
) -> anyhow::Result<ExitCode> {
    let rc = super::load_project(cx)?;
    let opts = yuzu_render::markdown_options(&rc.config);
    let pages = yuzu_core::build_source_pages(&rc.content_dir, &rc.config.input.ignore, &opts)?;

    let collected = collect::collect(
        &rc,
        &pages,
        &opts,
        &Scope::Check {
            fmt: !no_fmt,
            external_links,
        },
    )?;
    let skipped = collected.skipped();
    // frontmatter `lintDisable` のページ単位抑制。全検査の診断が
    // この漏斗を通ってから報告される（config-* は ProjectRoot なので素通り）
    let yuzu_core::SuppressionOutcome {
        diags,
        suppressed,
        disabled,
    } = collected.suppress(&pages, &opts);

    diag::report(
        format,
        diags,
        &diag::Context {
            root: &rc.root,
            content_dir: &rc.content_dir,
            pages: pages.iter().filter(|p| !p.is_generated()).count(),
            suppressed,
            disabled,
            skipped,
        },
    )
}
