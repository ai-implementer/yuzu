//! vaporetto による日本語分かち書き＋正規化。
//!
//! ⚠️ index 時と query 時の整合の要。このパイプラインの**順序を入れ替えないこと**:
//! 1. `KyteaFullwidthFilter` で半角→全角（BCCWJ モデルは全角文字で学習されているため、
//!    NFKC を先にかけるとモデル入力が半角化して分割精度が落ちる）
//! 2. vaporetto で分割
//! 3. token ごとに NFKC 正規化（全角英数→半角に戻る）→ 小文字化
//! 4. 英数・文字を 1 つも含まない token（記号のみ）を除外

use unicode_normalization::UnicodeNormalization;
use vaporetto::{Model, Predictor, Sentence};
use vaporetto_rules::StringFilter;
use vaporetto_rules::string_filters::KyteaFullwidthFilter;

use crate::error::FormatError;

pub struct Tokenizer {
    predictor: Predictor,
    fullwidth: KyteaFullwidthFilter,
}

impl Tokenizer {
    /// zstd 圧縮モデル（`.model.zst` のバイト列）から構築する。
    /// 伸長は純 Rust の ruzstd（wasm でも同じコードが動く）
    pub fn from_zstd_model_bytes(bytes: &[u8]) -> Result<Self, FormatError> {
        let mut reader = bytes;
        let decoder = ruzstd::decoding::StreamingDecoder::new(&mut reader)
            .map_err(|e| FormatError::Model(format!("zstd 伸長に失敗: {e}")))?;
        let model = Model::read(decoder)
            .map_err(|e| FormatError::Model(format!("モデルのパースに失敗: {e}")))?;
        // 第 2 引数 = タグ予測（品詞等）。検索には不要なので無効
        let predictor = Predictor::new(model, false)
            .map_err(|e| FormatError::Model(format!("Predictor の構築に失敗: {e}")))?;
        Ok(Self {
            predictor,
            fullwidth: KyteaFullwidthFilter,
        })
    }

    /// 正規化込みの分かち書き。index 側・query 側の両方が**これだけ**を使う
    pub fn tokenize(&self, text: &str) -> Vec<String> {
        let mut tokens = Vec::new();
        // vaporetto の Sentence は改行を含められないため行単位で処理
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let fullwidth = self.fullwidth.filter(line);
            let Ok(mut sentence) = Sentence::from_raw(fullwidth) else {
                continue;
            };
            self.predictor.predict(&mut sentence);
            for token in sentence.iter_tokens() {
                let normalized: String = token.surface().nfkc().collect::<String>().to_lowercase();
                let normalized = normalized.trim().to_string();
                // 記号・空白のみの token は索引に入れない
                if normalized.chars().any(|c| c.is_alphanumeric()) {
                    tokens.push(normalized);
                }
            }
        }
        tokens
    }
}

#[cfg(test)]
mod tests {
    use super::Tokenizer;

    fn tokenizer() -> Tokenizer {
        // テストは同梱モデルを直接読む（feature 不要）
        let bytes = include_bytes!("../assets/model/bccwj-suw_c1.0.model.zst");
        Tokenizer::from_zstd_model_bytes(bytes).unwrap()
    }

    /// 分割点はモデル依存なので、結合結果（＝分割不変量）と正規化を検証する
    #[test]
    fn 分割結果の結合は正規化済みテキストと一致する() {
        let t = tokenizer();
        let tokens = t.tokenize("日本語の全文検索");
        assert!(!tokens.is_empty());
        assert_eq!(tokens.concat(), "日本語の全文検索");
    }

    #[test]
    fn 全角英数は半角小文字に正規化される() {
        let t = tokenizer();
        let joined = t.tokenize("ＡＰＩサーバーの構築").concat();
        assert!(joined.contains("api"), "joined={joined}");
        assert!(joined.contains("サーバー"));
        assert!(!joined.contains("ＡＰＩ"));
    }

    #[test]
    fn 半角英字は小文字化される() {
        let t = tokenizer();
        let joined = t.tokenize("Yuzu Build Watch").concat();
        assert!(joined.contains("yuzu"), "joined={joined}");
        assert!(joined.contains("build"));
    }

    #[test]
    fn 記号のみの_token_は除外される() {
        let t = tokenizer();
        let tokens = t.tokenize("こんにちは、世界！！ --- ");
        assert!(
            tokens
                .iter()
                .all(|t| t.chars().any(|c| c.is_alphanumeric())),
            "tokens={tokens:?}"
        );
        assert!(tokens.concat().contains("世界"));
    }

    #[test]
    fn 複数行は行ごとに処理される() {
        let t = tokenizer();
        let tokens = t.tokenize("一行目の文\n\n二行目の文\n");
        let joined = tokens.concat();
        assert!(joined.contains("一行目"));
        assert!(joined.contains("二行目"));
    }

    /// モデル・vaporetto 更新で分割が変わったら差分として現れるゴールデンテスト
    #[test]
    fn 分かち書きのスナップショット() {
        let t = tokenizer();
        insta::assert_debug_snapshot!(
            "tokenize_golden",
            t.tokenize("Markdownで書いた設計書を静的HTMLサイトに変換する")
        );
    }
}
