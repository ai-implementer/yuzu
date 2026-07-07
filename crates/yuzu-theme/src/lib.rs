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
    #[test]
    fn 必須アセットが同梱されている() {
        for path in [
            "templates/base.jinja",
            "templates/page.jinja",
            "templates/partials/sidebar.jinja",
            "templates/partials/toc.jinja",
            "templates/partials/toc-mobile.jinja",
            "templates/partials/header.jinja",
            "static/css/theme.css",
            "static/js/theme.js",
            "static/js/nav.js",
            "static/js/scrollspy.js",
            "static/js/autorefresh.js",
            "static/js/livereload.js",
            "static/js/mermaid-init.js",
            "static/js/search-ui.js",
            "static/vendor/mermaid.min.js",
        ] {
            assert!(super::get(path).is_some(), "{path} が同梱されていない");
        }
    }
}
