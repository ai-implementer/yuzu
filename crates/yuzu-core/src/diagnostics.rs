//! fmt / lint / check が返す診断の型

use std::path::PathBuf;

use crate::model::SourceSpan;
use crate::rules::Rule;

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

/// fmt / lint / check の 1 診断。
///
/// ルールの定義（[`Rule`]）をそのまま持ち、深刻度はそこから決める（[`Diagnostic::severity`]）。
/// ID と深刻度を別々に持つと、組み立てる側が `rules::X.id` と `rules::X.severity` を
/// 対にして手で書くことになり、片方だけ別のルールを書いても通ってしまう（10-12 レビューの R2）
#[derive(Debug, Clone)]
pub struct Diagnostic {
    /// ルールの定義（ID・深刻度・抑制可否）
    pub rule: Rule,
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

impl Diagnostic {
    /// content 相対のページを指す、自動修正の無い診断
    pub fn new(
        rule: Rule,
        rel: impl Into<PathBuf>,
        span: Option<SourceSpan>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            rule,
            rel: rel.into(),
            base: DiagBase::Content,
            span,
            message: message.into(),
            fix: None,
        }
    }

    /// 自動修正の置換文字列を付ける
    pub fn with_fix(mut self, fix: impl Into<String>) -> Self {
        self.fix = Some(fix.into());
        self
    }

    /// `rel` の基点を変える（`yuzu.toml` などプロジェクトルート相対の診断）
    pub fn with_base(mut self, base: DiagBase) -> Self {
        self.base = base;
        self
    }

    /// 深刻度（ルールの定義から決まる）
    pub fn severity(&self) -> Severity {
        self.rule.severity
    }
}
