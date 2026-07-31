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

/// URL 値を **HTML 属性と `<script>` のどちらの文脈でも安全**な形へ変換する
/// minijinja フィルタ（`{{ url_value | url }}`）。
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
/// 通常の URL では出力が 1 バイトも変わらないので、`| safe` の置き換えとして
/// 既存の出力を保ったまま安全化できる。
fn url_filter(value: &str) -> minijinja::value::Value {
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
    // 変換後は安全なので autoescape の二重処理を避ける
    minijinja::value::Value::from_safe_string(out)
}

#[cfg(test)]
mod tests {
    use super::build_env;

    #[test]
    fn 埋め込みテンプレートをロードできる() {
        let env = build_env(None).unwrap();
        assert!(env.get_template("page.jinja").is_ok());
        assert!(env.get_template("base.jinja").is_ok());
        assert!(env.get_template("no-such.jinja").is_err());
    }

    fn render_url(value: &str) -> String {
        let mut env = build_env(None).unwrap();
        env.add_template("t", "{{ v | url }}").unwrap();
        env.get_template("t")
            .unwrap()
            .render(minijinja::context! { v => value })
            .unwrap()
    }

    /// `| safe` からの置き換えで既存の出力が変わらないことの担保
    #[test]
    fn url_フィルタは通常の_url_を変えない() {
        for url in [
            "/",
            "/docs/",
            "/docs/guide/getting-started/",
            "https://example.com/docs/?q=1&r=2#anchor",
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
        // URL 構文文字はそのまま
        assert_eq!(render_url("/a?q=1&r=2#f"), "/a?q=1&r=2#f");
    }
}
