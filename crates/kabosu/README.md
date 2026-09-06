# kabosu 🍋

TOML の解析・型変換・検証・生成を担う**依存ゼロ**の Rust ライブラリ。
名前は柑橘のカボス（香母酢）から。

- **依存ゼロ・純 Rust・`no_std + alloc`・Sans I/O** — ファイル探索・読み書き・
  環境変数処理を持たない。`#![forbid(unsafe_code)]`
- **正確な span** — キー・値・コメントすべてが原文のバイト範囲を持ち、
  位置付きの診断を組み立てられる（設定ファイルのエラー報告向け）
- **手書き decode / encode** — derive マクロなし。必須・任意・既定値・ネスト・
  検証・未知キー検出（Warn / Deny / Ignore）を `TableDecoder` で書く
- **正規化出力** — 同じ値から常に同じバイト列を生成する（ハッシュキーにも使える）
- **特定ツール非依存の汎用設計**（開発は [yuzu](https://github.com/ai-implementer/yuzu)
  monorepo で行っているが `yuzu-*` に依存しない独立ライブラリ）

## インストール

```bash
cargo add kabosu
```

## 互換性の定義

1. **構文互換**: 対応範囲について TOML 1.0 の入力をエラーなく受理し、
   値解釈は参照実装（`toml` crate）と一致する（差分テストで担保）
2. **未対応の区別**: TOML として正しいがまだ扱わない構文は、一般的な
   構文エラーにせず位置付きの `ParseErrorKind::Unsupported` として返す
3. **決定的出力**: `to_string` は同じ値から常に同じバイト列を生成し、
   その出力は必ず再パースできる（round-trip / fuzzing で担保）

`1.0.0` の条件は公式 [toml-test](https://github.com/toml-lang/toml-test) の
valid / invalid / encoder テストの完全通過。

## 対応状況

| 構文 | 状態 |
|---|---|
| UTF-8・LF / CRLF・`#` コメント | ✅ |
| bare key・quoted key・dotted key | ✅ |
| 標準テーブル・ネストテーブル | ✅（重複・競合は TOML 1.0 の規則で検出） |
| basic string・literal string（単行・複数行） | ✅（`\uXXXX` / `\UXXXXXXXX`・行末 `\`・閉じ直前の引用符 2 個まで） |
| 整数（10 進・16 / 8 / 2 進・`_` 区切り・i64 範囲） | ✅（進数表記は保持せず i64 に畳む） |
| float（小数・指数・`inf` / `nan`） | ✅（`Decode for f64` は整数リテラルを受けない。`nan` の符号は落とす） |
| boolean | ✅ |
| 配列（複数行・末尾カンマ・コメント・ネスト・型混在） | ✅ |
| date-time（offset / local × date / time の 4 種） | ✅（`T` / `t` / 空白区切り・小数秒は 9 桁まで・オフセット 0 は `Z` へ正規化） |
| inline table / array of tables | `Unsupported`（位置付き。0.2 の残り） |

## 使い方

```rust
use kabosu::{Decode, DecodeContext, Node, TableDecoder};

struct Config {
    title: String,
    port: u16,
}

impl Decode for Config {
    fn decode(node: &Node, cx: &mut DecodeContext<'_>) -> Option<Self> {
        let mut d = TableDecoder::new(node, cx)?;
        let title = d.required("title");
        let port = d.optional("port");
        d.finish(); // 未消費キーへ未知キー方針（Warn/Deny/Ignore）を適用
        Some(Config {
            title: title.unwrap_or_default(),
            port: port.unwrap_or(5173),
        })
    }
}

let report = kabosu::from_str::<Config>("title = \"docs\"\n").unwrap();
assert!(!report.has_errors());
assert_eq!(report.value().unwrap().port, 5173);
```

- 構文エラーは最初の 1 件（`ParseError`）で停止、型変換の診断は
  `DecodeReport` に**蓄積**される（エラーが 1 件でもあれば値を返さない）
- 診断は種別（`DiagnosticCode`）・重大度・キー経路（`KeyPath`）・`Span` を
  構造化して返す。組み込みのメッセージは英語で、利用側で翻訳できる
- `Span` はバイト範囲。表示用の行列は `Document::line_col`（1 始まり・
  Unicode スカラー値単位）で算出する

## fuzzing

`fuzz/` に cargo-fuzz のハーネス（parse / roundtrip / decode）がある。
nightly が必要:

```bash
cd crates/kabosu
cargo +nightly fuzz run parse -- -max_total_time=60
```

## ライセンス

MIT または Apache-2.0 のデュアルライセンス。
