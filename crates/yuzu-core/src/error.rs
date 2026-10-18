use std::path::PathBuf;

/// サイトモデル構築・本文 HTML 化で起きるエラー
#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    #[error("{path} を読み込めません: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    /// frontmatter の書き間違い。最初のページで止めず、全ページ分を位置付きで集める
    /// （yuzu.toml の設定エラーと同じ形）
    #[error("{}", frontmatter_message(.problems))]
    Frontmatter { problems: Vec<FrontmatterProblem> },

    #[error("ignore パターン `{pattern}` が不正です: {message}")]
    InvalidIgnorePattern { pattern: String, message: String },

    #[error("HTML 出力に失敗しました: {0}")]
    Render(#[from] std::fmt::Error),
}

/// [`CoreError::Frontmatter`] の 1 件
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrontmatterProblem {
    pub path: PathBuf,
    /// 1 始まりの行（ファイル上の行）
    pub line: usize,
    /// 1 始まりの列（バイト基準。診断の列規約に合わせる）
    pub col: usize,
    pub message: String,
}

fn frontmatter_message(problems: &[FrontmatterProblem]) -> String {
    let mut out = format!("frontmatter（YAML）が不正です（{} 件）:", problems.len());
    for p in problems {
        out.push_str(&format!(
            "\n  {}:{}:{}: {}",
            p.path.display(),
            p.line,
            p.col,
            p.message
        ));
    }
    out
}
