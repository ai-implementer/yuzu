//! 図表番号と相互参照（キャプション行方式）。
//!
//! 図・表・コードの直前または直後に「キャプション行」（段落）を書くと、
//! ページ内で自動採番されたキャプションになり、本文から参照できる。
//!
//! - キャプション行: `Figure: 依存関係 {#fig:deps}`
//!   （`Table:` / `Listing:`、日本語の `図:` / `表:` / `リスト:` も受理）
//! - 参照: `[](#fig:deps)` のようにリンクテキストを空にすると「図 1」が入る
//!   （テキストがある `[この図](#fig:deps)` は著者の指定をそのまま使う）
//!
//! 記法はプレーンな Markdown ビューアで壊れない形（ただの段落とリンク）に
//! 寄せている。採番はページ内連番で、種別ごとに独立したカウンタを使う。

/// キャプションの種別（種別ごとに独立採番する）
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum CaptionKind {
    Figure,
    Table,
    Listing,
}

impl CaptionKind {
    /// 表示ラベル（「図 1」の「図」）
    pub fn label(self) -> &'static str {
        match self {
            CaptionKind::Figure => "図",
            CaptionKind::Table => "表",
            CaptionKind::Listing => "リスト",
        }
    }

    /// HTML のクラス接尾辞（`caption-fig` 等）
    pub(crate) fn class_suffix(self) -> &'static str {
        match self {
            CaptionKind::Figure => "fig",
            CaptionKind::Table => "tbl",
            CaptionKind::Listing => "lst",
        }
    }

    /// 行頭の接頭辞（英語・日本語の両方を受理する）
    fn from_prefix(text: &str) -> Option<(Self, &str)> {
        const PREFIXES: &[(&str, CaptionKind)] = &[
            ("Figure:", CaptionKind::Figure),
            ("図:", CaptionKind::Figure),
            ("図：", CaptionKind::Figure),
            ("Table:", CaptionKind::Table),
            ("表:", CaptionKind::Table),
            ("表：", CaptionKind::Table),
            ("Listing:", CaptionKind::Listing),
            ("リスト:", CaptionKind::Listing),
            ("リスト：", CaptionKind::Listing),
        ];
        PREFIXES
            .iter()
            .find_map(|&(prefix, kind)| text.strip_prefix(prefix).map(|rest| (kind, rest)))
    }
}

/// 解釈済みのキャプション行
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Caption {
    pub kind: CaptionKind,
    /// `{#fig:deps}` の中身（`fig:deps`）。省略時は None（採番はされるが参照できない）
    pub label: Option<String>,
    /// キャプション本文（`{#...}` を除いた部分）
    pub text: String,
}

/// 段落テキストをキャプション行として解釈する。
/// 形式は `Figure: 説明 {#fig:label}`（ラベルは省略可・日本語接頭辞も可）
pub(crate) fn parse_caption(text: &str) -> Option<Caption> {
    let trimmed = text.trim();
    let (kind, rest) = CaptionKind::from_prefix(trimmed)?;
    let rest = rest.trim();
    // 末尾の {#label}。入力は解釈済みテキスト（`yuzu fmt` が書く `{\#label}` の
    // エスケープはパース時に解決済み）なので、ここでは `{#` だけを見ればよい
    let (body, label) = match rest.strip_suffix('}').and_then(|r| r.rsplit_once("{#")) {
        Some((body, label)) if !label.trim().is_empty() && !label.contains(char::is_whitespace) => {
            (body.trim_end(), Some(label.trim().to_string()))
        }
        _ => (rest, None),
    };
    if body.is_empty() && label.is_none() {
        return None;
    }
    Some(Caption {
        kind,
        label,
        text: body.to_string(),
    })
}

/// キャプションの採番機（種別ごとに独立したカウンタ）。
/// サイト全体の通し番号（`markdown.crossref.numbering: "site"`）では、
/// 先行ページまでの個数を初期値（オフセット）として渡す
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Numbering {
    figure: usize,
    table: usize,
    listing: usize,
}

impl Numbering {
    /// 次の番号を採番する（文書順に呼ぶこと）
    pub(crate) fn next(&mut self, kind: CaptionKind) -> usize {
        let counter = match kind {
            CaptionKind::Figure => &mut self.figure,
            CaptionKind::Table => &mut self.table,
            CaptionKind::Listing => &mut self.listing,
        };
        *counter += 1;
        *counter
    }

    /// 種別ごとの採番済み個数を足し込む（サイト通し番号のオフセット計算用）
    pub(crate) fn add(&mut self, other: &Numbering) {
        self.figure += other.figure;
        self.table += other.table;
        self.listing += other.listing;
    }

