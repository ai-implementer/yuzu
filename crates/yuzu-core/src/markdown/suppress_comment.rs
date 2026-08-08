//! 行単位の抑制コメント（`<!-- yuzu-lint-disable-next-line … -->`）の文字列解釈。
//!
//! `fence.rs` と同じく comrak 非依存の純粋なパーサ。AST からの抽出
//! （`mod.rs::extract_suppress_comments`）・fmt の密着形復元
//! （`restore_yuzu_syntax`）・照合（`suppress.rs`）が**同じ 1 実装**を共有する
//! （別々に解釈すると空行判定やマーカー判定が必ずズレる）。

/// 対応しているディレクティブ（次の内容行の診断を抑制する）
pub(crate) const NEXT_LINE_DIRECTIVE: &str = "yuzu-lint-disable-next-line";

/// 抑制コメントの共通接頭辞。これで始まるコメントだけを yuzu の記法として
/// 解釈し、それ以外の HTML コメントには一切干渉しない
const DIRECTIVE_PREFIX: &str = "yuzu-lint-";

/// コメント行の分類結果
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SuppressCommentKind {
    /// 正しい next-line 指定（ルール名 1 個以上。重複は呼び出し側で畳む）
    NextLine { rules: Vec<String> },
    /// ルール名なしの裸コメント
    Empty,
    /// `yuzu-lint-` 接頭だが未知のディレクティブ
    /// （`yuzu-lint-disable-line` 等の将来語彙もここで拾う = 黙って効かない事故を防ぐ）
    UnknownDirective { directive: String },
    /// 閉じ `-->` が同じ行に無い（閉じ忘れは以降の本文を丸ごと飲み込む）
    Unclosed,
    /// 単独の行になっていない（`-->` の後に本文が続く・段落中のインライン位置）
    NotStandalone,
}

/// コメント 1 行（HtmlBlock literal の 1 行目、または HtmlInline literal）を分類する。
/// `<!--` で始まらない・`yuzu-lint-` 接頭でないものは None（対象外の普通のコメント）
pub(crate) fn classify_comment_line(line: &str) -> Option<SuppressCommentKind> {
    let trimmed = line.trim();
    let inner = trimmed.strip_prefix("<!--")?.trim_start();
    if !inner.starts_with(DIRECTIVE_PREFIX) {
        return None;
    }
    let Some(close) = inner.find("-->") else {
        return Some(SuppressCommentKind::Unclosed);
    };
    if !inner[close + 3..].trim().is_empty() {
        return Some(SuppressCommentKind::NotStandalone);
    }
    let mut tokens = inner[..close].split_whitespace();
    let directive = tokens.next().expect("接頭辞つきなので必ず 1 トークンある");
    if directive != NEXT_LINE_DIRECTIVE {
        return Some(SuppressCommentKind::UnknownDirective {
            directive: directive.to_string(),
        });
    }
    let rules: Vec<String> = tokens.map(str::to_string).collect();
    if rules.is_empty() {
        return Some(SuppressCommentKind::Empty);
    }
    Some(SuppressCommentKind::NextLine { rules })
}

/// fmt 復元用の述語: この行（blockquote の `> ` 込み）は 1 行完結の抑制コメントか。
/// 未知ディレクティブ等の invalid でも true（修正中の原稿でも密着形を保つ）
pub(crate) fn is_suppress_comment_line(line: &str) -> bool {
    let s = line.trim_start_matches(['>', ' ', '\t']).trim_end();
    s.ends_with("-->")
        && s.strip_prefix("<!--")
            .is_some_and(|inner| inner.trim_start().starts_with(DIRECTIVE_PREFIX))
}

/// 「内容として空の行」判定（空白のみ・blockquote の `>` マーカーのみの行）。
/// 照合の「空行を飛ばした次の内容行」と fmt 復元の空行判定が共有する
pub(crate) fn is_content_blank(line: &str) -> bool {
    line.trim_start_matches(['>', ' ', '\t']).trim().is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 空白区切りの複数ルール名と前後空白を許容する() {
        let kind = classify_comment_line(
            "<!-- yuzu-lint-disable-next-line term-variant katakana-choon -->",
        );
        assert_eq!(
            kind,
            Some(SuppressCommentKind::NextLine {
                rules: vec!["term-variant".to_string(), "katakana-choon".to_string()]
            })
        );
        // `<!--` 直後の空白なし・タブ・連続空白も受理
        let kind = classify_comment_line(
            "<!--yuzu-lint-disable-next-line\tterm-variant   duplicate-h1-->",
        );
        assert_eq!(
            kind,
            Some(SuppressCommentKind::NextLine {
                rules: vec!["term-variant".to_string(), "duplicate-h1".to_string()]
            })
        );
        // 行頭インデント（コメント行自体の字下げ）も許容
        assert!(matches!(
            classify_comment_line("  <!-- yuzu-lint-disable-next-line term-variant -->"),
            Some(SuppressCommentKind::NextLine { .. })
        ));
    }

    #[test]
    fn 未知ディレクティブと裸コメントと閉じ忘れを分類する() {
        // 予約語彙（disable-line）は未知ディレクティブとして拾う
        assert_eq!(
            classify_comment_line("<!-- yuzu-lint-disable-line term-variant -->"),
            Some(SuppressCommentKind::UnknownDirective {
                directive: "yuzu-lint-disable-line".to_string()
            })
        );
        assert_eq!(
            classify_comment_line("<!-- yuzu-lint-disable-next-line -->"),
            Some(SuppressCommentKind::Empty)
        );
        assert_eq!(
            classify_comment_line("<!-- yuzu-lint-disable-next-line term-variant"),
            Some(SuppressCommentKind::Unclosed)
        );
        assert_eq!(
            classify_comment_line("<!-- yuzu-lint-disable-next-line term-variant --> 本文"),
            Some(SuppressCommentKind::NotStandalone)
        );
    }

    #[test]
    fn yuzu_lint_接頭でないコメントは対象外() {
        assert_eq!(classify_comment_line("<!-- ただのメモ -->"), None);
        assert_eq!(
            classify_comment_line("<!-- TODO: yuzu-lint を検討 -->"),
            None
        );
        assert_eq!(classify_comment_line("ただの本文"), None);
    }

    #[test]
    fn 述語と空行判定は_blockquote_マーカーを剥がして判定する() {
        assert!(is_suppress_comment_line(
            "> <!-- yuzu-lint-disable-next-line term-variant -->"
        ));
        assert!(
            is_suppress_comment_line("<!-- yuzu-lint-disable-typo -->"),
            "invalid でも密着形は保つ"
        );
        assert!(!is_suppress_comment_line("<!-- ただのメモ -->"));
        assert!(
            !is_suppress_comment_line("<!-- yuzu-lint-disable-next-line x"),
            "閉じの無い行は対象外"
        );

        assert!(is_content_blank(""));
        assert!(is_content_blank("   "));
        assert!(is_content_blank(">"));
        assert!(is_content_blank("> "));
        assert!(!is_content_blank("> 本文"));
    }
}
