//! `yuzu new <dir>`: サンプル docs プロジェクトの生成

use std::fs;
use std::path::Path;

use anyhow::{Context, bail};

use crate::out::outln;

/// 生成するファイル一式（scaffold/ からコンパイル時に埋め込む）
const FILES: &[(&str, &str)] = &[
    ("yuzu.toml", include_str!("../../scaffold/yuzu.toml")),
    (".gitignore", include_str!("../../scaffold/gitignore")),
    ("content/index.md", include_str!("../../scaffold/index.md")),
    (
        "content/guide/getting-started.md",
        include_str!("../../scaffold/getting-started.md"),
    ),
    (
        "public/images/yuzu-logo.svg",
        include_str!("../../scaffold/yuzu-logo.svg"),
    ),
    // content の外にある引用先（`file=` / ```include の実例。
    // content/index.md から参照する）
    ("snippets/greet.rs", include_str!("../../scaffold/greet.rs")),
    ("snippets/note.md", include_str!("../../scaffold/note.md")),
    (
        "theme/README.md",
        include_str!("../../scaffold/theme-readme.md"),
    ),
    (
        ".github/workflows/deploy.yml",
        include_str!("../../scaffold/deploy.yml"),
    ),
];

/// 雛形の中で、生成時に yuzu 自身の版へ置き換える印
const VERSION_PLACEHOLDER: &str = "__YUZU_VERSION__";

/// 雛形の印を埋める。deploy.yml のインストールを `yuzu new` した版のタグに
/// 固定するため（版指定なしだと利用者のサイトがデプロイのたびに main の最新で
/// ビルドされ、リリース前の非互換が届く）。版を雛形へ直書きしないのは、
/// バンプコミットを Cargo.toml と Cargo.lock だけにする規律のため
fn fill_placeholders(content: &str) -> String {
    content.replace(VERSION_PLACEHOLDER, env!("CARGO_PKG_VERSION"))
}

pub fn run(cx: &crate::cx::Cx, dir: &Path) -> anyhow::Result<()> {
    // `new` は既存プロジェクトを読まない唯一のコマンドなので `--root` の意味がない。
    // 黙って無視せずエラーにする（未知のキーを黙殺しないという設定側の姿勢と揃える）
    if cx.root().is_some() {
        bail!("yuzu new では --root を指定できません（生成先は位置引数で指定します）");
    }
    if dir.exists() && dir.read_dir()?.next().is_some() {
        bail!(
            "{} は空ではありません（既存ディレクトリを上書きしません）",
            dir.display()
        );
    }

    for (rel, content) in FILES {
        let path = dir.join(rel);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("{} を作成できません", parent.display()))?;
        }
        fs::write(&path, fill_placeholders(content))
            .with_context(|| format!("{} を書き込めません", path.display()))?;
    }

    outln!("✔ {} にサンプルプロジェクトを作成しました", dir.display());
    outln!();
    outln!("次の一歩:");
    outln!("  cd {}", dir.display());
    outln!("  yuzu dev            # 開発サーバ（監視＋WS ライブリロード）で執筆");
    outln!("  yuzu build          # dist/ に静的サイトを出力");
    outln!("  yuzu preview        # dist/ をブラウザで確認");
    outln!();
    outln!("GitHub に push すると Pages へ自動デプロイできます");
    outln!("（.github/workflows/deploy.yml 同梱。Settings > Pages > Source を GitHub Actions に）");
    outln!("GitHub Pages はリポジトリが非公開でも公開されます。社内だけに見せるときは");
    outln!("docs の「社内で公開する」を参照（https://ai.implementer.net/yuzu/guide/internal/）");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{FILES, VERSION_PLACEHOLDER, fill_placeholders};

    /// deploy.yml のインストールは `yuzu new` した版のタグに固定される
    #[test]
    fn deploy_yml_は自分の版のタグでインストールする() {
        let (_, deploy) = FILES
            .iter()
            .find(|(rel, _)| *rel == ".github/workflows/deploy.yml")
            .expect("deploy.yml が雛形にない");
        let filled = fill_placeholders(deploy);
        let tag = format!("--tag v{}", env!("CARGO_PKG_VERSION"));
        assert!(filled.contains(&tag), "{tag} が無い:\n{filled}");
        assert!(filled.contains("cargo install --locked --git"), "{filled}");
        // 公開先のフル URL を渡す（canonical・共有カード・sitemap が出る。Phase 80）
        assert!(
            filled.contains(
                r#"run: yuzu build --base-url "https://${{ steps.pages.outputs.host }}${{ steps.pages.outputs.base_path }}/""#
            ),
            "{filled}"
        );
    }

    /// 印の埋め忘れが無い（雛形のどのファイルにも生成後に印が残らない）
    #[test]
    fn 雛形に印が残らない() {
        for (rel, content) in FILES {
            assert!(
                !fill_placeholders(content).contains(VERSION_PLACEHOLDER),
                "{rel} に {VERSION_PLACEHOLDER} が残っている"
            );
        }
    }
}
