//! `yuzu lint [--fix]`: 文書規約の診断（見出し・frontmatter・表記ゆれ）。
//! リンク切れ・fmt 差分まで含めた統合チェックは `yuzu check`
//!
//! `--fix` は表記ゆれ系の変換候補（[`yuzu_core::Diagnostic::fix`]）をソースへ
//! 自動適用する。fmt と同じ規約: 冪等・差分のないファイルには書き込まない
//! （mtime を汚さない）。frontmatter は lint の対象外なので触れない。
//! 修正できない違反（見出し規約等）は従来どおり報告して終了コード 1

use std::process::ExitCode;

use anyhow::Context;
use yuzu_core::{Diagnostic, LintOptions, MarkdownOptions, Page};

use super::diag;

/// fix の適用が別のゆれを生む連鎖に備えた再 lint の上限（通常は 1 周で収束）
const MAX_FIX_ROUNDS: usize = 10;

pub fn run(fix: bool) -> anyhow::Result<ExitCode> {
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
    let collect = |pages: &[Page]| -> anyhow::Result<Vec<Diagnostic>> {
        let mut diags = Vec::new();
        for page in pages {
            diags.extend(yuzu_core::lint_page(page, &opts, &lint_opts)?);
        }
        // プロジェクト横断ルール（長音符ゆれの混在等）を合流させる
        diags.extend(yuzu_core::lint_project(pages, &opts, &lint_opts)?);
        Ok(diags)
    };

    let mut fixed_total = 0usize;
    let mut fixed_files = std::collections::BTreeSet::new();
    if fix {
        for _ in 0..MAX_FIX_ROUNDS {
            let pages =
                yuzu_core::build_source_pages(&rc.content_dir, &rc.config.input.ignore, &opts)?;
            let diags = collect(&pages)?;
            let mut applied_this_round = 0usize;
            for page in &pages {
                let page_diags: Vec<Diagnostic> = diags
                    .iter()
                    .filter(|d| d.rel == page.rel && d.fix.is_some())
                    .cloned()
                    .collect();
                if page_diags.is_empty() {
                    continue;
                }
                let (fixed, applied) = yuzu_core::apply_fixes(&page.source, &page_diags);
                // 適用 0 件（範囲交差で全スキップ等）なら書き込まない（mtime 温存）
                if applied == 0 || fixed == page.source {
                    continue;
                }
                std::fs::write(&page.src, &fixed)
                    .with_context(|| format!("{} に書き込めません", page.src.display()))?;
                applied_this_round += applied;
                fixed_files.insert(page.src.strip_prefix(&root).unwrap_or(&page.src).to_owned());
            }
            if applied_this_round == 0 {
                break; // 不動点（fix 対象なし or 全て適用不能）
            }
            fixed_total += applied_this_round;
        }
    }

    // 最終状態の報告（--fix 適用後に残った違反 = 機械修正できないもの）
    let pages = yuzu_core::build_source_pages(&rc.content_dir, &rc.config.input.ignore, &opts)?;
    let diags = collect(&pages)?;

    let prefix = rc
        .content_dir
        .strip_prefix(&root)
        .unwrap_or(&rc.content_dir);
    for file in &fixed_files {
        println!("修正: {}", file.display());
    }
    if fixed_total > 0 {
        println!(
            "{fixed_total} 件を自動修正しました（{} ファイル）",
            fixed_files.len()
        );
    }
    let (errors, warnings) = diag::print_diagnostics(&diags, prefix);
    if diags.is_empty() {
        println!("問題ありません（{} ページ）", pages.len());
        return Ok(ExitCode::SUCCESS);
    }
    println!("エラー {errors} 件・警告 {warnings} 件");
    Ok(ExitCode::from(1))
}
