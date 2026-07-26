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

/// glob パターン集合のマッチャ。`input.ignore`（content 相対）と
/// `build.watchIgnore`（プロジェクトルート相対）が**同じ解釈**を共有する。
/// globset を公開 API へ露出させないための薄いラッパでもある
pub struct IgnoreMatcher(GlobSet);

impl IgnoreMatcher {
    pub fn new(patterns: &[String]) -> Result<Self, CoreError> {
        build_ignore_set(patterns).map(Self)
    }

    /// 相対パスが除外に当たるか（`/` 区切りへ正規化して判定する）
    pub fn is_match(&self, rel: &std::path::Path) -> bool {
        self.0.is_match(crate::urlpath::rel_to_slash(rel))
    }

    /// 相対パス自身か**祖先ディレクトリのどれか**が除外に当たるか。
    ///
    /// 「当たったディレクトリの配下はすべて除外」の意味になる（`**/target` で
    /// `target/debug/x` も除外される）。ファイル監視ではディレクトリの作成
    /// イベント自体も飛んでくるため、`**/target/**` のような「配下だけ」の
    /// パターンでは `target` の作成を取りこぼす
    pub fn is_match_or_ancestor(&self, rel: &std::path::Path) -> bool {
        rel.ancestors()
            .filter(|p| !p.as_os_str().is_empty())
            .any(|p| self.is_match(p))
    }
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
    use super::{IgnoreMatcher, route_for_rel, stem_title};
    use std::path::Path;

    #[test]
    fn 祖先マッチはディレクトリ配下をすべて除外する() {
        let m = IgnoreMatcher::new(&["**/target".to_string()]).unwrap();
        // ディレクトリ自身（監視のディレクトリ作成イベント）
        assert!(m.is_match_or_ancestor(Path::new("target")));
        assert!(m.is_match_or_ancestor(Path::new("crates/x/target")));
        // 配下のファイル
        assert!(m.is_match_or_ancestor(Path::new("target/debug/yuzu")));
        // 名前が前方一致するだけのパスは除外しない
        assert!(!m.is_match_or_ancestor(Path::new("content/target.md")));
        assert!(!m.is_match_or_ancestor(Path::new("targets/x")));
        // is_match 単体は祖先を見ない（input.ignore の従来の意味）
        assert!(!m.is_match(Path::new("target/debug/yuzu")));
    }

    #[test]
    fn 配下だけのパターンでもファイルには当たる() {
        let m = IgnoreMatcher::new(&["**/target/**".to_string()]).unwrap();
        assert!(m.is_match_or_ancestor(Path::new("target/debug/yuzu")));
        // このパターンはディレクトリ自身には当たらない（既定値が `**/target` の理由）
        assert!(!m.is_match_or_ancestor(Path::new("target")));
    }

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
