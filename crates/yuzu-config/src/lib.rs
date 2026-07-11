//! yuzu の設定（`yuzu.jsonc`）の読み込み・探索・解決。
//!
//! - 設定ファイルの正本はプロジェクトルートの `yuzu.jsonc`（JSONC: コメント可）
//! - cwd から上方向に `yuzu.jsonc` を探索し、見つかったディレクトリを
//!   プロジェクトルートとする
//! - デフォルトをマージした解決済み設定は `.yuzu/settings.json` に書き出す

mod discover;
mod error;
mod resolve;
mod schema;

pub use discover::find_project_root;
pub use error::ConfigError;
pub use resolve::{ResolvedConfig, load, write_resolved};
pub use schema::{
    BuildConfig, Config, DevConfig, HighlightConfig, InputConfig, LintConfig, LlmsConfig,
    MarkdownConfig, MathConfig, MermaidBackend, MermaidConfig, NavConfig, OutputConfig,
    SearchConfig, ShardConfig, SiteConfig, ThemeConfig, TypoToleranceConfig,
};

/// 設定ファイル名（プロジェクトルートのマーカーを兼ねる）
pub const CONFIG_FILE_NAME: &str = "yuzu.jsonc";
