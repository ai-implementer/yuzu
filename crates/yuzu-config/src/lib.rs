//! yuzu の設定（`yuzu.toml`）の読み込み・探索・解決。
//!
//! - 設定ファイルの正本はプロジェクトルートの `yuzu.toml`（TOML。パーサは kabosu）
//! - cwd から上方向に `yuzu.toml` を探索し、見つかったディレクトリを
//!   プロジェクトルートとする
//! - すべてのキーは省略可能で、省略時は各 `Default` の値になる。キーは snake_case
//! - 未知キー・型不一致・不正値は**設定エラー**（位置付き・全件蓄積）として
//!   読み込みを止める。TOML の重複キーは構文エラー
//! - この crate は I/O を伴う探索と読み込みだけを持ち、ログは出さない
//!   （`ResolvedConfig::diagnostics` の警告は呼び出し側が表示する）

mod codec;
mod discover;
mod error;
mod resolve;
mod schema;

pub use discover::find_project_root;
pub use error::{ConfigError, ConfigIssue};
pub use resolve::{CONFIG_RULES, ConfigDiagnostic, ResolvedConfig, load, normalize_base_url};
pub use schema::{
    BuildConfig, Config, CrossrefConfig, CrossrefNumbering, DISABLEABLE_RULES, DevConfig,
    GitConfig, GlossaryConfig, HighlightConfig, InputConfig, LintConfig, LlmsConfig,
    MarkdownConfig, MathConfig, MermaidBackend, MermaidConfig, NavConfig, OutputConfig,
    SearchConfig, ShardConfig, SiteConfig, ThemeConfig, TocConfig, TypoToleranceConfig,
};

/// 設定ファイル名（プロジェクトルートのマーカーを兼ねる）
pub const CONFIG_FILE_NAME: &str = "yuzu.toml";
