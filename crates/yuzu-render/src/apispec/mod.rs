//! OpenAPI / JSON Schema のビルド時レンダリング（SSR。実装中のスタブ）。
//!
//! 方針:
//! - 入力（YAML / JSON テキスト）を serde_yaml_ng で `serde_json::Value` に読み、
//!   Value 走査で HTML を組み立てる（構造体に固く落とさず未知フィールドに耐える）
//! - **Err を返さない**: パース失敗・未対応形式は警告ログ＋エラーボックス HTML を
//!   返す（ビルドは止めない = draft 執筆に優しい）。呼び出し側は常に埋め込むだけ
//! - 出力は決定的（preserve_order で仕様の記述順を尊重）・全テキスト escape 済み

mod render;

/// レンダリング対象の種別
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SpecKind {
    /// OpenAPI 3.x 文書全体
    OpenApi,
    /// 単一の JSON Schema
    JsonSchema,
}

/// 仕様テキスト（YAML / JSON）を静的 HTML へ変換する。
/// 失敗時はエラーボックス HTML（`.markdown-alert-caution` 構造）を返す
pub(crate) fn render_spec(kind: SpecKind, source: &str) -> String {
    render::render(kind, source)
}

/// エラーボックス HTML（comrak alerts と同じ構造でテーマ CSS がそのまま当たる）。
/// `message` は見出し行、`source` はエスケープして `<pre>` で併記する
pub(crate) fn error_box(message: &str, source: &str) -> String {
    format!(
        "<div class=\"markdown-alert markdown-alert-caution\">\n\
         <p class=\"markdown-alert-title\">API 仕様のレンダリングに失敗しました</p>\n\
         <p>{}</p>\n\
         </div>\n\
         <pre><code>{}</code></pre>\n",
        crate::highlight::escape_html(message),
        crate::highlight::escape_html(source),
    )
}
