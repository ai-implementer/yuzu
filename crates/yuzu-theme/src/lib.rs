//! yuzu のデフォルトテーマ。
//!
//! `assets/` 以下（minijinja テンプレート・CSS・JS・vendor 物）を rust-embed で
//! バイナリに同梱する。プロジェクト側の `theme/` に同名ファイルがあれば
//! そちらが優先される（上書きは yuzu-render のローダが行う）。
//!
//! 注意: debug ビルドでは rust-embed はファイルシステムから読む
//! （テーマ編集が再コンパイルなしで反映される）。リリースビルドは常に埋め込み。

use std::borrow::Cow;

use rust_embed::RustEmbed;

/// デフォルトテーマのアセット一式。
/// パス例: `templates/base.jinja` / `static/css/theme.css` / `static/vendor/mermaid.min.js`
#[derive(RustEmbed)]
#[folder = "assets"]
pub struct DefaultTheme;

/// アセットを読む。存在しなければ None
pub fn get(path: &str) -> Option<Cow<'static, [u8]>> {
    DefaultTheme::get(path).map(|f| f.data)
}

/// 同梱アセットのパスを列挙する
pub fn iter() -> impl Iterator<Item = Cow<'static, str>> {
    DefaultTheme::iter()
}

#[cfg(test)]
mod tests {
    /// `selector {` の次の行から、対応する `  }` の手前までの宣言を返す
    fn block_body<'a>(css: &'a str, selector: &str) -> &'a str {
        let open = format!("  {selector} {{\n");
        let start = css
            .find(&open)
            .unwrap_or_else(|| panic!("{selector} が無い"))
            + open.len();
        let len = css[start..]
            .find("\n  }\n")
            .expect("ブロックが閉じていない");
        &css[start..start + len]
    }

    /// theme.css のダーク定義は 2 系統（明示の選択 / OS 追従）を手で並べている。
    /// 片方だけ直すと「ボタンで選んだダーク」と「OS 追従のダーク」で配色が食い違うので、
    /// 中身が同じであることを縛る（Phase 79）
    #[test]
    fn ダーク定義の_2_系統は同じ中身() {
        let css =
            String::from_utf8(super::get("static/css/theme.css").unwrap().into_owned()).unwrap();
        let explicit = block_body(&css, "html[data-theme=\"dark\"]");
        let os = block_body(&css, "html:not([data-theme])");
        assert!(explicit.contains("--bg:"), "{explicit}");
        assert_eq!(
            explicit, os,
            "theme.css のダーク定義 2 ブロックが食い違っている"
        );
        // OS 追従は画面専用かつ prefers-color-scheme の内側
        assert!(css.contains(
            "@media screen and (prefers-color-scheme: dark) {\n  html:not([data-theme]) {"
        ));
    }

    #[test]
    fn 必須アセットが同梱されている() {
        for path in [
            "templates/base.jinja",
            "templates/page.jinja",
            "templates/404.jinja",
            "templates/redirect.jinja",
            "templates/search.jinja",
            "templates/partials/sidebar.jinja",
            "templates/partials/toc.jinja",
            "templates/partials/toc-mobile.jinja",
            "templates/partials/header.jinja",
            "templates/partials/breadcrumb.jinja",
            "templates/partials/pager.jinja",
            "static/css/theme.css",
            "static/js/theme.js",
            "static/js/nav.js",
            "static/js/scrollspy.js",
            "static/js/copy-button.js",
            "static/js/page-copy.js",
            "static/js/details-target.js",
            "static/js/autorefresh.js",
            "static/js/livereload.js",
            "static/js/mermaid-init.js",
            "static/js/katex-init.js",
            "static/js/search-ui.js",
            "static/js/search-page.js",
            "static/js/search-hits.js",
            "static/vendor/mermaid.min.js",
            "static/vendor/katex/katex.min.js",
            "static/vendor/katex/katex.min.css",
            "static/vendor/katex/fonts/KaTeX_Main-Regular.woff2",
        ] {
            assert!(super::get(path).is_some(), "{path} が同梱されていない");
        }
    }
}
