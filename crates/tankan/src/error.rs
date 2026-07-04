use crate::kind::DiagramKind;

/// 変換エラー。呼び出し側はこれを見てフォールバック（クライアント描画等）を判断する
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// 図種ごと未対応（想定内のフォールバック）
    #[error("図種 {kind:?} は未対応です")]
    UnsupportedDiagram { kind: DiagramKind },

    /// 対応図種内の未対応構文（想定内のフォールバック）
    #[error("{line} 行目: 未対応の構文 `{construct}`")]
    UnsupportedSyntax { line: usize, construct: String },

    /// 構文エラー。書き間違いの可能性が高いので、呼び出し側は警告を出すことを推奨
    #[error("{line} 行目: {message}")]
    Parse { line: usize, message: String },
}

impl Error {
    /// 「想定内の未対応（静かにフォールバックしてよい）」か、
    /// 「構文エラー（書き間違いの可能性。警告推奨）」かの振り分け
    pub fn is_unsupported(&self) -> bool {
        matches!(
            self,
            Self::UnsupportedDiagram { .. } | Self::UnsupportedSyntax { .. }
        )
    }
}
