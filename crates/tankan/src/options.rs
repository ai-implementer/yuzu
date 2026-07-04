//! レンダリングオプションとテーマ

/// レンダリングオプション
#[derive(Debug, Clone)]
pub struct Options {
    pub theme: Theme,
    /// SVG 内 id（`<marker>` 等）の接頭辞。id は**文書グローバル**なので、
    /// 同一 HTML ページへ複数の SVG をインライン展開する場合は
    /// SVG ごとに一意にすること（例: "tk0", "tk1", …）
    pub id_prefix: String,
    /// CSS の font-family 値としてそのまま出力される
    pub font_family: String,
    /// 基準フォントサイズ（px）
    pub font_size: f32,
    /// width/height 属性への倍率（viewBox は不変）
    pub scale: f32,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            theme: Theme::default(),
            id_prefix: "tk".to_string(),
            font_family: "sans-serif".to_string(),
            font_size: 14.0,
            scale: 1.0,
        }
    }
}

/// 配色。すべて **CSS の `<color>` 値として `<style>` に埋め込まれる文字列**。
/// `"var(--fg, #1f2328)"` のような CSS 変数参照も可（HTML インライン SVG 前提。
/// ページ側の変数がカスケードで届き、ダークモード等に追従する）
#[derive(Debug, Clone)]
pub struct Theme {
    /// テキスト・メッセージ線・矢印
    pub foreground: String,
    /// ライフライン等の補助線
    pub muted: String,
    /// 参加者ボックスの塗り
    pub background: String,
    /// Note・activation バーの塗り
    pub surface: String,
    /// 枠線
    pub border: String,
    /// autonumber バッジ・ブロックラベル
    pub accent: String,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            foreground: "#1f2328".to_string(),
            muted: "#59636e".to_string(),
            background: "#ffffff".to_string(),
            surface: "#f6f8fa".to_string(),
            border: "#d1d9e0".to_string(),
            accent: "#9a6700".to_string(),
        }
    }
}
