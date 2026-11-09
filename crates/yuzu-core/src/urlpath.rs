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

/// 外部参照か（スキーム付き URL・`mailto:`・`tel:`）。
/// render の書き換え（`UrlResolver::rewrite`）と linkcheck の「触らない」判定が
/// 共有する唯一の定義
pub fn is_external_url(url: &str) -> bool {
    has_url_scheme(url) || url.starts_with("mailto:") || url.starts_with("tel:")
}

/// 先頭が `scheme://` のフル URL か。スキームは RFC 3986 の形（英字で始まり、英数字・`+`・`-`・`.`）。
/// 文字列のどこかに `://` があるかでは判定しない = `/search/?q=https://example.com/` は
/// クエリに URL を含むサイト内リンク（PR #66 のレビュー指摘。base が付かなかった）
pub fn has_url_scheme(url: &str) -> bool {
    let Some((scheme, _)) = url.split_once("://") else {
        return false;
    };
    let mut chars = scheme.chars();
    chars.next().is_some_and(|c| c.is_ascii_alphabetic())
        && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
}

/// `yuzu check --external-links` の検査対象か（`http://` / `https://` だけ。
/// スキームは大文字小文字を区別しない）
pub fn is_http_url(url: &str) -> bool {
    let scheme = match url.find("://") {
        Some(i) => &url[..i],
        None => return false,
    };
    scheme.eq_ignore_ascii_case("http") || scheme.eq_ignore_ascii_case("https")
}

/// `?query` / `#fragment` を切り離す
pub fn split_suffix(url: &str) -> (&str, &str) {
    match url.find(['?', '#']) {
        Some(i) => (&url[..i], &url[i..]),
        None => (url, ""),
    }
}

