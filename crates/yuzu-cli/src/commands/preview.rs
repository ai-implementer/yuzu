//! `yuzu preview [--port]`: dist/ の配信

use std::net::IpAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, bail};

use yuzu_config::ResolvedConfig;
use yuzu_server::{PathGuard, ServeOptions};

pub fn run(port: Option<u16>, host: Option<String>) -> anyhow::Result<()> {
    let (_, mut rc) = super::load_project()?;
    // --host は dev.host の設定より優先（コンテナ内から 0.0.0.0 で配信する用途）
    if let Some(host) = host {
        rc.config.dev.host = host;
    }

    if !rc.output_dir.is_dir() {
        bail!(
            "{} がありません。先に `yuzu build` を実行してください",
            rc.output_dir.display()
        );
    }
    serve_dist(&rc, port)
}

/// dist/ を配信する（`preview` と `build --watch` で共用。ブロッキング）
pub(crate) fn serve_dist(rc: &ResolvedConfig, port: Option<u16>) -> anyhow::Result<()> {
    let host: IpAddr = rc
        .config
        .dev
        .host
        .parse()
        .with_context(|| format!("dev.host が不正です: {}", rc.config.dev.host))?;

    yuzu_server::serve(ServeOptions {
        dir: rc.output_dir.clone(),
        host,
        port: port.unwrap_or(rc.config.dev.port),
        base_url: rc.base_url.clone(),
        live_reload: None,
        path_guard: Some(symlink_guard(&rc.root)),
    })?;
    Ok(())
}

/// 配信パスの検査述語（`preview` / `dev` / `build --watch` で共用）。
/// 書き側（`write_under` / 孤児掃除 / render 冒頭の `ensure_no_symlink_under`）が
/// 拒否するのと同じ規律で読み側もリンクを辿らない（server は core を知らないので
/// cli が包む）。
///
/// 起点は**プロジェクトルート**（書き側と同じ）。出力ディレクトリを起点にすると、
/// `output.dir = "alias/site"` の `alias` のような**出力先までの中間ディレクトリ**の
/// リンクを見逃し、build が拒否する構成を preview だけが配信してしまう
pub(crate) fn symlink_guard(project_root: &Path) -> PathGuard {
    let root: PathBuf = project_root.to_path_buf();
    Arc::new(move |path: &Path| {
        yuzu_core::output::ensure_symlink_free(&root, path).map_err(|e| e.to_string())
    })
}

#[cfg(test)]
mod tests {
    use super::symlink_guard;

    /// 出力先までの中間ディレクトリ（`root/alias -> outside`）のリンクも拒否する。
    /// build は `ensure_no_symlink_under(root, output_dir)` で同じ構成を拒否するので、
    /// preview だけが配信できてはいけない
    #[cfg(unix)]
    #[test]
    fn 出力先までの中間ディレクトリのリンクも拒否する() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("root");
        let outside = tmp.path().join("outside");
        std::fs::create_dir_all(root.join("dist")).unwrap();
        std::fs::create_dir_all(outside.join("site")).unwrap();
        std::fs::write(outside.join("site/index.html"), "<html>leaked</html>").unwrap();
        std::fs::write(root.join("dist/index.html"), "<html>home</html>").unwrap();
        std::os::unix::fs::symlink(&outside, root.join("alias")).unwrap();

        let guard = symlink_guard(&root);
        // output.dir = "alias/site" 相当: 要求ファイルまでの経路に alias（リンク）がある
        let err = guard(&root.join("alias/site/index.html")).unwrap_err();
        assert!(err.contains("alias"), "{err}");
        // 配信ディレクトリ自身（`/` の要求で metadata を引く）も同様
        assert!(guard(&root.join("alias/site")).is_err());
        // 実体の出力先は通る（ディレクトリ自身・配下のファイル・未存在のパス）
        assert!(guard(&root.join("dist")).is_ok());
        assert!(guard(&root.join("dist/index.html")).is_ok());
        assert!(guard(&root.join("dist/no-such/index.html")).is_ok());
        // プロジェクト外は起点の外なので拒否
        assert!(guard(&outside.join("site/index.html")).is_err());
    }
}
