use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::{CONFIG_FILE_NAME, Config, ConfigError};

/// ユーザテーマディレクトリ名（プロジェクトルート直下）
const THEME_DIR_NAME: &str = "theme";
/// 静的物パススルーのディレクトリ名（プロジェクトルート直下）
const PUBLIC_DIR_NAME: &str = "public";
/// ツール管理ディレクトリ名
const YUZU_DIR_NAME: &str = ".yuzu";

/// デフォルトをマージし、パスと baseUrl を解決した設定
#[derive(Debug, Clone)]
pub struct ResolvedConfig {
    pub config: Config,
    /// プロジェクトルート（`yuzu.jsonc` のあるディレクトリ）
    pub root: PathBuf,
    pub content_dir: PathBuf,
    pub output_dir: PathBuf,
    /// プロジェクトの `theme/` が存在する場合のみ Some（埋め込みテーマの上書き元）
    pub theme_dir: Option<PathBuf>,
    /// `public/` が存在する場合のみ Some
    pub public_dir: Option<PathBuf>,
    /// `build.baseUrl` ?? `site.baseUrl` ?? "/" を正規化したもの。
    /// パス形は常に先頭・末尾スラッシュ付き（`/` または `/docs/`）
    pub base_url: String,
    /// 設定ファイル自体の診断（重複キー・未知キー）。
    /// `yuzu lint` / `check` が診断として報告し、他コマンドは load 時の警告で済ませる
    pub diagnostics: Vec<ConfigDiagnostic>,
}

/// プロジェクトルートの `yuzu.jsonc` を読み込み、解決済み設定を返す
pub fn load(root: &Path) -> Result<ResolvedConfig, ConfigError> {
    let path = root.join(CONFIG_FILE_NAME);
    let text = fs::read_to_string(&path).map_err(|source| ConfigError::Io {
        path: path.clone(),
        source,
    })?;

    // 構文エラー（JSONC）とスキーマ不一致を別エラーで報告するため、
    // いったん serde_json::Value を経由する
    let value: serde_json::Value =
        jsonc_parser::parse_to_serde_value(&text, &jsonc_parser::ParseOptions::default()).map_err(
            |e| ConfigError::Jsonc {
                path: path.clone(),
                message: e.to_string(),
            },
        )?;

    let config: Config = serde_json::from_value(value).map_err(|source| ConfigError::Schema {
        path: path.clone(),
        source,
    })?;

    // 重複キーは後勝ちで黙って上書きされ、未知キー（タイポ）は無言で無視される。
    // どちらも「設定したのに効かない」事故になりやすい（実運用で複数回発生）。
    // `yuzu lint` / `check` は診断として報告し、それ以外のコマンドはここの警告で気づかせる
    let diagnostics = config_diagnostics(&text);
    for d in &diagnostics {
        tracing::warn!("yuzu.jsonc:{}:{}: {}", d.line, d.col, d.message);
    }

    let base_url = normalize_base_url(
        config
            .build
            .base_url
            .as_deref()
            .or(config.site.base_url.as_deref())
            .unwrap_or("/"),
    );

    let theme_dir = Some(root.join(THEME_DIR_NAME)).filter(|p| p.is_dir());
    let public_dir = Some(root.join(PUBLIC_DIR_NAME)).filter(|p| p.is_dir());

    Ok(ResolvedConfig {
        content_dir: root.join(&config.input.dir),
        output_dir: root.join(&config.output.dir),
        theme_dir,
        public_dir,
        base_url,
        root: root.to_path_buf(),
        config,
        diagnostics,
    })
}

/// `yuzu.jsonc` に対する診断 1 件。
///
/// yuzu-config は yuzu-core に依存しない（凍結した依存グラフでは葉）ため、
/// `yuzu_core::Diagnostic` ではなく中立な値型で返し、cli 側で変換する
#[derive(Debug, Clone)]
pub struct ConfigDiagnostic {
    /// ルール ID（`config-unknown-key` / `config-duplicate-key`）
    pub rule: &'static str,
    /// キーのパス（`markdown.crossref.numbering` 形式）
    pub key_path: String,
    /// 1 始まりの行
    pub line: usize,
    /// 1 始まりの列（バイト基準。診断の列規約に合わせる）
    pub col: usize,
    pub message: String,
}

/// 自由キーのマップ（配下はユーザ任意の名前なので未知キー検査をしない）
const FREE_FORM_PATHS: &[&str] = &["theme.cssVars", "theme.cssVarsDark", "lint.terms"];

