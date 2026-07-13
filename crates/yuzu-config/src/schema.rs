//! `yuzu.jsonc` の設定スキーマ。
//!
//! すべてのキーは省略可能で、省略時は各 `Default` 実装の値になる。
//! JSON 側のキーは camelCase（`baseUrl` など）。

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Config {
    pub site: SiteConfig,
    pub input: InputConfig,
    pub output: OutputConfig,
    pub theme: ThemeConfig,
    pub nav: NavConfig,
    pub markdown: MarkdownConfig,
    pub lint: LintConfig,
    pub search: SearchConfig,
    pub llms: LlmsConfig,
    pub build: BuildConfig,
    pub dev: DevConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SiteConfig {
    pub title: String,
    pub description: Option<String>,
    /// サイトを配信するパス接頭辞（例: `/docs/`）。`build.baseUrl` があればそちらが優先
    pub base_url: Option<String>,
    pub lang: String,
    /// ヘッダーのタイトル横に出すロゴ画像（例: `/images/logo.svg`。public/ 配下を指す）。
    /// フル URL も可。未指定ならテーマ既定の絵文字ロゴ
    pub logo: Option<String>,
}

impl Default for SiteConfig {
    fn default() -> Self {
        Self {
            title: "Documentation".to_string(),
            description: None,
            base_url: None,
            lang: "ja".to_string(),
            logo: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct InputConfig {
    pub dir: String,
    /// 除外 glob（`content/` からの相対パスに対して評価。例: `**/_drafts/**`）
    pub ignore: Vec<String>,
}

impl Default for InputConfig {
    fn default() -> Self {
        Self {
            dir: "content".to_string(),
            ignore: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct OutputConfig {
    pub dir: String,
    /// ビルド前に出力ディレクトリを削除するか
    pub clean: bool,
}

impl Default for OutputConfig {
    fn default() -> Self {
        Self {
            dir: "dist".to_string(),
            clean: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ThemeConfig {
    pub name: String,
    /// ダークモード切替 UI を有効にするか
    pub dark: bool,
    /// テーマ CSS 変数の上書き（キーは `--` 省略可。例: `"accent": "#0a6cff"`）。
    /// 変数名は theme.css の `:root` 定義を参照。BTreeMap なので出力は決定的
    pub css_vars: BTreeMap<String, String>,
    /// ダークモード時にのみ適用する上書き（`html[data-theme="dark"]` スコープ）
    pub css_vars_dark: BTreeMap<String, String>,
}

impl Default for ThemeConfig {
    fn default() -> Self {
        Self {
            name: "default".to_string(),
            dark: true,
            css_vars: BTreeMap::new(),
            css_vars_dark: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct NavConfig {
    /// ディレクトリ階層＋frontmatter `title`/`order` からナビを自動生成する。
    /// v0.1 では自動生成のみ（手動ナビ配列は非対応）
    pub auto: bool,
}

impl Default for NavConfig {
    fn default() -> Self {
        Self { auto: true }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct MarkdownConfig {
    /// GFM 拡張（表・打ち消し線・autolink・タスクリスト）
    pub gfm: bool,
    pub highlight: HighlightConfig,
    pub mermaid: MermaidConfig,
    pub math: MathConfig,
}

impl Default for MarkdownConfig {
    fn default() -> Self {
        Self {
            gfm: true,
            highlight: HighlightConfig::default(),
            mermaid: MermaidConfig::default(),
            math: MathConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct HighlightConfig {
    pub enabled: bool,
    /// syntect のライト側テーマ名
    pub theme_light: String,
    /// syntect のダーク側テーマ名
    pub theme_dark: String,
}

impl Default for HighlightConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            theme_light: "InspiredGitHub".to_string(),
            theme_dark: "base16-ocean.dark".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct MermaidConfig {
    /// mermaid コードブロックの描画を有効にするか
    pub enabled: bool,
    /// 描画方式。client = mermaid.js（従来）/ ssr = tankan によるビルド時 SVG
    /// （未対応図種はクライアント描画へ自動フォールバック）
    pub backend: MermaidBackend,
}

impl Default for MermaidConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            backend: MermaidBackend::Client,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MermaidBackend {
    /// mermaid.js によるクライアント描画（既定）
    #[default]
    Client,
    /// tankan によるビルド時 SVG（対応図種のみ。他はクライアントへフォールバック）
    Ssr,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct MathConfig {
    /// 数式（`$...$` / `$$...$$` / `` $`...`$ `` / ```math）を有効にするか。
    /// 描画は同梱 KaTeX のクライアント描画で、数式のあるページだけ読み込む
    // 将来: backend（"client" | "ssr"）。serde は未知キーを無視するので後方互換で追加できる
    pub enabled: bool,
}

impl Default for MathConfig {
    fn default() -> Self {
        Self { enabled: true }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct LintConfig {
    /// content 配下で許容するディレクトリ階層の最大深さ
    /// （直下 = 0。例: 1 なら `content/guide/x.md` まで）。未指定なら無制限
    pub max_directory_depth: Option<u32>,
    /// 用語統一の辞書（正しい表記 → ゆれ表記のリスト）。
    /// 例: `"terms": { "サーバー": ["サーバ"], "ユーザー": ["ユーザ"] }`
    pub terms: BTreeMap<String, Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SearchConfig {
    /// 全文検索（インデックス生成＋テーマの検索 UI）を有効にするか
    pub enabled: bool,
    /// vaporetto モデル（`.model.zst`）のパス。未指定なら同梱モデル
    pub dictionary: Option<String>,
    pub typo_tolerance: TypoToleranceConfig,
    pub shard: ShardConfig,
}

impl Default for SearchConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            dictionary: None,
            typo_tolerance: TypoToleranceConfig::default(),
            shard: ShardConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct TypoToleranceConfig {
    pub enabled: bool,
    /// 許容編集距離。v1 では 0..=1 に clamp される（2 以上はノイズと構築コストが跳ねる）
    pub max_edits: u8,
}

impl Default for TypoToleranceConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_edits: 1,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ShardConfig {
    /// 1 シャードあたりの term 数（term_id の連続範囲で分割）
    pub max_terms_per_shard: u32,
}

impl Default for ShardConfig {
    fn default() -> Self {
        Self {
            max_terms_per_shard: 16384,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct LlmsConfig {
    /// llms.txt / llms-full.txt を生成するか
    pub enabled: bool,
    /// llms-full.txt（正規化 Markdown の全文連結）も生成するか
    pub full: bool,
}

impl Default for LlmsConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            full: true,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct BuildConfig {
    /// ビルド時の baseUrl 上書き（`site.baseUrl` より優先）
    pub base_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct DevConfig {
    pub host: String,
    pub port: u16,
    /// `yuzu dev` の WebSocket ライブリロード。
    /// false なら監視ビルド＋配信のみ（WS 注入なし。反映は手動リロード）
    pub live_reload: bool,
    /// `yuzu dev` 起動時に既定ブラウザでサイトを開く
    pub open: bool,
}

impl Default for DevConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 5173,
            live_reload: true,
            open: false,
        }
    }
}
