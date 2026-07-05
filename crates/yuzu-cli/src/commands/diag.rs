//! 診断の表示ヘルパ（`yuzu lint` / `yuzu check` 共通）

use std::path::Path;

use yuzu_core::{Diagnostic, Severity};

/// `content/guide/x.md:12:1: warning[rule] メッセージ` 形式で表示し、
/// (エラー数, 警告数) を返す。`content_prefix` はプロジェクトルートから
/// content ディレクトリへの相対パス（通常 `content`）
pub fn print_diagnostics(diags: &[Diagnostic], content_prefix: &Path) -> (usize, usize) {
    let mut errors = 0;
    let mut warnings = 0;
    for d in diags {
        let severity = match d.severity {
            Severity::Error => {
                errors += 1;
                "error"
            }
            Severity::Warning => {
                warnings += 1;
                "warning"
            }
        };
        let path = content_prefix.join(&d.rel);
        match d.span {
            Some(span) => println!(
                "{}:{}:{}: {severity}[{}] {}",
                path.display(),
                span.start_line,
                span.start_col,
                d.rule,
                d.message
            ),
            None => println!("{}: {severity}[{}] {}", path.display(), d.rule, d.message),
        }
    }
    (errors, warnings)
}

/// ファイル → 行 → 列の順に安定ソートする（ルール間の順序を揃える）
pub fn sort_diagnostics(diags: &mut [Diagnostic]) {
    diags.sort_by(|a, b| {
        (
            &a.rel,
            a.span.map_or((0, 0), |s| (s.start_line, s.start_col)),
        )
            .cmp(&(
                &b.rel,
                b.span.map_or((0, 0), |s| (s.start_line, s.start_col)),
            ))
    });
}
