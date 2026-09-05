//! URL・相対パスの純関数ヘルパ。
//! yuzu-render の URL 書き換えと linkcheck（`yuzu check`）で共用する。
//!
//! **route → URL のパーセントエンコードはここが唯一の変換点**（[`encode_path`]）。
//! 「ディスクは生・URL はエンコード」の境界で、`Page.route` や route をキーにした
//! HashMap（nav / pager / breadcrumbs / llms / linkcheck / 検索のグループ）は
//! 生のまま持ち、**表示・書き出しの直前だけ**エンコードする（エンコード済み文字列を
//! キーに混ぜると診断の出ないズレになる）。逆方向の [`percent_decode`] は
//! 著者が書いた参照（`my%20page.md` / `/%E8%A8%AD…/` / aliases）を生の route や
//! ファイル名へ戻して照合するために使う。

use std::fmt::Write as _;
use std::path::Path;

/// URL のパスセグメントでエンコードせずに置く文字（英数字を除く）。
///
/// RFC 3986 の unreserved（`-._~`）と、パスセグメントに現れても HTML 属性・
/// Markdown のリンク先・URL 構文のどれも壊さない sub-delims / `:` `@`。
/// `'` `(` `)` `&` は RFC 上は許されるが、属性の引用符・CommonMark の
/// リンク先（括弧の対応）・実体参照の文脈を壊すのでエンコード側に回す
const PATH_SAFE: &[u8] = b"-._~!$*+,;=:@";

/// route / 相対パス（`/` 区切り）をパーセントエンコードした URL パスにする。
///
/// セグメント区切りの `/` はそのまま、それ以外は英数字と [`PATH_SAFE`] を除いて
/// UTF-8 のバイト単位で `%XX`（大文字）にする。`%` 自身も `%25` になるので、
/// `a%23b.md` の URL は `/a%2523b/` ＝ サーバがデコードすると物理パス `a%23b/` に
/// 一致する（**入力は常に生の文字列**。エンコード済みを渡すと二重になる）。
///
/// comrak の `escape_href` は `%XX` を素通しするので、本文リンクへ埋めた結果と
/// テンプレートへ渡した結果は同じバイト列になる（非 ASCII は comrak も
/// エンコードするため、ここで揃えないと本文とナビで表記が食い違う）
pub fn encode_path(path: &str) -> String {
    let mut out = String::with_capacity(path.len());
    for b in path.bytes() {
        if b == b'/' || b.is_ascii_alphanumeric() || PATH_SAFE.contains(&b) {
            out.push(b as char);
        } else {
            let _ = write!(out, "%{b:02X}");
        }
    }
    out
}

/// `%XX` の最小限デコード（不正な並びはそのまま残す。新規依存を避ける）。
///
/// UTF-8 として不正なバイト列は U+FFFD に置き換える。`+` は空白にしない
/// （それはクエリのフォームエンコードの規則で、パスには適用されない）
pub fn percent_decode(s: &str) -> String {
    if !s.contains('%') {
        return s.to_string();
    }
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hi = (bytes[i + 1] as char).to_digit(16);
            let lo = (bytes[i + 2] as char).to_digit(16);
            if let (Some(hi), Some(lo)) = (hi, lo) {
                out.push((hi * 16 + lo) as u8);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

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

/// 合成ページ（用語集・検索結果）の route 元 → content 相対 `.md` パス。
///
/// route が空・パスが不正（絶対パス / ドライブレター / `..` / 空セグメント）なら
/// `None` ＝ページを作らない（設定の書き間違いでビルドを止めず、かつルート外へは
/// 絶対に書かない）。`..` を弾くのは `content_dir.join(rel)` が外へ出ないため
pub(crate) fn synth_page_rel(raw: &str) -> Option<std::path::PathBuf> {
    let raw = raw.trim().trim_matches('/');
    if raw.is_empty() || raw.starts_with('\\') || raw.contains(':') {
        return None;
    }
    let segments: Vec<&str> = raw.split('/').collect();
    if segments
        .iter()
        .any(|s| s.is_empty() || *s == "." || *s == "..")
    {
        return None;
    }
    Some(std::path::PathBuf::from(format!("{raw}.md")))
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

    #[test]
    fn encode_path_は英数字と安全な記号を残す() {
        assert_eq!(
            encode_path("guide/getting-started/"),
            "guide/getting-started/"
        );
        assert_eq!(encode_path("a-b_c.d~e/"), "a-b_c.d~e/");
        assert_eq!(encode_path("v1.2+x,y;z=w:@!$*/"), "v1.2+x,y;z=w:@!$*/");
        assert_eq!(encode_path(""), "");
    }

    #[test]
    fn encode_path_は非_ascii_を_utf8_バイト単位でエンコードする() {
        assert_eq!(
            encode_path("設計/概要/"),
            "%E8%A8%AD%E8%A8%88/%E6%A6%82%E8%A6%81/"
        );
    }

    #[test]
    fn encode_path_は_url_と_html_で意味を持つ文字をエンコードする() {
        // `#` `?` は URL 構文、`%` は二重解釈、引用符・山括弧は属性、`( ) &` は
        // Markdown のリンク先と実体参照を壊す
        assert_eq!(encode_path("a#b/"), "a%23b/");
        assert_eq!(encode_path("a?b/"), "a%3Fb/");
        assert_eq!(encode_path("a%23b/"), "a%2523b/");
        assert_eq!(
            encode_path(r#"a"b'c<d>e`f\g/"#),
            "a%22b%27c%3Cd%3Ee%60f%5Cg/"
        );
        assert_eq!(encode_path("a b/"), "a%20b/");
        assert_eq!(encode_path("f(x)&y/"), "f%28x%29%26y/");
        assert_eq!(encode_path("a[b]{c}|d^e/"), "a%5Bb%5D%7Bc%7D%7Cd%5Ee/");
        assert_eq!(encode_path("a\tb/"), "a%09b/");
    }

    #[test]
    fn percent_decode_の基本() {
        assert_eq!(percent_decode("%E8%A6%8B%E5%87%BA%E3%81%97"), "見出し");
        assert_eq!(percent_decode("plain"), "plain");
        assert_eq!(percent_decode("a%2Gb"), "a%2Gb", "不正な 16 進はそのまま");
        assert_eq!(percent_decode("%"), "%", "末尾の % もそのまま");
        assert_eq!(percent_decode("a%2"), "a%2", "桁が足りない % もそのまま");
        assert_eq!(percent_decode("a+b"), "a+b", "+ は空白にしない");
        assert_eq!(percent_decode("%FF"), "\u{FFFD}", "不正な UTF-8 は置換文字");
    }

    #[test]
    fn encode_と_decode_は往復する() {
        for raw in ["設計/概 要#1/", "a%23b/", "f(x)&y/", "guide/x/"] {
            assert_eq!(percent_decode(&encode_path(raw)), raw, "{raw}");
        }
    }
}