/// 本文のリンクの URL の分類。リンク検査（`linkcheck::check_one`）と描画（yuzu-render の
/// `UrlResolver::rewrite`）が同じ分類を網羅 match で使う。以前は両方が同じ 6 つの分岐を同じ順に
/// 書き、一致をコメントで縛っていた（順序のずれで分類が食い違ったことがある。10-12 レビューの R5）
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinkTarget<'a> {
    /// 空の URL
    Empty,
    /// 同じページの見出し（`#frag`。値は `#` の後ろ）
    Fragment(&'a str),
    /// 外部（スキーム付き・`mailto:` 等。[`is_external_url`]）
    External,
    /// ルート絶対（`/foo`）。`rest` は先頭の `/` の後ろで**デコード前**（描画は書いたまま使い、
    /// 検査はデコードして照合する）。分類はデコード前に行う（`%2Flogo.png` は相対参照）
    Absolute { rest: &'a str, suffix: &'a str },
    /// 相対の `.md`（ページへのリンク）。`path` はデコード済み
    Page { path: String, suffix: &'a str },
    /// 末尾のセグメントに `.` を含む相対参照（画像・添付等。content の同伴アセット）。
    /// `path` はデコード済み
    Asset { path: String, suffix: &'a str },
    /// それ以外の相対参照（`guide/` のようなディレクトリ風。配信の形に依るので検査も書き換えもしない）
    Other,
}

impl<'a> LinkTarget<'a> {
    /// `?query` / `#fragment` のうちフラグメント（`#` の後ろ）
    pub fn fragment(suffix: &'a str) -> Option<&'a str> {
        suffix.split_once('#').map(|(_, f)| f)
    }
}

/// 本文のリンクの URL を分類する（[`LinkTarget`]）。`?` / `#` 以降の suffix はデコードしない
pub fn classify_link(url: &str) -> LinkTarget<'_> {
    if url.is_empty() {
        return LinkTarget::Empty;
    }
    if let Some(frag) = url.strip_prefix('#') {
        return LinkTarget::Fragment(frag);
    }
    if is_external_url(url) {
        return LinkTarget::External;
    }
    let (path, suffix) = split_suffix(url);
    if let Some(rest) = path.strip_prefix('/') {
        return LinkTarget::Absolute { rest, suffix };
    }
    // content 相対の参照は、著者がエンコード済みで書いた形（`my%20page.md`）を生のファイル名へ戻す
    let path = percent_decode(path);
    if path.ends_with(".md") {
        return LinkTarget::Page { path, suffix };
    }
    let last = path.rsplit('/').next().unwrap_or(&path);
    if last.contains('.') {
        return LinkTarget::Asset { path, suffix };
    }
    LinkTarget::Other
}

/// 相対パスを `dir`（`/` 区切り、空 = ルート）基準で解決し、`/` 区切りに正規化する。
///
/// `..` がルート（content）より上へ出るなら `None`。以前は出た分の `..` を捨てていたので、
/// `../README.md` が content の `README.md` のページに黙って解決されていた（v0.20 Phase 91）。
/// リンク検査（`linkcheck.rs`）と描画（yuzu-render `urls.rs`）が共有する
pub fn resolve_relative(dir: &str, target: &str) -> Option<String> {
    let mut parts: Vec<&str> = if dir.is_empty() {
        Vec::new()
    } else {
        dir.split('/').collect()
    };
    for seg in target.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            s => parts.push(s),
        }
    }
    Some(parts.join("/"))
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
/// route が空なら `None` ＝ページを作らない（設定で止めた状態）。パスが不正
/// （[`synth_page_problem`]）でも `None` を返してルート外へは絶対に書かないが、
/// cli は設定の読み込み直後に [`synth_page_problem`] で位置付きの設定エラーにするので、
/// 不正な値のままここへ来るのはライブラリを直接使ったときだけ
pub(crate) fn synth_page_rel(raw: &str) -> Option<std::path::PathBuf> {
    let route = raw.trim().trim_matches('/');
    if route.is_empty() || synth_page_problem(raw).is_some() {
        return None;
    }
    Some(std::path::PathBuf::from(format!("{route}.md")))
}

/// 合成ページの route の設定値（`search.page` / `markdown.glossary.page`）が使えない理由。
/// 空（ページを作らない）は問題なし。
///
/// 絶対パス・ドライブレター・`..`・`.`・空の階層・`\` を拒否する。`..` を弾くのは
/// `content_dir.join(rel)` が外へ出ないため。以前は不正な値で黙ってページを作らず、
/// build も check も成功していた（10-12 レビューの R3）。
/// `\` はどこにあっても拒否する。Windows では `\` がパスの区切りになるので、`"a\\b"` が
/// Windows では `a/b` のページになり、ほかの OS では使えない文字のエラーになっていた（Phase 87）
pub fn synth_page_problem(raw: &str) -> Option<&'static str> {
    let route = raw.trim().trim_matches('/');
    if route.is_empty() {
        // `"/"` は `/` を外すと空になり、以前は「作らない」と同じ扱いで黙ってページが消えていた
        // （v0.20 Phase 91。エイリアスの `"/"` はすでに設定エラー）
        return (!raw.trim().is_empty()).then_some(
            "サイトのルート（`/`）には置けません（トップページと重なります）。ページを作らないときは空文字（`\"\"`）にしてください",
        );
    }
    let bad_segment = route
        .split('/')
        .any(|s| s.is_empty() || s == "." || s == "..");
    (route.contains('\\') || route.contains(':') || bad_segment).then_some(
        "content 相対のパス（例: `search`・`help/search`）で書いてください。`..`・`.`・空の階層（`//`）・`\\`・`:` は使えません",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    /// リンク検査と描画が共有する分類（R5）。分類はデコード前に行い、相対参照のパスだけデコードする
    #[test]
    fn リンクの_url_を分類する() {
        assert_eq!(classify_link(""), LinkTarget::Empty);
        assert_eq!(classify_link("#a"), LinkTarget::Fragment("a"));
        assert_eq!(classify_link("https://example.com/"), LinkTarget::External);
        assert_eq!(classify_link("mailto:a@example.com"), LinkTarget::External);
        assert_eq!(
            classify_link("/foo/?x=1#y"),
            LinkTarget::Absolute {
                rest: "foo/",
                suffix: "?x=1#y"
            }
        );
        // クエリに URL を含んでも外部ではない（v0.20 Phase 90 の PR #66 の指摘）
        assert_eq!(
            classify_link("/search/?q=https://x"),
            LinkTarget::Absolute {
                rest: "search/",
                suffix: "?q=https://x"
            }
        );
        // ルート絶対はデコードしない（描画は書いたまま使う）
        assert_eq!(
            classify_link("/%E8%A8%AD/"),
            LinkTarget::Absolute {
                rest: "%E8%A8%AD/",
                suffix: ""
            }
        );
        assert_eq!(
            classify_link("my%20page.md#x"),
            LinkTarget::Page {
                path: "my page.md".to_string(),
                suffix: "#x"
            }
        );
        assert_eq!(
            classify_link("img/a.png"),
            LinkTarget::Asset {
                path: "img/a.png".to_string(),
                suffix: ""
            }
        );
        // `%2F` はデコードすると `/` だが、分類はデコード前なので相対参照
        assert_eq!(
            classify_link("%2Flogo.png"),
            LinkTarget::Asset {
                path: "/logo.png".to_string(),
                suffix: ""
            }
        );
        assert_eq!(classify_link("guide/"), LinkTarget::Other);
        assert_eq!(classify_link("guide"), LinkTarget::Other);
        assert_eq!(LinkTarget::fragment("?a=1#b"), Some("b"));
        assert_eq!(LinkTarget::fragment("?a=1"), None);
    }

    #[test]
    fn suffix_の分離() {
        assert_eq!(split_suffix("a.md#frag"), ("a.md", "#frag"));
        assert_eq!(split_suffix("a.md?q=1#f"), ("a.md", "?q=1#f"));
        assert_eq!(split_suffix("a.md"), ("a.md", ""));
    }

    #[test]
    fn 相対解決() {
        let some = |s: &str| Some(s.to_string());
        assert_eq!(
            resolve_relative("guide", "./index.md"),
            some("guide/index.md")
        );
        assert_eq!(resolve_relative("guide", "../index.md"), some("index.md"));
        assert_eq!(resolve_relative("", "a/b.md"), some("a/b.md"));
        assert_eq!(resolve_relative("a/b", "../../c.md"), some("c.md"));
        // 一度下りてから上がるのはルートの中
        assert_eq!(resolve_relative("", "x/../c.md"), some("c.md"));
        // ルート（content）より上へ出る `..` は None（以前は捨てて中のページに解決していた）
        assert_eq!(resolve_relative("", "../README.md"), None);
        assert_eq!(resolve_relative("guide", "../../README.md"), None);
        assert_eq!(resolve_relative("", "../content/a.md"), None);
    }

    #[test]
    fn 合成ページの_route_は空なら作らず不正なら理由を返す() {
        for ok in ["search", "/search/", " help/search ", "用語集"] {
            assert_eq!(synth_page_problem(ok), None, "{ok}");
            assert!(synth_page_rel(ok).is_some(), "{ok}");
        }
        assert_eq!(
            synth_page_rel("/help/search/"),
            Some(std::path::PathBuf::from("help/search.md"))
        );
        // 空はページを作らない設定（問題なし）
        assert_eq!(synth_page_problem(""), None);
        assert_eq!(synth_page_rel("  "), None);
        for bad in [
            "../search",
            "a/../b",
            "./search",
            "a//b",
            "C:/x",
            "\\x",
            // 途中の `\` も拒否する（Windows では区切りになり、OS で結果が変わっていた）
            "a\\b",
            "a:b",
            // サイトのルートは「作らない」ではなく設定エラー（v0.20 Phase 91）
            "/",
            "//",
            " / ",
        ] {
            assert!(synth_page_problem(bad).is_some(), "{bad}");
            assert_eq!(synth_page_rel(bad), None, "{bad}");
        }
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
    fn 外部参照の判定() {
        for url in [
            "https://example.com/a.md",
            "http://x",
            "ftp://x/y",
            "mailto:a@example.com",
            "tel:+81",
        ] {
            assert!(is_external_url(url), "{url}");
        }
        for url in ["guide/a.md", "/images/x.png", "#frag", "a:b.md"] {
            assert!(!is_external_url(url), "{url}");
        }
        // スキームは先頭だけを見る（クエリ・フラグメントの URL で外部扱いにしない）
        assert!(is_external_url("HTTPS://example.com/"));
        assert!(is_external_url("git+ssh://example.com/repo"));
        for url in [
            "/search/?q=https://example.com/",
            "guide/?next=http://x",
            "a.md#https://x",
            "://x",
            "1http://x",
        ] {
            assert!(!is_external_url(url), "{url}");
            assert!(!has_url_scheme(url), "{url}");
        }
        assert!(is_http_url("https://example.com/"));
        assert!(is_http_url("HTTP://example.com/"));
        assert!(!is_http_url("ftp://example.com/"));
        assert!(!is_http_url("mailto:a@example.com"));
        assert!(!is_http_url("guide/a.md"));
    }

    #[test]
    fn encode_と_decode_は往復する() {
        for raw in ["設計/概 要#1/", "a%23b/", "f(x)&y/", "guide/x/"] {
            assert_eq!(percent_decode(&encode_path(raw)), raw, "{raw}");
        }
    }
}
