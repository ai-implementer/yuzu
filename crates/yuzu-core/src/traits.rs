//! render 側が実装して差し込むフック trait。
//! core はこの trait 経由でのみ外部と連携し、パーサ実装を漏らさない

use crate::model::Page;

/// コードブロックの HTML 化フック。
/// `Some(html)` を返すと `<pre>` ごとその HTML で差し替える
/// （syntect ハイライトや `<pre class="mermaid">` 化）。
/// `None` なら Markdown パーサの既定出力（エスケープ済み `<pre><code>`）になる
pub trait CodeBlockRenderer {
    /// `lang` はフェンス情報文字列の先頭トークン（```rust → `Some("rust")`）
    fn render(&self, lang: Option<&str>, code: &str) -> Option<String>;
}

/// 本文中のリンク・画像 URL の書き換えフック。
/// `None` なら無変更。base path 解決や `.md` 相互リンク解決に使う
pub trait UrlRewriter {
    fn rewrite(&self, page: &Page, url: &str) -> Option<String>;
}

/// 何もしない [`CodeBlockRenderer`]（テスト・素の HTML 出力用）
pub struct NoopCodeBlockRenderer;

impl CodeBlockRenderer for NoopCodeBlockRenderer {
    fn render(&self, _lang: Option<&str>, _code: &str) -> Option<String> {
        None
    }
}

/// 何もしない [`UrlRewriter`]（テスト・素の HTML 出力用）
pub struct NoopUrlRewriter;

impl UrlRewriter for NoopUrlRewriter {
    fn rewrite(&self, _page: &Page, _url: &str) -> Option<String> {
        None
    }
}
