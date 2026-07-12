//! 静的アセットの書き出し:
//! テーマアセット（埋め込み→ `theme/static` 上書き）・`public/` パススルー・build_id
//!
//! 書き込みはすべて「内容一致ならスキップ」（mtime 温存）。インクリメンタルビルド時は
//! [`OutputTracker`] に dist 相対パスを記録し、孤児掃除マニフェストの材料にする

use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use walkdir::WalkDir;
use yuzu_core::OutputTracker;

use crate::error::RenderError;

/// dist 相対パスへ書き出す（tracker があれば記録、なければ直接書き込み）
pub(crate) fn write_output(
    outputs: Option<&OutputTracker>,
    output_dir: &Path,
    rel: &str,
    data: &[u8],
) -> Result<(), RenderError> {
    match outputs {
        Some(tracker) => {
            tracker
                .write(rel, data)
                .map_err(RenderError::io(output_dir.join(rel)))?;
        }
        None => write_file(&output_dir.join(rel), data)?,
    }
    Ok(())
}

/// テーマの静的アセットを `dist/_assets/` へ書き出す。
/// 埋め込みデフォルトテーマを先に書き、プロジェクト `theme/static/` を上書きコピーする
pub(crate) fn write_theme_assets(
    output_dir: &Path,
    theme_dir: Option<&Path>,
    outputs: Option<&OutputTracker>,
) -> Result<(), RenderError> {
    for path in yuzu_theme::iter() {
        let Some(rest) = path.strip_prefix("static/") else {
            continue;
        };
        let data = yuzu_theme::get(&path).expect("iter() で列挙したパスは必ず存在する");
        write_output(outputs, output_dir, &format!("_assets/{rest}"), &data)?;
    }

    if let Some(theme_static) = theme_dir.map(|d| d.join("static")) {
        if theme_static.is_dir() {
            copy_tree(&theme_static, "_assets", output_dir, outputs)?;
        }
    }
    Ok(())
}

/// `public/` を `dist/` 直下へそのままコピーする
pub(crate) fn copy_public(
    public_dir: Option<&Path>,
    output_dir: &Path,
    outputs: Option<&OutputTracker>,
) -> Result<(), RenderError> {
    if let Some(dir) = public_dir {
        copy_tree(dir, "", output_dir, outputs)?;
    }
    Ok(())
}

/// オートリフレッシュ用のビルド ID を `dist/__yuzu/build_id` に書く。
/// HTML には埋め込まない（通常ビルドの出力を決定的に保つため）。
/// 内容が毎回変わるため常に書き込まれる（--watch のポーリング変更シグナル）
pub(crate) fn write_build_id(
    output_dir: &Path,
    outputs: Option<&OutputTracker>,
) -> Result<(), RenderError> {
    let id = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos().to_string())
        .unwrap_or_else(|_| "0".to_string());
    write_output(outputs, output_dir, "__yuzu/build_id", id.as_bytes())
}

/// `src` 配下を dist の `dest_prefix/`（"" なら直下）へ再帰コピーする
fn copy_tree(
    src: &Path,
    dest_prefix: &str,
    output_dir: &Path,
    outputs: Option<&OutputTracker>,
) -> Result<(), RenderError> {
    for entry in WalkDir::new(src).into_iter().filter_map(Result::ok) {
        if !entry.file_type().is_file() {
            continue;
        }
        let rel = entry
            .path()
            .strip_prefix(src)
            .expect("walkdir は src 配下のみ返す");
        let rel = rel
            .iter()
            .map(|c| c.to_string_lossy())
            .collect::<Vec<_>>()
            .join("/");
        let dest_rel = if dest_prefix.is_empty() {
            rel
        } else {
            format!("{dest_prefix}/{rel}")
        };
        let data = fs::read(entry.path()).map_err(RenderError::io(entry.path()))?;
        write_output(outputs, output_dir, &dest_rel, &data)?;
    }
    Ok(())
}

/// 絶対パスへの書き出し（内容一致ならスキップ = mtime 温存）
pub(crate) fn write_file(path: &Path, data: &[u8]) -> Result<(), RenderError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(RenderError::io(parent))?;
    }
    yuzu_core::output::write_if_changed(path, data).map_err(RenderError::io(path))?;
    Ok(())
}
