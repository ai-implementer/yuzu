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
    // ディレクトリの表示名・並び順（無いとサイドバーに `guide` が生名で出る）
    (
        "content/guide/index.md",
        include_str!("../../scaffold/guide-index.md"),
    ),
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
        // Markdown がすでにあるフォルダで始める人を手順へ案内する（v0.20 Phase 91）
        bail!(
            "{} は空ではありません（既存ディレクトリを上書きしません）。\
             Markdown がすでにあるフォルダで始めるときは、yuzu.toml を置いて input.dir に\
             原稿のディレクトリを指定します（https://ai.implementer.net/yuzu/guide/install/ の\
             「既存のフォルダで始める」）",
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
    outln!("  yuzu dev            # 開発サーバで執筆（保存するとブラウザが自動で再読み込み）");
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

    /// deploy.yml は `yuzu new` した版のリリースを取得し、SHA256SUMS で照合してから使う
    #[test]
    fn deploy_yml_は自分の版のリリースを照合してから使う() {
        let (_, deploy) = FILES
            .iter()
            .find(|(rel, _)| *rel == ".github/workflows/deploy.yml")
            .expect("deploy.yml が雛形にない");
        let filled = fill_placeholders(deploy);
        let version = format!("YUZU_VERSION: {}", env!("CARGO_PKG_VERSION"));
        assert!(filled.contains(&version), "{version} が無い:\n{filled}");
        // release.yml のアーカイブ名と同じ形（yuzu-v<版>-<target>.tar.gz）
        assert!(
            filled.contains(r#"archive="yuzu-v${YUZU_VERSION}-x86_64-unknown-linux-gnu.tar.gz""#),
            "{filled}"
        );
        assert!(filled.contains("| sha256sum -c -"), "{filled}");
        // ソースから入れる代わりの手順も同じ版のタグを指す
        let tag = format!("--tag v{} yuzu-cli", env!("CARGO_PKG_VERSION"));
        assert!(filled.contains(&tag), "{tag} が無い:\n{filled}");
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

    /// 空でないフォルダでは止め、既存のフォルダで始める手順へ案内する（v0.20 Phase 91）
    #[test]
    fn 空でないフォルダでは止めて手順を案内する() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("README.md"), "# 既存\n").unwrap();
        let cx = crate::cx::Cx::new(None).unwrap();
        let err = super::run(&cx, dir.path()).unwrap_err().to_string();
        assert!(err.contains("空ではありません"), "{err}");
        assert!(err.contains("「既存のフォルダで始める」"), "{err}");
        assert!(!dir.path().join("yuzu.toml").exists());
    }
}
