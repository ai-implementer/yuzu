//! kabosu — 依存ゼロの TOML 解析・型変換・検証・生成ライブラリ。
//!
//! - **依存ゼロ・純 Rust・`no_std + alloc`・Sans I/O**（ファイル探索・読み書き・
//!   環境変数処理を持たない）
//! - **正確な span**: キー・値・コメントすべてが原文のバイト範囲を持ち、
//!   診断を位置付きで返せる
//! - **手書き decode / encode**（derive マクロなし）: 必須・任意・既定値・ネスト・
//!   検証・未知キー検出（Warn / Deny / Ignore）をサポート
//! - **正規化出力**: 同じ値から常に同じバイト列を生成する
//! - 対応範囲は TOML 1.0 のサブセット（文字列は複数行含む全種・整数は 10 進と
//!   16,8,2 進・float・boolean・配列・テーブル）。まだ未対応の構文（date-time /
//!   inline table / array of tables）は一般的な構文エラーにせず、位置付きの
//!   [`ParseErrorKind::Unsupported`] として返す
//!
//! ```
//! let doc = kabosu::Document::parse("title = \"yuzu\"\n[dev]\nport = 5173\n").unwrap();
//! let dev = doc.root().get("dev").unwrap().node().as_table().unwrap();
//! assert_eq!(dev.get("port").unwrap().node().as_integer(), Some(5173));
//! ```

#![no_std]
#![forbid(unsafe_code)]
#![deny(
    clippy::std_instead_of_core,
    clippy::std_instead_of_alloc,
    clippy::alloc_instead_of_core
)]

extern crate alloc;

mod decode;
mod encode;
mod error;
mod lexer;
mod model;
mod normalize;
mod parser;

pub use decode::{
    Decode, DecodeContext, DecodeOptions, DecodeReport, Diagnostic, DiagnosticCode, Severity,
    TableDecoder, UnknownKeys,
};
pub use encode::{ArrayEncoder, Encode, EncodeError, EncodeErrorKind, Encoder, TableEncoder};
pub use error::{ParseError, ParseErrorKind, UnsupportedFeature};
pub use model::{
    Comment, Document, Entry, KeyPath, KeySegment, LineCol, Node, Span, Table, Value, ValueKind,
    line_col_of,
};

/// パースして既定オプションで `T` へ decode する。
/// 構文エラーは最初の 1 件で `Err`、型変換の診断は `DecodeReport` に蓄積される
pub fn from_str<T: Decode>(src: &str) -> Result<DecodeReport<T>, ParseError> {
    from_str_with_options(src, DecodeOptions::default())
}

/// オプション付きの [`from_str`]
pub fn from_str_with_options<T: Decode>(
    src: &str,
    options: DecodeOptions,
) -> Result<DecodeReport<T>, ParseError> {
    let doc = Document::parse(src)?;
    Ok(decode(&doc, options))
}

/// パース済みの [`Document`] を `T` へ decode する
/// （span → 行列変換などで `Document` を手元に残したい利用者向け）
pub fn decode<T: Decode>(doc: &Document, options: DecodeOptions) -> DecodeReport<T> {
    decode::run(doc, options)
}

/// 値を正規形の TOML 文字列へ変換する（ルートはテーブルであること）。
/// 同じ値からは常に同じバイト列が出る
pub fn to_string<T: Encode + ?Sized>(value: &T) -> Result<alloc::string::String, EncodeError> {
    encode::to_string_impl(value)
}
