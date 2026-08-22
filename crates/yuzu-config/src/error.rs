use std::fmt;
use std::path::PathBuf;

/// 設定の探索・読み込み・解決で起きるエラー
#[derive(Debug)]
pub enum ConfigError {
    ProjectRootNotFound {
        start: PathBuf,
    },

    Io {
        path: PathBuf,
        source: std::io::Error,
    },

    /// TOML の構文エラー（最初の 1 件で停止。重複キー・未対応構文もここ）
    Syntax {
        path: PathBuf,
        line: usize,
        col: usize,
        message: String,
    },

    /// 型不一致・未知キー・不正値（位置付きで全件蓄積）
    Invalid {
        path: PathBuf,
        issues: Vec<ConfigIssue>,
    },

    /// `output.clean` の既定は true で、出力ディレクトリを丸ごと再帰削除する。
    /// 無検証だとルート外・ルート自身・原稿ディレクトリが消えるため load で弾く
    UnsafeOutputDir {
        key: &'static str,
        value: String,
        reason: &'static str,
        root: PathBuf,
    },
}

/// [`ConfigError::Invalid`] の 1 件
#[derive(Debug, Clone)]
pub struct ConfigIssue {
    /// キーのパス（`markdown.crossref.numbering` 形式）
    pub key_path: String,
    /// 1 始まりの行
    pub line: usize,
    /// 1 始まりの列（バイト基準。診断の列規約に合わせる）
    pub col: usize,
    pub message: String,
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ProjectRootNotFound { start } => write!(
                f,
                "{} が見つかりません（{} から上方向に探索）。`yuzu new` で作成するか、プロジェクトルートで実行してください",
                crate::CONFIG_FILE_NAME,
                start.display()
            ),
            Self::Io { path, source } => {
                write!(f, "{} を読み込めません: {source}", path.display())
            }
            Self::Syntax {
                path,
                line,
                col,
                message,
            } => write!(
                f,
                "{}:{line}:{col}: TOML の構文エラー: {message}",
                path.display()
            ),
            Self::Invalid { path, issues } => {
                write!(
                    f,
                    "{} の設定が不正です（{} 件）:",
                    crate::CONFIG_FILE_NAME,
                    issues.len()
                )?;
                for issue in issues {
                    write!(
                        f,
                        "\n  {}:{}:{}: {}",
                        path.display(),
                        issue.line,
                        issue.col,
                        issue.message
                    )?;
                }
                Ok(())
            }
            Self::UnsafeOutputDir {
                key,
                value,
                reason,
                root,
            } => write!(
                f,
                "{key} の値 `{value}` は使えません（{reason}）。プロジェクトルート（{}）配下の相対パスを指定してください — output.clean はこのディレクトリを丸ごと削除します",
                root.display()
            ),
        }
    }
}

impl std::error::Error for ConfigError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}
