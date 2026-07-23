//! `yuzu check`: lint ＋ リンク切れ検査 ＋ fmt 差分検出の統合チェック（CI 用）。
//! 1 件でも診断があれば終了コード 1

use std::process::ExitCode;

use anyhow::Context;
use yuzu_core::{Diagnostic, LintOptions, MarkdownOptions, Severity};

use super::diag;

pub fn run() -> anyhow::Result<ExitCode> {
    let cwd = std::env::current_dir().context("カレントディレクトリを取得できません")?;
    let root = yuzu_config::find_project_root(&cwd)?;
    let rc = yuzu_config::load(&root)?;
    let opts = MarkdownOptions {
        gfm: rc.config.markdown.gfm,
        math: rc.config.markdown.math.enabled,
        mermaid: rc.config.markdown.mermaid.enabled,
    };
    let lint_opts = LintOptions {
        max_directory_depth: rc.config.lint.max_directory_depth,
        terms: rc.config.lint.terms.clone(),
        rules: yuzu_core::LintRules {
            fullwidth_alphanumeric: rc.config.lint.rules.fullwidth_alphanumeric,
            halfwidth_kana: rc.config.lint.rules.halfwidth_kana,
            katakana_choon: rc.config.lint.rules.katakana_choon,
        },
    };

    let pages = yuzu_core::build_source_pages(&rc.content_dir, &rc.config.input.ignore, &opts)?;

    let mut diags = Vec::new();
    for page in &pages {
        // 文書規約
        diags.extend(yuzu_core::lint_page(page, &opts, &lint_opts)?);
        // fmt 差分（ファイル単位・位置なし）
        if yuzu_core::format_document(page, &opts)? != page.source {
            diags.push(Diagnostic {
                rule: "fmt",
                severity: Severity::Error,
                rel: page.rel.clone(),
                span: None,
                message: "整形差分があります（`yuzu fmt` を実行してください）".to_string(),
                fix: None,
            });
        }
    }
    // プロジェクト横断ルール（長音符ゆれの混在等）
    diags.extend(yuzu_core::lint_project(&pages, &opts, &lint_opts)?);
    // エイリアス（frontmatter aliases）の形式・衝突。
    // draft 込みの全ソースで検証する（公開前に矛盾を検出する）
    diags.extend(yuzu_core::validate_aliases(&pages, &opts));
    // 内部リンク・アンカー
    diags.extend(yuzu_core::check_links(
        &pages,
        rc.public_dir.as_deref(),
        &rc.content_dir,
        &opts,
    )?);

    diag::sort_diagnostics(&mut diags);
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
