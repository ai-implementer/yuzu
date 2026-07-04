//! tankan — Mermaid 互換のダイアグラムテキストを SVG に変換する純 Rust ライブラリ。
//!
//! - I/O なし・時刻/乱数非依存（wasm 対応）。特定ツールに依存しない汎用設計
//! - 未対応の図種・構文は [`Error`] で明示し、呼び出し側がクライアント描画等へ
//!   フォールバックできる（[`Error::is_unsupported`]）
//!
//! ```
//! let svg = tankan::render_svg(
//!     "sequenceDiagram\n    A->>B: こんにちは\n",
//!     &tankan::Options::default(),
//! ).unwrap();
//! assert!(svg.starts_with("<svg"));
//! ```

mod common;
mod error;
mod kind;
mod options;
mod sequence;

pub use error::Error;
pub use kind::DiagramKind;
pub use options::{Options, Theme};

/// ソース先頭のキーワードから図種を判別する
/// （`%%` コメント・`%%{init}%%` ディレクティブ・YAML frontmatter はスキップ）
pub fn detect(source: &str) -> DiagramKind {
    kind::detect(source)
}

/// ダイアグラムテキストを SVG 文字列に変換する。
/// 未対応図種は [`Error::UnsupportedDiagram`]、対応図種内の未対応構文は
/// [`Error::UnsupportedSyntax`]、書式誤りは [`Error::Parse`] を返す
pub fn render_svg(source: &str, options: &Options) -> Result<String, Error> {
    match detect(source) {
        DiagramKind::Sequence => sequence::render(source, options),
        kind => Err(Error::UnsupportedDiagram { kind }),
    }
}
