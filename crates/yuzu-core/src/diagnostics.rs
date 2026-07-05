//! fmt / lint / check が返す診断の型

use std::path::PathBuf;

use crate::model::SourceSpan;

/// 診断の深刻度。Error は check の失敗（非ゼロ終了）に直結する
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
}

/// fmt / lint / check の 1 診断
#[derive(Debug, Clone)]
pub struct Diagnostic {
    /// ルール ID（ASCII。例: `broken-link` / `duplicate-h1`）
    pub rule: &'static str,
    pub severity: Severity,
    /// content_dir からの相対パス
    pub rel: PathBuf,
    /// ソース上の位置。ファイル単位の診断（fmt 差分等）は None
    pub span: Option<SourceSpan>,
    /// 説明（日本語）
    pub message: String,
}
