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
mod templates;
mod urls;

pub use error::RenderError;
pub use highlight::SyntectCodeRenderer;
pub use llms::{generate_llms_full_txt, generate_llms_txt};
pub use pipeline::{LiveReloadMode, RenderCtx, RenderParams, render_site};
pub use shared::RenderShared;
pub use urls::UrlResolver;
