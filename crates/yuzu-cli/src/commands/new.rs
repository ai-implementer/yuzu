//! `yuzu new <dir>`: サンプル docs プロジェクトの生成

use std::fs;
use std::path::Path;

use anyhow::{Context, bail};

/// 生成するファイル一式（scaffold/ からコンパイル時に埋め込む）
const FILES: &[(&str, &str)] = &[
    ("yuzu.jsonc", include_str!("../../scaffold/yuzu.jsonc")),
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
    (
        "theme/README.md",
        include_str!("../../scaffold/theme-readme.md"),
    ),
    (
        ".github/workflows/deploy.yml",
        include_str!("../../scaffold/deploy.yml"),
    ),
];

pub fn run(dir: &Path) -> anyhow::Result<()> {
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
        fs::write(&path, content)
            .with_context(|| format!("{} を書き込めません", path.display()))?;
    }

    println!("✔ {} にサンプルプロジェクトを作成しました", dir.display());
    println!();
    println!("次の一歩:");
    println!("  cd {}", dir.display());
    println!("  yuzu dev            # 開発サーバ（監視＋WS ライブリロード）で執筆");
    println!("  yuzu build          # dist/ に静的サイトを出力");
    println!("  yuzu preview        # dist/ をブラウザで確認");
    println!();
    println!("GitHub に push すると Pages へ自動デプロイできます");
    println!(
        "（.github/workflows/deploy.yml 同梱。Settings > Pages > Source を GitHub Actions に）"
    );
    Ok(())
}
