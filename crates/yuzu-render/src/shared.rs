//! watch / dev セッションで再利用する重い共有状態。
//!
//! 設定はセッション中固定（yuzu.toml の変更は cli がセッションを作り直して反映）なので、
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
    /// 生成済み syntect.css。`markdown.highlight.enabled: false` なら None
    /// （ファイルを書き出さず、base.jinja の `<link>` も `highlight_enabled` で消える）
    pub(crate) syntect_css: Option<String>,
}

impl RenderShared {
    pub fn new(rc: &ResolvedConfig) -> Result<Self, RenderError> {
        let cfg = &rc.config;
        let mut highlighter =
            SyntectCodeRenderer::new(&cfg.markdown.highlight, &cfg.markdown.mermaid);
        // openapi/jsonschema の `file:` 参照はプロジェクトルート相対
        highlighter.set_project_root(rc.root.clone());
        // ハイライト無効なら syntect テーマの読み込みも CSS 生成もしない
        // （使わないテーマ名で UnknownHighlightTheme になるのは不自然）
        let syntect_css = cfg
            .markdown
            .highlight
            .enabled
            .then(|| {
                css::generate_syntect_css(
                    &cfg.markdown.highlight.theme_light,
                    &cfg.markdown.highlight.theme_dark,
                )
            })
            .transpose()?;
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
    fn ハイライト無効なら_syntect_css_は生成せずテーマ名も検証しない() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("yuzu.toml"),
            "[markdown.highlight]\nenabled = false\ntheme_light = \"存在しないテーマ\"\n",
        )
        .unwrap();
        let rc = yuzu_config::load(dir.path()).unwrap();

        let shared = RenderShared::new(&rc).expect("使わないテーマ名で落ちない");
        assert!(shared.syntect_css.is_none());
    }
}
