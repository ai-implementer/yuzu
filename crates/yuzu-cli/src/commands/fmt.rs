//! `yuzu fmt [--check] [--diff]`: content/ の Markdown を正規形へ整形する。
//!
//! - 既定はその場で書き換え（rustfmt/gofmt 流）。差分がなければ**書き込まない**
//!   （mtime を汚さず `yuzu dev` の無駄な再ビルドも防ぐ）
//! - `--check` は差分のあるファイルを列挙するだけ（gofmt -l 流）。
//!   差分があれば終了コード 1（CI 用）
//! - `--diff` は unified diff を標準出力へ出す（`--check` を含意 = 書き換えない）。
//!   **標準出力は diff 本体だけ**にして `> x.patch` → `patch -p1` が通る形を保つ
//!   （集計行は stderr。`--format json` の「stdout に契約物以外を書かない」と同じ規律）
//! - draft ページも対象（リポジトリ内のソースは全て規約対象）

use std::path::Path;
use std::process::ExitCode;

use anyhow::Context;
use yuzu_core::MarkdownOptions;

use crate::out::outln;

pub fn run(check: bool, diff: bool) -> anyhow::Result<ExitCode> {
    // --diff は「差分を見せる = 書き換えない」（gofmt -d 流）
    let dry_run = check || diff;
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
    };

    let pages = yuzu_core::build_source_pages(&rc.content_dir, &rc.config.input.ignore, &opts)?;

    let mut changed = 0usize;
    // 合成ページ（用語集）は原稿ではないので整形対象外。**ガードが無いと
    // `fs::write` が実在しない content/glossary.md を新規作成してしまう**
    for page in pages.iter().filter(|p| !p.generated) {
        let formatted = yuzu_core::format_document(page, &opts)?;
        if formatted == page.source {
            continue;
        }
        changed += 1;
        let display = diff_path(&root, &page.src);
        if diff {
            // ファイル単位で 1 回だけ書く（行ごとに書くと遅く、SIGPIPE の窓も増える）
            crate::out::str(&unified_diff(&display, &page.source, &formatted));
        } else if check {
            outln!("{display}");
        } else {
            std::fs::write(&page.src, &formatted)
                .with_context(|| format!("{} に書き込めません", page.src.display()))?;
            outln!("整形: {display}");
        }
    }

    if dry_run && changed > 0 {
        let hint = match diff {
            true => "`yuzu fmt` で適用できます",
            false => "`yuzu fmt` を実行してください。`yuzu fmt --diff` で内容を確認できます",
        };
        eprintln!("{changed} ファイルに整形差分があります（{hint}）");
        return Ok(ExitCode::from(1));
    }
    if !dry_run {
        if changed == 0 {
            outln!("整形の必要はありません（{} ページ）", pages.len());
        } else {
            outln!("{changed} ファイルを整形しました");
        }
    }
    Ok(ExitCode::SUCCESS)
}

/// diff ヘッダ用のパス表記: プロジェクトルート相対・`/` 区切り
/// （`patch -p1` に食わせられる形。診断の path 契約と同じ規律）
fn diff_path(root: &Path, src: &Path) -> String {
    // ルート外（想定外だが起こりうる）は加工せずそのまま出す。
    // components() で組み立てると絶対パスの RootDir が "/" として混ざり `//x` になる
    let Ok(rel) = src.strip_prefix(root) else {
        return src.display().to_string();
    };
    rel.components()
        .map(|c| c.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

/// 整形前後の unified diff（同一なら空文字列）。
///
/// `a/` `b/` 接頭辞付き・コンテキスト 3 行・タイムスタンプなし（決定的出力）。
/// 末尾改行の有無は similar が `\ No newline at end of file` で表す
/// （整形後は必ず末尾改行が付くため、実際に出る）
fn unified_diff(display_path: &str, before: &str, after: &str) -> String {
    if before == after {
        return String::new();
    }
    similar::TextDiff::from_lines(before, after)
        .unified_diff()
        .context_radius(3)
        .header(&format!("a/{display_path}"), &format!("b/{display_path}"))
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 1〜n 行の連番テキスト（末尾改行あり）
    fn lines(n: usize) -> String {
        (1..=n).map(|i| format!("{i} 行目\n")).collect()
    }

    /// `changed`（1 始まりの行番号）だけを書き換えたテキスト。
    /// 文字列置換だと「3 行目」が「13 行目」にも当たるので行単位で差し替える
    fn lines_with_changes(n: usize, changed: &[usize]) -> String {
        (1..=n)
            .map(|i| match changed.contains(&i) {
                true => format!("{i} 行目（変更）\n"),
                false => format!("{i} 行目\n"),
            })
            .collect()
    }

    #[test]
    fn 差分が無ければ空文字列を返す() {
        assert_eq!(unified_diff("content/x.md", "同じ\n", "同じ\n"), "");
    }

    #[test]
    fn ヘッダはルート相対のスラッシュ区切りになる() {
        let path = diff_path(
            Path::new("/proj"),
            &Path::new("/proj")
                .join("content")
                .join("guide")
                .join("x.md"),
        );
        assert_eq!(path, "content/guide/x.md");
        let out = unified_diff(&path, "前\n", "後\n");
        assert!(
            out.starts_with("--- a/content/guide/x.md\n+++ b/content/guide/x.md\n"),
            "{out}"
        );
    }

    #[test]
    fn ルート外のパスはそのまま表示する() {
        // strip_prefix に失敗しても診断を落とさない（絶対パスで出す）
        let path = diff_path(Path::new("/other"), Path::new("/proj/content/x.md"));
        assert_eq!(path, "/proj/content/x.md");
    }

    #[test]
    fn 変更行の前後にコンテキストが3行付く() {
        let out = unified_diff("content/x.md", &lines(10), &lines_with_changes(10, &[5]));
        assert!(out.contains("@@ -2,7 +2,7 @@"), "{out}");
        assert!(out.contains("-5 行目"), "{out}");
        assert!(out.contains("+5 行目（変更）"), "{out}");
        assert!(!out.contains(" 1 行目"), "3 行より前は出ない: {out}");
    }

    #[test]
    fn 離れた変更箇所は独立したハンクになる() {
        let out = unified_diff(
            "content/x.md",
            &lines(40),
            &lines_with_changes(40, &[3, 30]),
        );
        assert_eq!(out.matches("@@ ").count(), 2, "{out}");
    }

    #[test]
    fn 近接した変更箇所は1つのハンクに結合される() {
        // 間隔がコンテキスト 2 倍以下なら 1 ハンクへ結合される
        let out = unified_diff(
            "content/x.md",
            &lines(20),
            &lines_with_changes(20, &[8, 10]),
        );
        assert_eq!(out.matches("@@ ").count(), 1, "{out}");
    }

    #[test]
    fn 末尾改行の無いファイルは_no_newline_マーカーを出す() {
        // fmt の正規形は必ず末尾改行を持つので、この差分は実運用で出る
        let out = unified_diff("content/x.md", "本文", "本文\n");
        assert!(out.contains("\\ No newline at end of file"), "{out}");
    }

    #[test]
    fn 日本語だけの行でもハンクの行番号が壊れない() {
        let before = "あいうえお\nかきくけこ\nさしすせそ\n";
        let after = "あいうえお\nかきくけこ！\nさしすせそ\n";
        let out = unified_diff("content/x.md", before, after);
        // 行番号は文字数ではなく行の数え上げ（1 行目から 3 行ぶん）
        assert!(out.contains("@@ -1,3 +1,3 @@"), "{out}");
    }
}
