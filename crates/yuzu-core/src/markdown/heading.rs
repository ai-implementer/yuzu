//! 見出しの描画（comrak の `HeadingAdapter`）。
//!
//! comrak の `header_ids` の既定出力は見出しの先頭に
//! `<a href="#id" aria-hidden="true" class="anchor" id="id"></a>` を置く形で固定されている。
//! `aria-hidden` に加えてテーマの CSS が `visibility: hidden` で隠していたため、
//! パーマリンクはキーボードからも支援技術からも到達できなかった。
//!
//! ここでは見出し自身に id を付け、パーマリンクは見出しの**末尾**に `aria-label` 付きで
//! 置く。飛んだ先が見出しそのものになり、読み上げも「見出し文 → リンク名」の順になる。
//!
//! ⚠️ アンカー採番の同期: id は comrak の既定と同じ入力（`HeadingMeta::content` =
//! comrak の `collect_text`）を、同じ `Anchorizer` に文書順で通して得る。
//! extract_meta / extract_plain_sections の採番と一致し、既存の `#id` リンクも変わらない

use std::fmt;
use std::sync::{Mutex, PoisonError};

use comrak::Anchorizer;
use comrak::adapters::{HeadingAdapter, HeadingMeta};
use comrak::nodes::Sourcepos;

use super::escape_html;

/// 1 ページぶんの見出し描画の状態。ページごとに新しく作る（採番の重複回避は
/// ページ内で閉じる）
pub(crate) struct PermalinkHeadings {
    state: Mutex<State>,
}

struct State {
    anchorizer: Anchorizer,
    /// 開いている見出しの id（`exit` でリンクに使う。見出しは入れ子にならないが、
    /// `enter` と `exit` の対応を崩さないためスタックにする）
    open: Vec<String>,
}

impl PermalinkHeadings {
    pub(crate) fn new() -> Self {
        Self {
            state: Mutex::new(State {
                anchorizer: Anchorizer::new(),
                open: Vec::new(),
            }),
        }
    }
}

impl HeadingAdapter for PermalinkHeadings {
    fn enter(
        &self,
        output: &mut dyn fmt::Write,
        heading: &HeadingMeta,
        _sourcepos: Option<Sourcepos>,
    ) -> fmt::Result {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        let id = state.anchorizer.anchorize(&heading.content);
        write!(output, "<h{} id=\"{}\">", heading.level, escape_html(&id))?;
        state.open.push(id);
        Ok(())
    }

    fn exit(&self, output: &mut dyn fmt::Write, heading: &HeadingMeta) -> fmt::Result {
        let id = {
            let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
            state.open.pop().unwrap_or_default()
        };
        // 改行は comrak の既定（`</hN>` の後に lf）に合わせる
        writeln!(
            output,
            "<a class=\"anchor\" href=\"#{}\" aria-label=\"「{}」へのリンク\"></a></h{}>",
            escape_html(&id),
            escape_html(&heading.content),
            heading.level
        )
    }
}
