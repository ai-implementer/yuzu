//! minijinja 環境の構築。
//!
//! テンプレート解決の優先順:
//! 1. プロジェクトの `theme/templates/<name>`（部分上書き可）
//! 2. 埋め込みデフォルトテーマ（`yuzu-theme`）

use std::fs;
use std::path::{Path, PathBuf};

use minijinja::{AutoEscape, Environment};

use crate::error::RenderError;

pub(crate) fn build_env(theme_dir: Option<&Path>) -> Result<Environment<'static>, RenderError> {
    let mut env = Environment::new();
    // テンプレート名の拡張子（.jinja）に関わらず常に HTML エスケープする。
    // 本文 HTML はテンプレート側で `| safe` を通す
    env.set_auto_escape_callback(|_| AutoEscape::Html);
    // URL 値専用のエスケープ（`| safe` の置き換え。下の doc コメント参照）
    env.add_filter("url", url_filter);
    env.add_filter("url_js", url_js_filter);

    let override_dir: Option<PathBuf> = theme_dir.map(|d| d.join("templates"));
    env.set_loader(move |name| {
        if let Some(dir) = &override_dir {
            let path = dir.join(name);
            if path.is_file() {
                let text = fs::read_to_string(&path).map_err(|e| {
                    minijinja::Error::new(
                        minijinja::ErrorKind::InvalidOperation,
                        format!("テーマテンプレート {} を読めません: {e}", path.display()),
                    )
                })?;
                return Ok(Some(text));
            }
        }
        match yuzu_theme::get(&format!("templates/{name}")) {
            Some(data) => Ok(Some(String::from_utf8_lossy(&data).into_owned())),
            None => Ok(None),
        }
    });

    Ok(env)
}

/// URL 値を **HTML 属性で安全**な形へ変換する minijinja フィルタ
/// （`{{ url_value | url }}`。`<script>` 内の文字列は [`url_js_filter`]）。
///
/// `/` `:` `?` `&` `#` 等の URL 構文文字はそのまま残し、文脈を壊す文字だけを
/// percent エンコードする。yuzu には slug 化が無く、ファイル名がそのまま route →
/// URL になるため、空白や引用符を含むファイル名が生の URL としてテンプレートへ届く。
///
/// - `| safe` は**エスケープを止めるだけ**なので、これらを素通しさせてしまう
/// - 逆に `| safe` を外すと minijinja の HTML エスケープが `/` を `&#x2f;` にして
///   全 URL が読めなくなる（かつ `<script>` 内は実体参照がデコードされないため、
///   `</script>` や `"` を含む値には HTML エスケープでは対処できない）
///
/// **HTML 属性専用**（`href` / `content` / `data-*`）。属性値は HTML パーサが実体参照を
/// デコードするため `&` を `&amp;` にする — 生の `&` を残すと `?label=a&copy;b` が
/// `a©b` に化けて別の URL になる（レビュー指摘）。`<script>` 内の文字列は実体参照が
/// デコードされない文脈なので、そちらは [`url_js_filter`] を使う
fn url_filter(value: &str) -> minijinja::value::Value {
    let encoded = percent_encode_unsafe(value);
    minijinja::value::Value::from_safe_string(encoded.replace('&', "&amp;"))
}

/// **`<script>` 内の JS 文字列専用**。`&` はそのまま（実体参照がデコードされない文脈で
/// `&amp;` にすると別の URL になる）。`"` / `\` / `<` はパーセントエンコードするので
/// 文字列リテラルも script 要素も抜けられない
fn url_js_filter(value: &str) -> minijinja::value::Value {
    minijinja::value::Value::from_safe_string(percent_encode_unsafe(value))
}

