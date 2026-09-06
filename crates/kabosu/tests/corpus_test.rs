//! corpus のテーブル駆動テスト（tests/corpus/README.md 参照）。
//!
//! - valid/: 全件受理される
//! - invalid/: 全件エラーになる（書き間違い）

use std::fs;
use std::path::PathBuf;

use kabosu::Document;

fn corpus(dir: &str) -> Vec<(String, String)> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!("tests/corpus/{dir}"));
    let mut files: Vec<_> = fs::read_dir(root)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "toml"))
        .collect();
    files.sort();
    assert!(!files.is_empty(), "corpus/{dir} が空");
    files
        .into_iter()
        .map(|p| {
            (
                p.file_stem().unwrap().to_string_lossy().into_owned(),
                fs::read_to_string(p).unwrap(),
            )
        })
        .collect()
}

#[test]
fn valid_corpus_を全件受理する() {
    for (name, src) in corpus("valid") {
        Document::parse(&src).unwrap_or_else(|e| {
            let lc = kabosu::line_col_of(&src, e.span().start);
            panic!(
                "{name}:{}:{}: 受理できるはずの入力でエラー: {e}",
                lc.line, lc.col
            )
        });
    }
}

#[test]
fn invalid_corpus_は全件エラーになる() {
    for (name, src) in corpus("invalid") {
        let _ = Document::parse(&src).unwrap_err_or_panic(&name);
    }
}

/// Result 拡張（エラーメッセージにケース名を含める）
trait UnwrapErrOrPanic<T, E> {
    fn unwrap_err_or_panic(self, name: &str) -> E;
}

impl<T, E> UnwrapErrOrPanic<T, E> for Result<T, E> {
    fn unwrap_err_or_panic(self, name: &str) -> E {
        match self {
            Err(e) => e,
            Ok(_) => panic!("{name}: エラーになるはずの入力が受理された"),
        }
    }
}