/// バイトオフセットを 1 始まりの (行, 列) へ変換する
fn line_col(text: &str, offset: usize) -> (usize, usize) {
    let head = &text[..offset.min(text.len())];
    let line = head.matches('\n').count() + 1;
    let col = head.rsplit_once('\n').map_or(head.len(), |(_, l)| l.len()) + 1;
    (line, col)
}

/// 既知キーの木。`Config::default()` を JSON 化して実行時に得るので、
/// 手書きの定数と構造体がズレる事故が起きない（frontmatter の KNOWN_KEYS と違う点）
fn known_key_tree() -> serde_json::Value {
    serde_json::to_value(Config::default()).unwrap_or(serde_json::Value::Null)
}

/// `yuzu.jsonc` を走査して重複キー・未知キーを診断する。
/// 構文エラー時は空（本体パースが別途エラーを報告する）
pub(crate) fn config_diagnostics(text: &str) -> Vec<ConfigDiagnostic> {
    use jsonc_parser::ast::Value;
    use jsonc_parser::common::Ranged;

    fn walk(
        value: &Value,
        path: &str,
        known: Option<&serde_json::Value>,
        text: &str,
        out: &mut Vec<ConfigDiagnostic>,
    ) {
        match value {
            Value::Object(obj) => {
                let mut seen = std::collections::HashSet::new();
                for prop in &obj.properties {
                    let name = prop.name.as_str();
                    let child = if path.is_empty() {
                        name.to_string()
                    } else {
                        format!("{path}.{name}")
                    };
                    let (line, col) = line_col(text, prop.name.range().start);
                    if !seen.insert(name.to_string()) {
                        out.push(ConfigDiagnostic {
                            rule: "config-duplicate-key",
                            key_path: child.clone(),
                            line,
                            col,
                            message: format!(
                                "キー `{child}` が重複しています（JSONC は後勝ちのため、先に書いた方は無視されます）"
                            ),
                        });
                    }
                    // 既知キーの木を同時に降下する。木が非オブジェクトになったら
                    // そこから先は値なので検査しない（enum 値や配列の中身など）
                    let child_known = known.and_then(|k| k.get(&*name));
                    if known.is_some_and(serde_json::Value::is_object) && child_known.is_none() {
                        let siblings = known
                            .and_then(|k| k.as_object())
                            .map(|m| m.keys().cloned().collect::<Vec<_>>().join("/"))
                            .unwrap_or_default();
                        out.push(ConfigDiagnostic {
                            rule: "config-unknown-key",
                            key_path: child.clone(),
                            line,
                            col,
                            message: format!(
                                "未知のキー `{child}` があります（この階層の対応キー: {siblings}）"
                            ),
                        });
                        continue; // 未知キーの配下は検査しない（誤検知が連鎖する）
                    }
                    if FREE_FORM_PATHS.contains(&child.as_str()) {
                        continue; // 配下はユーザ任意の名前
                    }
                    walk(&prop.value, &child, child_known, text, out);
                }
            }
            Value::Array(arr) => {
                for (i, v) in arr.elements.iter().enumerate() {
                    // 配列要素は既知キーの木を持たない（中身は値）
                    walk(v, &format!("{path}[{i}]"), None, text, out);
                }
            }
            _ => {}
        }
    }

    let Ok(result) = jsonc_parser::parse_to_ast(
        text,
        &jsonc_parser::CollectOptions::default(),
        &jsonc_parser::ParseOptions::default(),
    ) else {
        return Vec::new();
    };
    let known = known_key_tree();
    let mut out = Vec::new();
    if let Some(root) = &result.value {
        walk(root, "", Some(&known), text, &mut out);
    }
    out.sort_by_key(|d| (d.line, d.col));
    out
}

/// 解決済み設定を `.yuzu/settings.json` に書き出す
pub fn write_resolved(rc: &ResolvedConfig) -> Result<PathBuf, ConfigError> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Settings<'a> {
        config: &'a Config,
        root: &'a Path,
        content_dir: &'a Path,
        output_dir: &'a Path,
        theme_dir: Option<&'a Path>,
        public_dir: Option<&'a Path>,
        base_url: &'a str,
    }

    let dir = rc.root.join(YUZU_DIR_NAME);
    fs::create_dir_all(&dir).map_err(|source| ConfigError::Io {
        path: dir.clone(),
        source,
    })?;

    let path = dir.join("settings.json");
    let settings = Settings {
        config: &rc.config,
        root: &rc.root,
        content_dir: &rc.content_dir,
        output_dir: &rc.output_dir,
        theme_dir: rc.theme_dir.as_deref(),
        public_dir: rc.public_dir.as_deref(),
        base_url: &rc.base_url,
    };
    let json = serde_json::to_string_pretty(&settings).expect("設定は常に JSON 化できる");
    fs::write(&path, json + "\n").map_err(|source| ConfigError::Io {
        path: path.clone(),
        source,
    })?;
    Ok(path)
}