/// 属性・JS 文字列・タグを抜けられる文字と、URL に入れてはいけない空白類を
/// パーセントエンコードする（両フィルタの共通部分。通常の URL は 1 バイトも変わらない）
fn percent_encode_unsafe(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for c in value.chars() {
        match c {
            // 属性・JS 文字列・タグを抜けられる文字と、URL に入れてはいけない空白類
            '"' | '\'' | '<' | '>' | '`' | '\\' | ' ' => {
                let mut buf = [0u8; 4];
                for b in c.encode_utf8(&mut buf).as_bytes() {
                    out.push_str(&format!("%{b:02X}"));
                }
            }
            c if c.is_control() || c.is_whitespace() => {
                let mut buf = [0u8; 4];
                for b in c.encode_utf8(&mut buf).as_bytes() {
                    out.push_str(&format!("%{b:02X}"));
                }
            }
            c => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::build_env;

    #[test]
    fn 埋め込みテンプレートをロードできる() {
        let env = build_env(None).unwrap();
        assert!(env.get_template("page.jinja").is_ok());
        assert!(env.get_template("base.jinja").is_ok());
        assert!(env.get_template("search.jinja").is_ok());
        assert!(env.get_template("no-such.jinja").is_err());
    }

    fn render_with(filter: &str, value: &str) -> String {
        let mut env = build_env(None).unwrap();
        env.add_template_owned("t", format!("{{{{ v | {filter} }}}}"))
            .unwrap();
        env.get_template("t")
            .unwrap()
            .render(minijinja::context! { v => value })
            .unwrap()
    }

    fn render_url(value: &str) -> String {
        render_with("url", value)
    }

    /// `| safe` からの置き換えで既存の出力が変わらないことの担保（`&` を含まない URL）
    #[test]
    fn url_フィルタは通常の_url_を変えない() {
        for url in [
            "/",
            "/docs/",
            "/docs/guide/getting-started/",
            "https://example.com/docs/?q=1#anchor",
            "/日本語/ページ/",
            "_assets/css/theme.css",
        ] {
            assert_eq!(render_url(url), url, "出力が変わらないこと: {url}");
        }
    }

    #[test]
    fn url_フィルタは文脈を壊す文字だけをエンコードする() {
        // 属性・script を抜けられる文字
        assert_eq!(render_url(r#"/a"b/"#), "/a%22b/");
        assert_eq!(render_url("/a'b/"), "/a%27b/");
        // `/` は URL 構文文字なので残すが、`<` を潰せば script 要素は閉じられない
        assert_eq!(render_url("/a</script>b/"), "/a%3C/script%3Eb/");
        assert_eq!(render_url("/a b/"), "/a%20b/");
        assert_eq!(render_url("/a\tb/"), "/a%09b/");
        // `?` `#` `=` はそのまま
        assert_eq!(render_url("/a?q=1#f"), "/a?q=1#f");
    }

    /// 属性値は HTML パーサが実体参照をデコードするので、`&` は `&amp;` にして
    /// 「`&copy;` が `©` に化けて別の URL になる」のを防ぐ（レビュー指摘で発覚）
    #[test]
    fn url_フィルタは属性用に_and_をエスケープする() {
        assert_eq!(render_url("/a?q=1&r=2"), "/a?q=1&amp;r=2");
        assert_eq!(
            render_url("https://cdn.example.com/og.png?label=a&copy;b"),
            "https://cdn.example.com/og.png?label=a&amp;copy;b"
        );
    }

    /// `<script>` 内の文字列は実体参照がデコードされないので `&` はそのまま。
    /// 抜けられる文字のエンコードは `url` と同じ
    #[test]
    fn url_js_フィルタは_and_を残し文脈を壊す文字だけをエンコードする() {
        assert_eq!(render_with("url_js", "/a?q=1&r=2#f"), "/a?q=1&r=2#f");
        assert_eq!(render_with("url_js", r#"/a"b/"#), "/a%22b/");
        assert_eq!(render_with("url_js", "/a</script>b/"), "/a%3C/script%3Eb/");
        assert_eq!(render_with("url_js", "/a\\b/"), "/a%5Cb/");
    }
}
