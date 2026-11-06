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

/// 走査エラーを対象パス付きの [`CoreError`] へ詰め替える。
///
/// 握りつぶすと権限拒否やシンボリックリンクのループで**サブツリーが丸ごと
/// 無言で消え、ビルドは成功扱い**になる（ページが消えたのに気づけない）。
/// walkdir はループ検出時に io::Error を持たないので、そこだけ補う
fn walk_error(e: walkdir::Error) -> CoreError {
    let path = e.path().map(Path::to_path_buf).unwrap_or_default();
    CoreError::Io {
        path,
        source: e.into_io_error().unwrap_or_else(|| {
            std::io::Error::other("ディレクトリ走査に失敗しました（シンボリックリンクのループ）")
        }),
    }
}

/// 走査対象そのものが無いのは「0 ページ」であってエラーではない
/// （`content/` を作る前の `yuzu build` を落とさない）
fn is_missing_root(e: &walkdir::Error, root: &Path) -> bool {
    e.io_error()
        .is_some_and(|io| io.kind() == std::io::ErrorKind::NotFound)
        && e.path() == Some(root)
}

/// `.md` の走査で 1 件ごとに決まる扱い（[`scan_markdown_files`] と [`markdown_census`] が共有する）
#[derive(Clone, Copy, PartialEq, Eq)]
enum MarkdownEntry {
    /// ページとして読み込む（draft を含む）
    Load,
    /// `ignore` glob に一致した
    Ignored,
    /// シンボリックリンク（走査はリンクを辿らないので読まない）
    Symlink,
}

