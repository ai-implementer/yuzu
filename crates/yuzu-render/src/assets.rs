//! 静的アセットの書き出し:
//! テーマアセット（埋め込み→ `theme/static` 上書き）・`public/` パススルー・build_id

use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use walkdir::WalkDir;

use crate::error::RenderError;

/// テーマの静的アセットを `dist/_assets/` へ書き出す。
/// 埋め込みデフォルトテーマを先に書き、プロジェクト `theme/static/` を上書きコピーする
pub(crate) fn write_theme_assets(
    output_dir: &Path,
    theme_dir: Option<&Path>,
) -> Result<(), RenderError> {
    let assets_dir = output_dir.join("_assets");

    for path in yuzu_theme::iter() {
        let Some(rest) = path.strip_prefix("static/") else {
            continue;
        };
        let data = yuzu_theme::get(&path).expect("iter() で列挙したパスは必ず存在する");
        let dest = assets_dir.join(rest);
        write_file(&dest, &data)?;
    }

    if let Some(theme_static) = theme_dir.map(|d| d.join("static")) {
        if theme_static.is_dir() {
            copy_tree(&theme_static, &assets_dir)?;
        }
    }
    Ok(())
}

/// `public/` を `dist/` 直下へそのままコピーする
pub(crate) fn copy_public(public_dir: Option<&Path>, output_dir: &Path) -> Result<(), RenderError> {
    if let Some(dir) = public_dir {
        copy_tree(dir, output_dir)?;
    }
    Ok(())
}

/// オートリフレッシュ用のビルド ID を `dist/__yuzu/build_id` に書く。
/// HTML には埋め込まない（通常ビルドの出力を決定的に保つため）
pub(crate) fn write_build_id(output_dir: &Path) -> Result<(), RenderError> {
    let id = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos().to_string())
        .unwrap_or_else(|_| "0".to_string());
    write_file(&output_dir.join("__yuzu/build_id"), id.as_bytes())
}

fn copy_tree(src: &Path, dest: &Path) -> Result<(), RenderError> {
    for entry in WalkDir::new(src).into_iter().filter_map(Result::ok) {
        if !entry.file_type().is_file() {
            continue;
        }
        let rel = entry
            .path()
            .strip_prefix(src)
            .expect("walkdir は src 配下のみ返す");
        let data = fs::read(entry.path()).map_err(RenderError::io(entry.path()))?;
        write_file(&dest.join(rel), &data)?;
    }
    Ok(())
}

pub(crate) fn write_file(path: &Path, data: &[u8]) -> Result<(), RenderError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(RenderError::io(parent))?;
    }
    fs::write(path, data).map_err(RenderError::io(path))
}
