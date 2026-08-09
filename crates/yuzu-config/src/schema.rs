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
    pub git: GitConfig,
}

/// git 連携メタ（ページフッターの最終更新日・編集リンク）
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct GitConfig {
    /// ページの最終更新日（最終コミット日）をフッターに表示する。
    /// git が無い・リポジトリ外・未コミットのファイルでは表示しない（縮退）
    pub last_updated: bool,
    /// 「このページを編集」リンクの URL テンプレート。`{path}` が content 相対パスに
    /// 置換される（例: `https://github.com/me/docs/edit/main/content/{path}`）
    pub edit_url: Option<String>,
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
    /// ページ内 TOC の表示設定
    pub toc: TocConfig,
}

impl Default for ThemeConfig {
    fn default() -> Self {
        Self {
            name: "default".to_string(),
            dark: true,
            css_vars: BTreeMap::new(),
            css_vars_dark: BTreeMap::new(),
            toc: TocConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct TocConfig {
    /// ページ内 TOC に表示する見出しレベルの範囲（h1〜h6 = 1〜6）。
    /// インクルードの `lines=` と同じ記法で `"2-3"` / `"4"` のように書く。
    /// 不正な値は警告して既定へ縮退する
    pub levels: String,
}

impl Default for TocConfig {
    fn default() -> Self {
        Self {
            levels: "2-3".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct NavConfig {
    /// ディレクトリ階層＋frontmatter `title`/`order` からナビを自動生成する。
    /// 現在は自動生成のみ対応で、`false` は将来の手動ナビ定義用の予約（効果なし）
    pub auto: bool,
    /// サイドバーで現在ページの祖先セクションだけを開き、他を折りたたむ
    /// （`<details>` によるクリック展開可能な折りたたみ）。false で従来の全展開
    pub collapse: bool,
}

impl Default for NavConfig {
    fn default() -> Self {
        Self {
            auto: true,
            collapse: true,
        }
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
    pub crossref: CrossrefConfig,
    pub glossary: GlossaryConfig,
}

/// 用語集・略語（`<abbr title>` 化と用語集ページの自動生成）の設定。
///
/// 辞書を設定に置くのは `lint.terms` と同じ思想で、本文の Markdown を
/// 汚さずに済む（素の Markdown ビューアでも読める、を保つ）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct GlossaryConfig {
    /// 用語辞書（略語 → 説明文）。例: `{ "API": "Application Programming Interface" }`。
    /// `BTreeMap` なので反復順が決定的（出力バイト同一・envKey の安定に効く）
    pub terms: BTreeMap<String, String>,
    /// 本文中の初出を `<abbr title="説明">略語</abbr>` にするか
    pub abbr: bool,
    /// 用語集ページの route（`content` 相対のパス。拡張子なし）。
    /// 空文字ならページを生成しない（`abbr` だけ使う運用）
    pub page: String,
    /// 用語集ページのタイトル（h1 とナビの表示名）
    pub page_title: String,
}

impl Default for GlossaryConfig {
    fn default() -> Self {
        Self {
            terms: BTreeMap::new(),
            abbr: true,
            page: "glossary".to_string(),
            page_title: "用語集".to_string(),
        }
    }
}

/// 図表番号（`Figure:` / `Table:` / `Listing:` キャプション行）の採番設定
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CrossrefConfig {
    /// 採番の単位。`"page"`（既定）はページごとに 1 から、
    /// `"site"` はサイドバーの表示順でサイト全体を通し番号にする
    pub numbering: CrossrefNumbering,
}

impl Default for CrossrefConfig {
    fn default() -> Self {
        Self {
            numbering: CrossrefNumbering::Page,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CrossrefNumbering {
    /// ページ内連番（既定）
    #[default]
    Page,
    /// サイト全体の通し番号（サイドバー表示順）
    Site,
}

impl Default for MarkdownConfig {
    fn default() -> Self {
        Self {
            gfm: true,
            highlight: HighlightConfig::default(),
            mermaid: MermaidConfig::default(),
            math: MathConfig::default(),
            crossref: CrossrefConfig::default(),
            glossary: GlossaryConfig::default(),
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
    /// コードブロックに行番号を表示するか（サイト既定。ブロック単位の
    /// `showLineNumbers` / `noLineNumbers` が優先される）
    pub line_numbers: bool,
}

impl Default for HighlightConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            theme_light: "InspiredGitHub".to_string(),
            theme_dark: "base16-ocean.dark".to_string(),
            line_numbers: false,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct LintConfig {
    /// content 配下で許容するディレクトリ階層の最大深さ
    /// （直下 = 0。例: 1 なら `content/guide/x.md` まで）。未指定なら無制限
    pub max_directory_depth: Option<u32>,
    /// 用語統一の辞書（正しい表記 → ゆれ表記のリスト）。
    /// 例: `"terms": { "サーバー": ["サーバ"], "ユーザー": ["ユーザ"] }`
    pub terms: BTreeMap<String, Vec<String>>,
    /// ルール ID → 有効フラグ。`false` でプロジェクト全体無効化
    /// （例: `"rules": { "katakana-choon": false }`。`true` は no-op として受理）。
    /// **ユーザの部分マップは既定を丸ごと置き換える**ため、参照側は
    /// 「マップに無い ID = 有効」と解釈する（解釈は yuzu-core の漏斗が持つ）
    pub rules: BTreeMap<String, bool>,
}

// rules の既定を「全 disableable ID → true」の非空マップにするため手書き
// （MathConfig と同じ前例。非空にする理由は DISABLEABLE_RULES の doc 参照）
impl Default for LintConfig {
    fn default() -> Self {
        Self {
            max_directory_depth: None,
            terms: BTreeMap::new(),
            rules: default_lint_rules(),
        }
    }
}

/// `lint.rules` で無効化できるルール ID（= レジストリの suppressible 集合と同一）。
/// yuzu-config は依存グラフの葉で `yuzu_core::rules` を参照できないため一覧をここに
/// 持ち、一致は yuzu-cli 側のテストが縛る（`CONFIG_RULES` と同じ規律）。
/// `Config::default()` の JSON 化（既知キー木）にこの ID が全部載ることで、
/// タイポ・旧キー・無効化不可の ID は行番号付き `config-unknown-key`
/// （正しい ID の兄弟一覧入り）になる
pub const DISABLEABLE_RULES: &[&str] = &[
    "code-block-meta",
    "directory-too-deep",
    "duplicate-h1",
    "duplicate-label",
    "frontmatter-unknown-key",
    "fullwidth-alphanumeric",
    "halfwidth-kana",
    "heading-level-skip",
    "katakana-choon",
    "spec-warning",
    "term-variant",
];

fn default_lint_rules() -> BTreeMap<String, bool> {
    DISABLEABLE_RULES
        .iter()
        .map(|id| ((*id).to_string(), true))
        .collect()
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
    /// 同義語グループ（例: `[["ログイン", "サインイン"]]`）。
    /// `lint.terms` の辞書と合成され、ゆれ表記での検索が正表記の文書にヒットする
    pub synonyms: Vec<Vec<String>>,
    /// フェンスコードブロックの本文を検索インデックスに含めるか（既定 false）。
    /// on にすると関数名・設定キー等コード内の語で引ける。特別レンダリングされる
    /// 言語（mermaid / openapi / jsonschema / math）は on でも索引しない
    /// （ただし mermaid / math を設定で無効化しプレーンコード表示になる場合は索引する）。
    /// インデントコードブロック（非フェンス）は常に対象外。コード本文は抜粋用
    /// fragment にもそのまま入るため、巨大なコードブロックは配信サイズに影響する
    pub index_code: bool,
    /// 検索結果ページの route（content 相対・拡張子なし。例 `"search"`）。
    /// **空なら生成しない**（既定）。既存プロジェクトの `content/search.md` と
    /// route が衝突してビルド不能になるのを避けるため、明示オプトインにする
    pub page: String,
    /// 検索結果ページのタイトル
    pub page_title: String,
    /// 検索結果ページで 1 回に表示する件数（「さらに表示」で追加）
    pub page_size: u32,
}

impl Default for SearchConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            dictionary: None,
            typo_tolerance: TypoToleranceConfig::default(),
            shard: ShardConfig::default(),
            synonyms: Vec::new(),
            index_code: false,
            page: String::new(),
            page_title: "検索".to_string(),
            // `yuzu search --limit` の既定と揃える（実装は独立。CLI 側は cli.rs）
            page_size: 10,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct BuildConfig {
    /// ビルド時の baseUrl 上書き（`site.baseUrl` より優先）
    pub base_url: Option<String>,
    /// `yuzu dev` / `yuzu build --watch` の監視から除外する glob。
    /// プロジェクトルート相対・`/` 区切りで評価し、**当たったディレクトリの
    /// 配下もすべて除外**する（`**/target` で `target/debug/x` も除外）。
    /// **指定すると既定値を置き換える**（追記ではない）。
    /// 出力ディレクトリと隠しディレクトリ（`.git` / `.yuzu`）は指定に関係なく常に除外
    pub watch_ignore: Vec<String>,
}

impl Default for BuildConfig {
    fn default() -> Self {
        Self {
            base_url: None,
            // ビルド生成物・依存物を既定で除外する。これが無いと `target/` の
            // 1 回のコンパイルで大量のイベントが飛び、再ビルドが暴発する
            // （`[]` を書けば外せる）
            watch_ignore: vec!["**/target".to_string(), "**/node_modules".to_string()],
        }
    }
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