/// `content_dir` 以下の `*.md` をパスのソート順で走査し、1 件ごとの扱いを `visit` へ渡す。
/// ページとして読み込むのは通常のファイルだけで、`ignore` glob（相対パス・`/` 区切りで評価）に
/// 一致するものは除外する
fn walk_markdown_files(
    content_dir: &Path,
    ignore: &[String],
    mut visit: impl FnMut(MarkdownEntry, ScannedFile),
) -> Result<(), CoreError> {
    let ignore_set = build_ignore_set(ignore)?;

    for entry in WalkDir::new(content_dir).sort_by_file_name() {
        let entry = match entry {
            Ok(entry) => entry,
            Err(e) if is_missing_root(&e, content_dir) => return Ok(()),
            Err(e) => return Err(walk_error(e)),
        };
        let file_type = entry.file_type();
        if !file_type.is_file() && !file_type.is_symlink() {
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
        let kind = if ignore_set.is_match(crate::urlpath::rel_to_slash(&rel)) {
            tracing::debug!(path = %rel.display(), "ignore パターンに一致したため除外");
            MarkdownEntry::Ignored
        } else if file_type.is_symlink() {
            MarkdownEntry::Symlink
        } else {
            MarkdownEntry::Load
        };
        visit(kind, ScannedFile { abs, rel });
    }
    Ok(())
}

/// `content_dir` 以下の `*.md` をパスのソート順で列挙する。
/// `ignore` glob（相対パス・`/` 区切りで評価）に一致するもの・シンボリックリンクは除外
pub(crate) fn scan_markdown_files(
    content_dir: &Path,
    ignore: &[String],
) -> Result<Vec<ScannedFile>, CoreError> {
    let mut files = Vec::new();
    walk_markdown_files(content_dir, ignore, |kind, file| {
        if kind == MarkdownEntry::Load {
            files.push(file);
        }
    })?;
    Ok(files)
}

/// `content_dir` 以下の `.md` を、ページの読み込みと同じ走査で分けたもの（`content_dir` からの
/// 相対パス。パスのソート順）。ビルドしたページに無い原稿の理由を案内するために使う
/// （v0.20 Phase 91。理由をファイルシステムから別に調べると走査の規則とずれ、シンボリックリンクの
/// `index.md` を draft と案内していた。PR #69 のレビュー指摘）
#[derive(Debug, Default)]
pub struct MarkdownCensus {
    /// ページとして読み込む原稿（draft を含む。ビルドしたページに無ければ draft）
    pub loaded: Vec<PathBuf>,
    /// `input.ignore` に一致して外したもの
    pub ignored: Vec<PathBuf>,
    /// シンボリックリンク（走査はリンクを辿らないので読まない）
    pub symlinks: Vec<PathBuf>,
}

/// [`MarkdownCensus`] を作る
pub(crate) fn markdown_census(
    content_dir: &Path,
    ignore: &[String],
) -> Result<MarkdownCensus, CoreError> {
    let mut census = MarkdownCensus::default();
    walk_markdown_files(content_dir, ignore, |kind, file| {
        let list = match kind {
            MarkdownEntry::Load => &mut census.loaded,
            MarkdownEntry::Ignored => &mut census.ignored,
            MarkdownEntry::Symlink => &mut census.symlinks,
        };
        list.push(file.rel);
    })?;
    Ok(census)
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

    // 隠しディレクトリ（`.git` 等）へは降りない。どうせ下の除外で捨てるので、
    // 走査エラーを拾う面積だけを減らす（ルート自身には述語を当てない
    // = content_dir の名前が `.` 始まりでも走査できる）
    let walk = WalkDir::new(content_dir)
        .sort_by_file_name()
        .into_iter()
        .filter_entry(|e| {
            e.depth() == 0
                || !e.file_name().to_string_lossy().starts_with('.')
                || e.file_type().is_file()
        });
    for entry in walk {
        let entry = match entry {
            Ok(entry) => entry,
            Err(e) if is_missing_root(&e, content_dir) => return Ok(files),
            Err(e) => return Err(walk_error(e)),
        };
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
/// `build.watch_ignore`（プロジェクトルート相対）が**同じ解釈**を共有する。
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
    if stem == "index"
        && let Some(parent) = rel.parent().and_then(|p| p.file_name())
    {
        return parent.to_string_lossy().into_owned();
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

    /// 分類はページの読み込みと同じ走査で、読み込むのは `scan_markdown_files` と同じ集合。
    /// シンボリックリンクはページにならないので分けて数える（PR #69 のレビュー指摘）
    #[cfg(unix)]
    #[test]
    fn markdown_census_は読み込み_ignore_シンボリックリンクに分ける() {
        use super::{markdown_census, scan_markdown_files};
        use std::path::PathBuf;

        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::create_dir_all(root.join("notes")).unwrap();
        std::fs::write(root.join("guide.md"), "# g\n").unwrap();
        std::fs::write(root.join("notes/a.md"), "# a\n").unwrap();
        std::fs::write(root.join("image.png"), "png").unwrap();
        std::os::unix::fs::symlink(root.join("guide.md"), root.join("index.md")).unwrap();
        std::os::unix::fs::symlink(root.join("guide.md"), root.join("notes/b.md")).unwrap();

        let ignore = ["notes/**".to_string()];
        let census = markdown_census(root, &ignore).unwrap();
        let paths = |v: &[&str]| v.iter().map(PathBuf::from).collect::<Vec<_>>();
        assert_eq!(census.loaded, paths(&["guide.md"]));
        // ignore に当たるものはシンボリックリンクでも ignore として数える
        assert_eq!(census.ignored, paths(&["notes/a.md", "notes/b.md"]));
        assert_eq!(census.symlinks, paths(&["index.md"]));
        let scanned: Vec<PathBuf> = scan_markdown_files(root, &ignore)
            .unwrap()
            .into_iter()
            .map(|f| f.rel)
            .collect();
        assert_eq!(scanned, census.loaded);

        // 原稿のディレクトリが無ければ空（scan_markdown_files と同じく 0 件でエラーにしない）
        let census = markdown_census(&root.join("missing"), &[]).unwrap();
        assert!(census.loaded.is_empty() && census.ignored.is_empty());
        assert!(census.symlinks.is_empty());
    }
}
