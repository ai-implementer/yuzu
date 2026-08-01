//! タブ / コードグループ（` ```rust tab="Rust" `）。
//!
//! **隣接する** `tab=` 付きフェンスを 1 つのタブグループへ束ねる。素の Markdown
//! ビューアではコードが縦に並ぶだけなので壊れない（判断軸「素のビューアで壊れない」）。
//!
//! 切り替えは radio + CSS の `order` だけで、クライアント JS はゼロ。
//! ラジオの `name` はページ内で一意な連番にする（同じページに複数グループが
//! 置かれたとき、名前が衝突すると別グループのタブが巻き添えで切り替わる）。
//!
//! 記法として `:::tabs`（comrak の `block_directive`）を採らなかった理由は
//! ROADMAP の Phase 50 を参照（同じ長さのフェンスがネストできない・info が
//! 丸ごと class に入る・素のビューアで `:::` が見える）。

use comrak::nodes::{AstNode, NodeValue};

use crate::MarkdownOptions;
use crate::markdown::escape_html;
use crate::markdown::fence::parse_fence_info;

/// このノードが `tab=` 付きのコードブロックなら見出しを返す。
/// 特別レンダリング言語（mermaid / 仕様 / math）は表示メタを無視する契約なので除く
fn tab_label(node: &AstNode, opts: &MarkdownOptions) -> Option<String> {
    let data = node.data.borrow();
    let NodeValue::CodeBlock(cb) = &data.value else {
        return None;
    };
    let (lang, meta) = parse_fence_info(&cb.info);
    if lang.is_some_and(|l| crate::is_special_render_lang(l, opts)) {
        return None;
    }
    meta.tab
}

/// タブグループを作れるか＝**隣接する兄弟にも `tab=` があるか**。
///
/// 描画（`render_body_html`）と lint（`code-block-meta`）が**同じ判定を共有する**。
/// 片方だけ変えると「lint は黙るのに `tab=` が効かない」という、いちばん
/// 気づけない壊れ方になる。
///
/// 1 枚だけのタブをグループにしないのは、切り替え先が無くラベルだけが浮くため。
pub(crate) fn has_tab_neighbor(node: &AstNode, opts: &MarkdownOptions) -> bool {
    if tab_label(node, opts).is_none() {
        return false;
    }
    let neighbor_is_tab = |n: Option<&AstNode>| n.is_some_and(|n| tab_label(n, opts).is_some());
    neighbor_is_tab(node.previous_sibling()) || neighbor_is_tab(node.next_sibling())
}

/// グループ全体の開始タグ。`seq` はページ内で一意な連番
pub(crate) fn open_group(seq: usize) -> String {
    format!("<div class=\"tabs\" data-tabs=\"{seq}\">")
}

/// グループ全体の終了タグ（最後のパネルも閉じる）
pub(crate) const CLOSE_GROUP: &str = "</div></div>";

/// 1 タブぶんの開始マークアップ（ラジオ・ラベル・パネル開始）。
///
/// `index == 0` を `checked` にする。2 つ目以降は直前のパネルを閉じてから開く
/// （パネルは `<div class="tab-panel">` で、閉じは次のタブの開始か
/// [`CLOSE_GROUP`] が受け持つ）。
pub(crate) fn open_tab(seq: usize, index: usize, label: &str) -> String {
    let id = format!("yz-tab-{seq}-{index}");
    let name = format!("yz-tabs-{seq}");
    let checked = if index == 0 { " checked" } else { "" };
    // 2 つ目以降は直前のパネルを閉じる
    let close_prev = if index == 0 { "" } else { "</div>" };
    format!(
        "{close_prev}<input class=\"tab-radio\" type=\"radio\" name=\"{name}\" id=\"{id}\"{checked}>\
         <label class=\"tab-label\" for=\"{id}\">{}</label>\
         <div class=\"tab-panel\">",
        escape_html(label)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 先頭のタブだけが_checked_になる() {
        assert!(open_tab(0, 0, "Rust").contains(" checked>"));
        assert!(!open_tab(0, 1, "TypeScript").contains(" checked>"));
    }

    #[test]
    fn 二つ目以降は直前のパネルを閉じる() {
        assert!(!open_tab(0, 0, "a").starts_with("</div>"));
        assert!(open_tab(0, 1, "b").starts_with("</div>"));
    }

    #[test]
    fn ラジオ名はグループごとに変わる() {
        // 同じページに 2 グループ置いても切り替えが巻き添えにならない
        assert!(open_tab(0, 0, "a").contains("name=\"yz-tabs-0\""));
        assert!(open_tab(1, 0, "a").contains("name=\"yz-tabs-1\""));
    }

    #[test]
    fn 見出しはエスケープされる() {
        let html = open_tab(0, 0, "<script>&\"");
        assert!(html.contains("&lt;script&gt;&amp;&quot;"), "{html}");
        assert!(!html.contains("<script>"));
    }
}
