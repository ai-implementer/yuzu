//! `content/` の走査と route（出力 URL）の決定

use std::path::{Path, PathBuf};

use globset::{Glob, GlobSet, GlobSetBuilder};
use walkdir::WalkDir;

use crate::error::CoreError;

pub(crate) struct ScannedFile {
    pub abs: PathBuf,
    /// `content_dir` からの相対パス
    pub rel: PathBuf,
}

/// `content_dir` 以下の `*.md` をパスのソート順で列挙する。
/// `ignore` glob（相対パス・`/` 区切りで評価）に一致するものは除外
pub(crate) fn scan_markdown_files(
    content_dir: &Path,
    ignore: &[String],
) -> Result<Vec<ScannedFile>, CoreError> {
    let ignore_set = build_ignore_set(ignore)?;
    let mut files = Vec::new();

    for entry in WalkDir::new(content_dir)
        .sort_by_file_name()
        .into_iter()
        .filter_map(Result::ok)
    {
        if !entry.file_type().is_file() {
            continue;
        }
        let abs = entry.path().to_path_buf();
        if abs.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }
        let rel = abs
            .strip_prefix(content_dir)
            .expect("walkdir は content_dir 配下のみ返す")
            .to_path_buf();
        if ignore_set.is_match(crate::urlpath::rel_to_slash(&rel)) {
            tracing::debug!(path = %rel.display(), "ignore パターンに一致したため除外");
            continue;
        }
        files.push(ScannedFile { abs, rel });
    }
    Ok(files)
}

/// `content_dir` 以下の `.md` 以外のファイル（ページ横の画像等の同伴アセット）を
/// パスのソート順で列挙する。`ignore` glob の評価は [`scan_markdown_files`] と同一。
/// 隠しファイル（`.` 始まりの構成要素を含むパス。`.DS_Store` やエディタの
/// 管理ディレクトリ等）は既定で除外する
pub(crate) fn scan_content_assets(
    content_dir: &Path,
    ignore: &[String],
) -> Result<Vec<ScannedFile>, CoreError> {
    let ignore_set = build_ignore_set(ignore)?;
    let mut files = Vec::new();

    for entry in WalkDir::new(content_dir)
        .sort_by_file_name()
        .into_iter()
        .filter_map(Result::ok)
    {
        if !entry.file_type().is_file() {
            continue;
        }
        let abs = entry.path().to_path_buf();
        if abs.extension().and_then(|e| e.to_str()) == Some("md") {
            continue;
        }
        let rel = abs
            .strip_prefix(content_dir)
            .expect("walkdir は content_dir 配下のみ返す")
            .to_path_buf();
        if rel.iter().any(|c| c.to_string_lossy().starts_with('.')) {
            continue;
        }
        if ignore_set.is_match(crate::urlpath::rel_to_slash(&rel)) {
            tracing::debug!(path = %rel.display(), "ignore パターンに一致したため除外");
            continue;
        }
        files.push(ScannedFile { abs, rel });
    }
    Ok(files)
}

fn build_ignore_set(patterns: &[String]) -> Result<GlobSet, CoreError> {
    let mut builder = GlobSetBuilder::new();
    for pattern in patterns {
        let glob = Glob::new(pattern).map_err(|e| CoreError::InvalidIgnorePattern {
            pattern: pattern.clone(),
            message: e.to_string(),
        })?;
        builder.add(glob);
    }
    builder
        .build()
        .map_err(|e| CoreError::InvalidIgnorePattern {
            pattern: patterns.join(", "),
            message: e.to_string(),
        })
}

/// 相対パス → route（pretty URL、末尾スラッシュ付きサイト相対パス）。
///
/// - `index.md` → `""`
/// - `guide/getting-started.md` → `"guide/getting-started/"`
/// - `guide/index.md` → `"guide/"`
pub(crate) fn route_for_rel(rel: &Path) -> String {
    let mut parts: Vec<String> = rel
        .iter()
        .map(|c| c.to_string_lossy().into_owned())
        .collect();
    let file = parts.pop().unwrap_or_default();
    let stem = file.strip_suffix(".md").unwrap_or(&file);
    if stem != "index" {
        parts.push(stem.to_string());
    }
    if parts.is_empty() {
        String::new()
    } else {
        parts.join("/") + "/"
    }
}

/// タイトルの最終フォールバック: ファイル名の stem（`index.md` は親ディレクトリ名）
pub(crate) fn stem_title(rel: &Path) -> String {
    let stem = rel
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    if stem == "index" {
        if let Some(parent) = rel.parent().and_then(|p| p.file_name()) {
            return parent.to_string_lossy().into_owned();
        }
    }
    stem
}

#[cfg(test)]
mod tests {
    use super::{route_for_rel, stem_title};
    use std::path::Path;

    #[test]
    fn route_の決定() {
        assert_eq!(route_for_rel(Path::new("index.md")), "");
        assert_eq!(
            route_for_rel(Path::new("guide/getting-started.md")),
            "guide/getting-started/"
        );
        assert_eq!(route_for_rel(Path::new("guide/index.md")), "guide/");
        assert_eq!(route_for_rel(Path::new("a/b/c.md")), "a/b/c/");
    }

    #[test]
    fn stem_title_のフォールバック() {
        assert_eq!(
            stem_title(Path::new("getting-started.md")),
            "getting-started"
        );
        assert_eq!(stem_title(Path::new("guide/index.md")), "guide");
        assert_eq!(stem_title(Path::new("index.md")), "index");
    }
}
