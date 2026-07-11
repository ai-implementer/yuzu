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
    })
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
    use super::normalize_base_url;

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
