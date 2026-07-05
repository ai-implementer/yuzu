//! `yuzu fmt [--check]`: content/ の Markdown を正規形へ整形する。
//!
//! - 既定はその場で書き換え（rustfmt/gofmt 流）。差分がなければ**書き込まない**
//!   （mtime を汚さず `yuzu dev` の無駄な再ビルドも防ぐ）
//! - `--check` は差分のあるファイルを列挙するだけ（gofmt -l 流）。
//!   差分があれば終了コード 1（CI 用）
//! - draft ページも対象（リポジトリ内のソースは全て規約対象）

use std::process::ExitCode;

use anyhow::Context;
use yuzu_core::MarkdownOptions;

pub fn run(check: bool) -> anyhow::Result<ExitCode> {
    let cwd = std::env::current_dir().context("カレントディレクトリを取得できません")?;
    let root = yuzu_config::find_project_root(&cwd)?;
    let rc = yuzu_config::load(&root)?;
    let opts = MarkdownOptions {
        gfm: rc.config.markdown.gfm,
    };

    let pages = yuzu_core::build_source_pages(&rc.content_dir, &rc.config.input.ignore, &opts)?;

    let mut changed = 0usize;
    for page in &pages {
        let formatted = yuzu_core::format_document(page, &opts)?;
        if formatted == page.source {
            continue;
        }
        changed += 1;
        // プロジェクトルート相対で表示（例: content/guide/x.md）
        let display = page.src.strip_prefix(&root).unwrap_or(&page.src);
        if check {
            println!("{}", display.display());
        } else {
            std::fs::write(&page.src, &formatted)
                .with_context(|| format!("{} に書き込めません", page.src.display()))?;
            println!("整形: {}", display.display());
        }
    }

    if check && changed > 0 {
        eprintln!("{changed} ファイルに整形差分があります（`yuzu fmt` を実行してください）");
        return Ok(ExitCode::from(1));
    }
    if !check {
        if changed == 0 {
            println!("整形の必要はありません（{} ページ）", pages.len());
        } else {
            println!("{changed} ファイルを整形しました");
        }
    }
    Ok(ExitCode::SUCCESS)
}
