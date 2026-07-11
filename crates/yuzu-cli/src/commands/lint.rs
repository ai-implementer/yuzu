//! `yuzu lint`: 文書規約の診断（見出し・frontmatter）。
//! リンク切れ・fmt 差分まで含めた統合チェックは `yuzu check`

use std::process::ExitCode;

use anyhow::Context;
use yuzu_core::{LintOptions, MarkdownOptions};

use super::diag;

pub fn run() -> anyhow::Result<ExitCode> {
    let cwd = std::env::current_dir().context("カレントディレクトリを取得できません")?;
    let root = yuzu_config::find_project_root(&cwd)?;
    let rc = yuzu_config::load(&root)?;
    let opts = MarkdownOptions {
        gfm: rc.config.markdown.gfm,
        math: rc.config.markdown.math.enabled,
    };
    let lint_opts = LintOptions {
        max_directory_depth: rc.config.lint.max_directory_depth,
    };

    let pages = yuzu_core::build_source_pages(&rc.content_dir, &rc.config.input.ignore, &opts)?;
    let mut diags = Vec::new();
    for page in &pages {
        diags.extend(yuzu_core::lint_page(page, &opts, &lint_opts)?);
    }

    let prefix = rc
        .content_dir
        .strip_prefix(&root)
        .unwrap_or(&rc.content_dir);
    let (errors, warnings) = diag::print_diagnostics(&diags, prefix);
    if diags.is_empty() {
        println!("問題ありません（{} ページ）", pages.len());
        return Ok(ExitCode::SUCCESS);
    }
    println!("エラー {errors} 件・警告 {warnings} 件");
    Ok(ExitCode::from(1))
}
