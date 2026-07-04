//! syntect テーマ → CSS の生成（ライト/ダーク両対応）。
//!
//! ライト側の CSS をそのまま出し、ダーク側は各ルールのセレクタに
//! `html[data-theme="dark"]` を前置してスコープする。
//! syntect の生成 CSS はフラットなクラスルール列なので、この文字列処理で安全。

use syntect::highlighting::ThemeSet;
use syntect::html::css_for_theme_with_class_style;

use crate::error::RenderError;
use crate::highlight::CLASS_STYLE;

const DARK_SCOPE: &str = "html[data-theme=\"dark\"]";

/// 設定されたライト/ダークのテーマ名から `syntect.css` の中身を生成する
pub(crate) fn generate_syntect_css(light: &str, dark: &str) -> Result<String, RenderError> {
    let themes = ThemeSet::load_defaults();
    let get = |name: &str| {
        themes
            .themes
            .get(name)
            .ok_or_else(|| RenderError::UnknownHighlightTheme {
                name: name.to_string(),
            })
    };
    let light_css = css_for_theme_with_class_style(get(light)?, CLASS_STYLE)?;
    let dark_css = css_for_theme_with_class_style(get(dark)?, CLASS_STYLE)?;

    Ok(format!(
        "/* yuzu build が生成（light: {light} / dark: {dark}）。手で編集しない */\n\n{light_css}\n{}",
        scope_css(&dark_css, DARK_SCOPE)
    ))
}

/// フラットな CSS のトップレベルセレクタへ `scope` を前置する
fn scope_css(css: &str, scope: &str) -> String {
    let mut out = String::with_capacity(css.len() + 1024);
    let mut depth: usize = 0;
    for line in css.lines() {
        let trimmed = line.trim_start();
        let is_selector_line = depth == 0
            && trimmed.contains('{')
            && !trimmed.starts_with("/*")
            && !trimmed.starts_with('@');
        if is_selector_line {
            // `sel1, sel2 { …` → `scope sel1, scope sel2 { …`
            let (selectors, rest) = line.split_once('{').expect("contains('{') 確認済み");
            let scoped = selectors
                .split(',')
                .map(|s| format!("{scope} {}", s.trim()))
                .collect::<Vec<_>>()
                .join(", ");
            out.push_str(&scoped);
            out.push_str(" {");
            out.push_str(rest);
        } else {
            out.push_str(line);
        }
        out.push('\n');
        depth += line.matches('{').count();
        depth = depth.saturating_sub(line.matches('}').count());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scope_css_はトップレベルセレクタだけに前置する() {
        let css = "/* comment */\n.yz-code {\n color: #111;\n}\n.yz-a, .yz-b {\n color: #222;\n}\n";
        let scoped = scope_css(css, "html[data-theme=\"dark\"]");
        assert!(scoped.contains("html[data-theme=\"dark\"] .yz-code {"));
        assert!(
            scoped.contains("html[data-theme=\"dark\"] .yz-a, html[data-theme=\"dark\"] .yz-b {")
        );
        assert!(scoped.contains("/* comment */"));
        // 宣言行はそのまま
        assert!(scoped.contains(" color: #111;"));
    }

    #[test]
    fn デフォルトテーマ名で生成できる() {
        let css = generate_syntect_css("InspiredGitHub", "base16-ocean.dark").unwrap();
        assert!(css.contains("yz-"), "接頭辞付きクラス: {}", &css[..200]);
        assert!(css.contains("html[data-theme=\"dark\"]"));
    }

    #[test]
    fn 不明なテーマ名はエラー() {
        assert!(generate_syntect_css("no-such-theme", "base16-ocean.dark").is_err());
    }
}