    /// 種別の現在値（= 採番済み個数）
    pub(crate) fn get(&self, kind: CaptionKind) -> usize {
        match kind {
            CaptionKind::Figure => self.figure,
            CaptionKind::Table => self.table,
            CaptionKind::Listing => self.listing,
        }
    }
}

/// キャプション行の HTML（採番済み）。ラベルがあればアンカー id を付ける
pub(crate) fn render_caption(caption: &Caption, numbering: &mut Numbering) -> String {
    let number = numbering.next(caption.kind);
    let id = match &caption.label {
        Some(label) => format!(" id=\"{}\"", escape_html(label)),
        None => String::new(),
    };
    let text = escape_html(&caption.text);
    let separator = if caption.text.is_empty() { "" } else { ": " };
    format!(
        "<p class=\"caption caption-{}\"{id}><span class=\"caption-label\">{} {number}</span>{separator}{text}</p>\n",
        caption.kind.class_suffix(),
        caption.kind.label(),
    )
}

/// HTML エスケープ（テキストノード・属性値用の最小集合）
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
    fn キャプション_html_は採番とアンカーを持つ() {
        let mut n = Numbering::default();
        let html = render_caption(
            &parse_caption("Figure: 依存関係 {#fig:deps}").unwrap(),
            &mut n,
        );
        assert_eq!(
            html,
            "<p class=\"caption caption-fig\" id=\"fig:deps\"><span class=\"caption-label\">図 1</span>: 依存関係</p>\n"
        );
        // ラベルなしは id なしで採番だけ進む
        let html = render_caption(&parse_caption("Figure: 次の図").unwrap(), &mut n);
        assert!(html.contains("図 2"), "{html}");
        assert!(!html.contains("id="), "{html}");
    }

    #[test]
    fn キャプションはエスケープされる() {
        let mut n = Numbering::default();
        let html = render_caption(
            &parse_caption("Table: <script> & \"引用\" {#tbl:x}").unwrap(),
            &mut n,
        );
        assert!(
            html.contains("&lt;script&gt; &amp; &quot;引用&quot;"),
            "{html}"
        );
        assert!(html.contains("class=\"caption caption-tbl\""));
        assert!(html.contains("表 1"));
    }

    #[test]
    fn 英語と日本語の接頭辞を受理する() {
        for (src, kind) in [
            ("Figure: 図の説明 {#fig:a}", CaptionKind::Figure),
            ("図: 図の説明 {#fig:a}", CaptionKind::Figure),
            ("図： 図の説明 {#fig:a}", CaptionKind::Figure),
            ("Table: 表の説明 {#tbl:a}", CaptionKind::Table),
            ("表: 表の説明 {#tbl:a}", CaptionKind::Table),
            ("Listing: コード {#lst:a}", CaptionKind::Listing),
            ("リスト: コード {#lst:a}", CaptionKind::Listing),
        ] {
            let cap = parse_caption(src).unwrap_or_else(|| panic!("解釈できる: {src}"));
            assert_eq!(cap.kind, kind, "{src}");
        }
    }

    #[test]
    fn ラベルとテキストを分離する() {
        let cap = parse_caption("Figure: 依存関係の図 {#fig:deps}").unwrap();
        assert_eq!(cap.text, "依存関係の図");
        assert_eq!(cap.label.as_deref(), Some("fig:deps"));

        // ラベル省略（採番はされるが参照はできない）
        let cap = parse_caption("Figure: ラベルなし").unwrap();
        assert_eq!(cap.text, "ラベルなし");
        assert_eq!(cap.label, None);
    }

    #[test]
    fn キャプションでない段落は_none() {
        assert!(parse_caption("ふつうの段落です").is_none());
        assert!(parse_caption("図表は大事").is_none(), "コロンなしは対象外");
        assert!(parse_caption("Figure:").is_none(), "空は対象外");
        // 空白を含むラベルはラベルとして扱わない（本文の一部）
        let cap = parse_caption("Figure: 説明 {#fig a}").unwrap();
        assert_eq!(cap.label, None);
        assert_eq!(cap.text, "説明 {#fig a}");
    }

    #[test]
    fn 採番は種別ごとに独立する() {
        let mut n = Numbering::default();
        assert_eq!(n.next(CaptionKind::Figure), 1);
        assert_eq!(n.next(CaptionKind::Table), 1);
        assert_eq!(n.next(CaptionKind::Figure), 2);
        assert_eq!(n.next(CaptionKind::Listing), 1);
        assert_eq!(n.next(CaptionKind::Table), 2);
    }
}