/// baseUrl を「常に先頭・末尾スラッシュ付き」の形へ正規化する。
/// フル URL（`https://…`）は末尾スラッシュのみ保証する。
/// CLI の `--base-url` 上書き（CI から configure-pages の base_path を渡す用途）でも使う
pub fn normalize_base_url(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return "/".to_string();
    }
    if trimmed.contains("://") {
        let mut s = trimmed.to_string();
        if !s.ends_with('/') {
            s.push('/');
        }
        return s;
    }
    let core = trimmed.trim_matches('/');
    if core.is_empty() {
        return "/".to_string();
    }
    format!("/{core}/")
}

#[cfg(test)]
mod tests {
    use super::{config_diagnostics, normalize_base_url};

    /// (ルール, キーパス, 行) の並び
    fn found(text: &str) -> Vec<(&'static str, String, usize)> {
        config_diagnostics(text)
            .into_iter()
            .map(|d| (d.rule, d.key_path, d.line))
            .collect()
    }

    #[test]
    fn 重複キーをパス付きで検出する() {
        let text = r#"{
          // コメントや入れ子があっても検出できる
          "dev": { "port": 5173 },
          "site": { "title": "a", "title": "b" },
          "dev": { "host": "0.0.0.0" }
        }"#;
        let dups: Vec<_> = found(text)
            .into_iter()
            .filter(|(rule, ..)| *rule == "config-duplicate-key")
            .map(|(_, path, _)| path)
            .collect();
        assert_eq!(dups, ["site.title", "dev"]);
    }

    #[test]
    fn 重複キーには行と列が付く() {
        let text = "{\n  \"site\": { \"title\": \"a\" },\n  \"site\": { \"title\": \"b\" }\n}";
        let diags = config_diagnostics(text);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].line, 3);
        assert_eq!(diags[0].col, 3);
    }

    #[test]
    fn 問題がなければ空() {
        assert!(config_diagnostics(r#"{ "site": { "title": "a" } }"#).is_empty());
        assert!(
            config_diagnostics("{ broken").is_empty(),
            "構文エラーは対象外（本体パースが報告する）"
        );
    }

    #[test]
    fn 未知のトップレベルキーを検出する() {
        let diags = found(r#"{ "markdwon": { "gfm": true } }"#);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].0, "config-unknown-key");
        assert_eq!(diags[0].1, "markdwon");
    }

    #[test]
    fn 入れ子の未知キーも検出する() {
        let diags = found(r#"{ "markdown": { "crossreff": { "numbering": "site" } } }"#);
        assert_eq!(diags.len(), 1, "{diags:?}");
        assert_eq!(diags[0].1, "markdown.crossreff");
    }

    #[test]
    fn 未知キーの配下は検査しない() {
        // 親が未知なら子も当然未知だが、報告は親の 1 件だけにする
        let diags = found(r#"{ "typo": { "a": 1, "b": { "c": 2 } } }"#);
        assert_eq!(diags.len(), 1, "{diags:?}");
    }

    #[test]
    fn 自由キーのマップは未知キー扱いしない() {
        // cssVars / cssVarsDark / lint.terms はユーザ任意の名前
        assert!(
            config_diagnostics(r##"{ "theme": { "cssVars": { "--accent": "#0a6cff" } } }"##)
                .is_empty()
        );
        assert!(
            config_diagnostics(r#"{ "lint": { "terms": { "サーバ": ["サーバー"] } } }"#).is_empty()
        );
    }

    #[test]
    fn 既知キーは値の型によらず通る() {
        // enum 値・配列・null の中身へは降りない
        let text = r#"{
          "markdown": { "mermaid": { "backend": "ssr" } },
          "input": { "ignore": ["**/_drafts/**"] },
          "site": { "description": null }
        }"#;
        assert!(config_diagnostics(text).is_empty(), "{:?}", found(text));
    }

    #[test]
    fn base_url_の正規化() {
        assert_eq!(normalize_base_url(""), "/");
        assert_eq!(normalize_base_url("/"), "/");
        assert_eq!(normalize_base_url("docs"), "/docs/");
        assert_eq!(normalize_base_url("/docs"), "/docs/");
        assert_eq!(normalize_base_url("docs/"), "/docs/");
        assert_eq!(normalize_base_url("/docs/"), "/docs/");
        assert_eq!(normalize_base_url("/a/b"), "/a/b/");
        assert_eq!(
            normalize_base_url("https://example.com/docs"),
            "https://example.com/docs/"
        );
    }
}
