//! URL・相対パスの純関数ヘルパ。
//! yuzu-render の URL 書き換えと linkcheck（`yuzu check`）で共用する

use std::path::Path;

/// `?query` / `#fragment` を切り離す
pub fn split_suffix(url: &str) -> (&str, &str) {
    match url.find(['?', '#']) {
        Some(i) => (&url[..i], &url[i..]),
        None => (url, ""),
    }
}

/// 相対パスを `dir`（`/` 区切り、空 = ルート）基準で解決し、`/` 区切りに正規化する
pub fn resolve_relative(dir: &str, target: &str) -> String {
    let mut parts: Vec<&str> = if dir.is_empty() {
        Vec::new()
    } else {
        dir.split('/').collect()
    };
    for seg in target.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            s => parts.push(s),
        }
    }
    parts.join("/")
}

/// 相対パスを `/` 区切りの文字列へ正規化する（Windows でも出力 URL を安定させる）
pub fn rel_to_slash(rel: &Path) -> String {
    rel.iter()
        .map(|c| c.to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn suffix_の分離() {
        assert_eq!(split_suffix("a.md#frag"), ("a.md", "#frag"));
        assert_eq!(split_suffix("a.md?q=1#f"), ("a.md", "?q=1#f"));
        assert_eq!(split_suffix("a.md"), ("a.md", ""));
    }

    #[test]
    fn 相対解決() {
        assert_eq!(resolve_relative("guide", "./index.md"), "guide/index.md");
        assert_eq!(resolve_relative("guide", "../index.md"), "index.md");
        assert_eq!(resolve_relative("", "a/b.md"), "a/b.md");
        assert_eq!(resolve_relative("a/b", "../../c.md"), "c.md");
    }

    #[test]
    fn スラッシュ区切りへの正規化() {
        assert_eq!(rel_to_slash(Path::new("guide/index.md")), "guide/index.md");
    }
}
