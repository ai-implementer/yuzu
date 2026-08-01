//! yuzu のレンダリング: サイトモデル → 静的 HTML サイト（`dist/`）。
//!
//! - テンプレートは minijinja（プロジェクト `theme/` → 埋め込みデフォルトテーマの
//!   順で解決）
//! - コードブロックは syntect で **CSS クラス出力**（配色はビルド時生成の
//!   `syntect.css` が担い、ライト/ダーク両対応）
//! - ` ```mermaid ` は `<pre class="mermaid">` へ変換（クライアント描画）
//! - リンク・アセット参照は `baseUrl` 付きの絶対パスへ解決
//!
//! `llms.txt` / `llms-full.txt`（正規化 md の連結）もこの crate が担う（Phase 4）。
//! `yuzu fmt` の整形コアは yuzu-core の `format_document`（Phase 6）。

mod apispec;
mod assets;
mod context;
mod css;
mod error;
mod highlight;
mod llms;
mod pipeline;
mod shared;
mod speccheck;
mod templates;
mod urls;

pub use error::RenderError;
pub use highlight::SyntectCodeRenderer;

/// `markdown.glossary` を yuzu-core 側の中立型へ写す。
///
/// **写像をここ 1 箇所に置く**理由: `MarkdownOptions` の構築点は cli と render に
/// 8 箇所あり、辞書だけ配線を落とすと「設定したのに用語集が出ない」になる。
/// yuzu-render は yuzu-config と yuzu-core の両方に依存する唯一の共通の下層
pub fn glossary_options(cfg: &yuzu_config::Config) -> yuzu_core::GlossaryOptions {
    let g = &cfg.markdown.glossary;
    yuzu_core::GlossaryOptions {
        terms: g.terms.clone(),
        abbr: g.abbr,
        page: g.page.clone(),
        page_title: g.page_title.clone(),
    }
}
pub use llms::{generate_llms_full_txt, generate_llms_txt};
pub use pipeline::{LiveReloadMode, RenderCtx, RenderParams, render_site, validate_pages};
pub use shared::RenderShared;
pub use speccheck::validate_api_specs;
pub use urls::UrlResolver;
