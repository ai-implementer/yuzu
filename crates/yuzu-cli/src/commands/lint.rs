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
use crate::out::outln;

/// fix の適用が別のゆれを生む連鎖に備えた再 lint の上限（通常は 1 周で収束）
const MAX_FIX_ROUNDS: usize = 10;

pub fn run(fix: bool, format: diag::Format) -> anyhow::Result<ExitCode> {
    let cwd = std::env::current_dir().context("カレントディレクトリを取得できません")?;
    let root = yuzu_config::find_project_root(&cwd)?;
    let rc = yuzu_config::load(&root)?;
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
            for page in pages.iter().filter(|p| !p.is_generated()) {
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
    let mut diags = collect(&pages)?;
    diags.extend(diag::config_diagnostics(&rc));

    // --fix の進捗は human 以外では stderr へ逃がす
    // （json は JSON オブジェクト以外を標準出力へ書かない契約のため）
    let progress_to_stdout = format == diag::Format::Human;
    for file in &fixed_files {
        let line = format!("修正: {}", file.display());
        if progress_to_stdout {
            outln!("{line}");
        } else {
            eprintln!("{line}");
        }
    }
    if fixed_total > 0 {
        let line = format!(
            "{fixed_total} 件を自動修正しました（{} ファイル）",
            fixed_files.len()
        );
        if progress_to_stdout {
            outln!("{line}");
        } else {
            eprintln!("{line}");
        }
    }

    diag::report(
        format,
        diags,
        &diag::Context {
            root: &root,
            content_dir: &rc.content_dir,
            // 集計行は原稿の数を出す（合成した用語集ページは数えない）
            pages: pages.iter().filter(|p| !p.is_generated()).count(),
        },
    )
}
