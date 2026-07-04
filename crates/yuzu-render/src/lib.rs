//! yuzu のレンダリング: サイトモデル → 静的 HTML サイト（`dist/`）。
//!
//! - テンプレートは minijinja（プロジェクト `theme/` → 埋め込みデフォルトテーマの
//!   順で解決）
//! - コードブロックは syntect で **CSS クラス出力**（配色はビルド時生成の
//!   `syntect.css` が担い、ライト/ダーク両対応）
//! - ` ```mermaid ` は `<pre class="mermaid">` へ変換（クライアント描画）
//! - リンク・アセット参照は `baseUrl` 付きの絶対パスへ解決
//!
//! 将来: Markdown 正規化出力（`yuzu fmt`）、`llms.txt` / `llms-full.txt`
//! （正規化 md の連結）もこの crate が担う（Phase 4/6）。

mod assets;
mod context;
mod css;
mod error;
mod highlight;
mod pipeline;
mod templates;
mod urls;

pub use error::RenderError;
pub use highlight::SyntectCodeRenderer;
pub use pipeline::{LiveReloadMode, RenderParams, render_site};
pub use urls::UrlResolver;
