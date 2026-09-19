//! `yuzu completions <shell>`: シェル補完スクリプトを標準出力へ出す。
//!
//! clap の定義（`Cli::command()`）から**実行時に**生成するので、サブコマンドや
//! オプションを足せば自動で追随する。リポジトリにもリリースアセットにも同梱しない
//! （同梱すると変更のたびに再生成が要り、バイナリとずれる）。
//! 動的補完（`--section` にセクション名を出す等）は clap_complete の
//! `unstable-dynamic` 扱いなので使わず、静的な候補（サブコマンド・フラグ・
//! `--format` の値など）までにする。

use anyhow::bail;
use clap::CommandFactory;
use clap_complete::Shell;

pub fn run(cx: &crate::cx::Cx, shell: Shell) -> anyhow::Result<()> {
    // プロジェクトを読まないので `--root` の意味がない。`new` と同じ規律で黙殺しない
    if cx.root().is_some() {
        bail!("yuzu completions では --root を指定できません");
    }
    crate::out::str(&generate(shell));
    Ok(())
}

/// 補完スクリプトを文字列で返す（標準出力への書き出しと分けてテストで中身を見る）
pub(crate) fn generate(shell: Shell) -> String {
    let mut buf = Vec::new();
    clap_complete::generate(shell, &mut crate::cli::Cli::command(), "yuzu", &mut buf);
    String::from_utf8_lossy(&buf).into_owned()
}

#[cfg(test)]
mod tests {
    use super::generate;
    use clap::ValueEnum;
    use clap_complete::Shell;

    /// 対応する全シェルで生成でき、サブコマンドとグローバル引数が候補に入る。
    /// グローバル引数（`--quiet`）を見るのは「clap の定義から生成している」ことの
    /// 確認 = 手書きの補完ではないので、オプションを足したときに黙って欠けない
    #[test]
    fn 全シェルでサブコマンドとグローバル引数を含む() {
        for shell in Shell::value_variants() {
            let script = generate(*shell);
            for needle in ["completions", "search", "quiet"] {
                assert!(
                    script.contains(needle),
                    "{shell}: `{needle}` が補完スクリプトに無い"
                );
            }
        }
    }

    /// `--format` の値（human / json）も候補になる（value_enum の効果）
    #[test]
    fn 値の候補も含む() {
        let script = generate(Shell::Bash);
        assert!(script.contains("human"), "{script}");
        assert!(script.contains("json"), "{script}");
    }

    /// bash の補完関数名は clap_complete の慣例（`_yuzu`）で、複数バイナリと衝突しない
    #[test]
    fn bash_の補完関数名() {
        assert!(generate(Shell::Bash).contains("_yuzu()"));
    }
}
