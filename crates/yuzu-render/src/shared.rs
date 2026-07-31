//! watch / dev セッションで再利用する重い共有状態。
//!
//! 設定はセッション中固定（yuzu.jsonc の変更は再起動で反映）なので、
//! syntect ハイライタ（two_face 構文セット）と syntect CSS は不変。
//! minijinja Env はテーマ（theme/templates/）変更時のみ再構築する。
//!
//! 既知の限界: debug ビルドの rust-embed は埋め込みテーマを FS から読むため、
//! yuzu 本体の開発者がセッション中に**埋め込み側**テンプレートを編集しても
//! Env には反映されない（プロジェクトの theme/ 上書きは反映される）

use std::path::Path;

use minijinja::Environment;

use yuzu_config::ResolvedConfig;

use crate::css;
use crate::error::RenderError;
use crate::highlight::SyntectCodeRenderer;
use crate::templates;

pub struct RenderShared {
    pub(crate) env: Environment<'static>,
    pub(crate) highlighter: SyntectCodeRenderer,
    pub(crate) syntect_css: String,
}

impl RenderShared {
    pub fn new(rc: &ResolvedConfig) -> Result<Self, RenderError> {
        let cfg = &rc.config;
        let mut highlighter =
            SyntectCodeRenderer::new(&cfg.markdown.highlight, &cfg.markdown.mermaid);
        // openapi/jsonschema の `file:` 参照はプロジェクトルート相対
        highlighter.set_project_root(rc.root.clone());
        // ハイライト無効なら syntect テーマの読み込みも CSS 生成もしない
        // （使わないテーマ名で UnknownHighlightTheme になるのは不自然）。
        // ただし base.jinja は syntect.css を**無条件で** <link> するため、
        // ファイル自体は空の内容で書き出して 404 を出さない
        let syntect_css = if cfg.markdown.highlight.enabled {
            css::generate_syntect_css(
                &cfg.markdown.highlight.theme_light,
                &cfg.markdown.highlight.theme_dark,
            )?
        } else {
            "/* markdown.highlight.enabled: false のため空です */\n".to_string()
        };
        Ok(Self {
            env: templates::build_env(rc.theme_dir.as_deref())?,
            highlighter,
            syntect_css,
        })
    }

    /// テーマ（theme/templates/）変更時に Env だけ作り直す
    pub fn reload_templates(&mut self, theme_dir: Option<&Path>) -> Result<(), RenderError> {
        self.env = templates::build_env(theme_dir)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ハイライト無効なら_syntect_css_は空でテーマ名を検証しない() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("yuzu.jsonc"),
            r#"{ "markdown": { "highlight": {
                 "enabled": false, "themeLight": "存在しないテーマ" } } }"#,
        )
        .unwrap();
        let rc = yuzu_config::load(dir.path()).unwrap();

        let shared = RenderShared::new(&rc).expect("使わないテーマ名で落ちない");
        assert!(shared.syntect_css.contains("highlight.enabled: false"));
    }
}
