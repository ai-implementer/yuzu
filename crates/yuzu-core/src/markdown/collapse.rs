//! Admonition の折りたたみ（`> [!NOTE]- タイトル`）。
//!
//! GitHub 互換の Admonition（`> [!NOTE]`）の種別直後に `-` / `+` を付けると
//! `<details>` / `<details open>` で描画する（Obsidian callouts と同じ記法）。
//! ネイティブ要素なのでクライアント JS は不要。
//!
//! comrak は `[!NOTE]- タイトル` の `-` を**タイトルの一部**として解釈するため
//! （タイトルは `"- タイトル"` になる）、ここで接頭辞を剥がして判定する。
//! `yuzu fmt` は `[!NOTE] - タイトル` の形へ正規化するが、解釈は変わらず冪等。

use comrak::nodes::AlertType;

/// 折りたたみの指定（種別直後のマーカー）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Collapse {
    /// `-`: 閉じた状態で表示する
    Closed,
    /// `+`: 開いた状態で表示する（折りたためる）
    Open,
}

/// Admonition のタイトルから折りたたみ指定と表示タイトルを取り出す。
/// マーカーが無ければ `None`（従来どおりの `<div>` 描画）
pub(crate) fn parse_title(title: Option<&str>) -> Option<(Collapse, String)> {
    let title = title?.trim_start();
    let (collapse, rest) = match title.strip_prefix('-') {
        Some(rest) => (Collapse::Closed, rest),
        None => (Collapse::Open, title.strip_prefix('+')?),
    };
    Some((collapse, rest.trim().to_string()))
}

/// `<details>` の開始タグ（`<summary>` 込み）。
/// タイトル省略時は comrak と同じ既定タイトル（Note / Tip …）を使う
pub(crate) fn open_tag(kind: AlertType, collapse: Collapse, title: &str) -> String {
    let title = if title.is_empty() {
        kind.default_title().to_string()
    } else {
        escape_html(title)
    };
    let open_attr = match collapse {
        Collapse::Open => " open",
        Collapse::Closed => "",
    };
    format!(
        "<details class=\"markdown-alert {}\"{open_attr}>\n<summary class=\"markdown-alert-title\">{title}</summary>\n",
        kind.css_class(),
    )
}

/// `<details>` の終了タグ
pub(crate) const CLOSE_TAG: &str = "</details>\n";

/// HTML エスケープ（テキストノード用の最小集合）
fn escape_html(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn マーカーで折りたたみ指定を判定する() {
        assert_eq!(
            parse_title(Some("- 詳細")),
            Some((Collapse::Closed, "詳細".to_string()))
        );
        assert_eq!(
            parse_title(Some("+ 詳細")),
            Some((Collapse::Open, "詳細".to_string()))
        );
        // マーカーのみ（タイトル省略）
        assert_eq!(
            parse_title(Some("-")),
            Some((Collapse::Closed, String::new()))
        );
        // fmt が入れる空白（`[!NOTE] - タイトル`）でも同じ解釈
        assert_eq!(
            parse_title(Some(" - 詳細")),
            Some((Collapse::Closed, "詳細".to_string()))
        );
    }

    #[test]
    fn マーカーなしは折りたたまない() {
        assert_eq!(parse_title(None), None);
        assert_eq!(parse_title(Some("独自タイトル")), None);
        // 本文中のハイフンは先頭でなければ影響しない
        assert_eq!(parse_title(Some("A - B")), None);
    }

    #[test]
    fn 開始タグは種別と状態を反映する() {
        let html = open_tag(AlertType::Note, Collapse::Closed, "詳細");
        assert_eq!(
            html,
            "<details class=\"markdown-alert markdown-alert-note\">\n<summary class=\"markdown-alert-title\">詳細</summary>\n"
        );
        // open 指定
        let html = open_tag(AlertType::Tip, Collapse::Open, "ヒント");
        assert!(html.contains("markdown-alert-tip"), "{html}");
        assert!(
            html.contains("<details class=\"markdown-alert markdown-alert-tip\" open>"),
            "{html}"
        );
        // タイトル省略時は comrak と同じ既定タイトル
        let html = open_tag(AlertType::Caution, Collapse::Closed, "");
        assert!(html.contains(">Caution</summary>"), "{html}");
    }

    #[test]
    fn タイトルはエスケープされる() {
        let html = open_tag(AlertType::Warning, Collapse::Closed, "<script> & \"x\"");
        assert!(
            html.contains("&lt;script&gt; &amp; &quot;x&quot;</summary>"),
            "{html}"
        );
    }
}
