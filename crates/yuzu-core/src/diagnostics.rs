//! fmt / lint / check が返す診断の型

use std::path::PathBuf;

use crate::model::SourceSpan;

/// 診断の深刻度。Error は check の失敗（非ゼロ終了）に直結する
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
}

/// [`Diagnostic::rel`] の基点。ほとんどの診断は content 配下のページを指すが、
/// `yuzu.toml` のように content の外にあるファイルも報告できるようにする
/// （`rel` に `../yuzu.toml` を入れると出力パスに `..` が混じり、
/// GitHub の注釈がリポジトリのファイルへ紐づかなくなる）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DiagBase {
    /// content ディレクトリ相対（既定）
    #[default]
    Content,
    /// プロジェクトルート相対
    ProjectRoot,
}

/// fmt / lint / check の 1 診断
#[derive(Debug, Clone)]
pub struct Diagnostic {
    /// ルール ID（ASCII。例: `broken-link` / `duplicate-h1`）
    pub rule: &'static str,
    pub severity: Severity,
    /// [`Diagnostic::base`] からの相対パス
    pub rel: PathBuf,
    /// `rel` の基点
    pub base: DiagBase,
    /// ソース上の位置。ファイル単位の診断（fmt 差分等）は None
    pub span: Option<SourceSpan>,
    /// 説明（日本語）
    pub message: String,
    /// 自動修正の置換文字列（`yuzu lint --fix` が `span` の範囲をこれで置き換える）。
    /// None は機械修正できない診断（見出し規約・未知キー等）
    pub fix: Option<String>,
}
