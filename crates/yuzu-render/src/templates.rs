//! minijinja 環境の構築。
//!
//! テンプレート解決の優先順:
//! 1. プロジェクトの `theme/templates/<name>`（部分上書き可）
//! 2. 埋め込みデフォルトテーマ（`yuzu-theme`）

use std::fs;
use std::path::{Path, PathBuf};

use minijinja::{AutoEscape, Environment};

use crate::error::RenderError;

pub(crate) fn build_env(theme_dir: Option<&Path>) -> Result<Environment<'static>, RenderError> {
    let mut env = Environment::new();
    // テンプレート名の拡張子（.jinja）に関わらず常に HTML エスケープする。
    // 本文 HTML はテンプレート側で `| safe` を通す
    env.set_auto_escape_callback(|_| AutoEscape::Html);

    let override_dir: Option<PathBuf> = theme_dir.map(|d| d.join("templates"));
    env.set_loader(move |name| {
        if let Some(dir) = &override_dir {
            let path = dir.join(name);
            if path.is_file() {
                let text = fs::read_to_string(&path).map_err(|e| {
                    minijinja::Error::new(
                        minijinja::ErrorKind::InvalidOperation,
                        format!("テーマテンプレート {} を読めません: {e}", path.display()),
                    )
                })?;
                return Ok(Some(text));
            }
        }
        match yuzu_theme::get(&format!("templates/{name}")) {
            Some(data) => Ok(Some(String::from_utf8_lossy(&data).into_owned())),
            None => Ok(None),
        }
    });

    Ok(env)
}

#[cfg(test)]
mod tests {
    use super::build_env;

    #[test]
    fn 埋め込みテンプレートをロードできる() {
        let env = build_env(None).unwrap();
        assert!(env.get_template("page.jinja").is_ok());
        assert!(env.get_template("base.jinja").is_ok());
        assert!(env.get_template("no-such.jinja").is_err());
    }
}
