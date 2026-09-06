---
title: kabosu の設計
order: 3
description: 依存ゼロの TOML 設定ライブラリと yuzu-config の責務分離
---

# kabosu の設計

> 状態: 確定設計（2026-08-16）。v0.1 の実装と yuzu-config への統合は完了済み
> （2026-08-22。`crates/kabosu/`・`crates/yuzu-config/src/codec.rs`）。以下は設計の原文

`kabosu` は、TOML の解析・型変換・検証・生成を担う汎用ライブラリです。
yuzu 固有の設定スキーマやファイル操作を含めず、crates.io で単独公開します。

## 境界と依存保証

- workspace 内の `crates/kabosu` で開発し、独立した `0.1.0` から公開する
- edition 2024、MSRV 1.85、`MIT OR Apache-2.0`
- `#![forbid(unsafe_code)]`
- 常時 `#![no_std] + alloc`。v0.1 では feature を設けない
- Sans I/O とし、ファイル探索・読み書き・環境変数処理を持たない
- `[dependencies]`、build 依存、optional 依存、ターゲット固有依存を
  完全にゼロにする。dev 依存だけはテストと fuzzing に許可する
- パッケージ化後の manifest を CI で検査し、依存ゼロを継続的に保証する

`yuzu-config` は非公開の薄いラッパーとして残し、通常依存を `kabosu` だけに
します。`yuzu.toml` の探索、yuzu 固有スキーマ、デフォルト、パス解決は
`yuzu-config` が担当します。

## TOML の対応範囲

対応する構文:

- UTF-8、LF / CRLF、`#` コメント
- bare key、quoted key、dotted key
- 標準テーブルとネストテーブル
- basic string と literal string（単行・複数行。エスケープは TOML 1.0 の全種）
- 符号付き 10 進整数と 16 / 8 / 2 進整数、`_` 区切り、TOML の i64 範囲
- float（小数・指数・`inf` / `nan`）
- boolean
- date-time（offset date-time、local date-time、local date、local time）
- 複数行、末尾カンマ、コメント、ネストを含む配列
- インラインテーブル（`{ ... }`）とテーブルの配列（`[[a]]`）
- 重複キーとテーブル競合の検出
- キー、値、コメントの正確な span

v0.1 は設定ファイル用途のサブセットで、float、date/time、16 / 8 / 2 進整数、
複数行文字列、inline table、array of tables を扱いませんでした。0.2 で
float、16 / 8 / 2 進整数、複数行文字列（v0.16 Phase 68）、date/time（Phase 69）、
inline table と array of tables（Phase 70）に対応し、**TOML 1.0 の全構文が揃いました**。

インラインテーブルは TOML 1.0 どおり 1 行に収める必要があり（改行・コメント・
末尾カンマは不可）、作られたテーブルは閉じています。`x = { a = 1 }` と書いた後に
`[x.b]` や `x.c = 2` で足すことはできません。`[[a]]` は配列の末尾に要素を追加し、
続く `[a.b]` や `[[a.b]]` は直前の要素の中に入ります。静的配列（`a = [1]`・
`a = [{ b = 1 }]`）を `[[a]]` で拡張することはできません。

正規化出力では、テーブル直下にある「要素が全部テーブルの配列」を `[[a]]` へ
展開します。インラインテーブルで出力するのは、ヘッダー形式では書けない位置
（スカラーと混在した配列、配列の中の配列）だけです。

参照実装（dev 依存の `toml`）は TOML 1.1 の構文も受理するため、kabosu が
TOML 1.0 どおり拒否するもの（秒を省略した `07:32`、インラインテーブルの改行と
末尾カンマ）は差分テストの `invalid/` コーパスに置けません。これらは crate 内の
ユニットテストで縛っています。

date-time は依存ゼロを保つため独自型（`Datetime` / `Date` / `Time` / `Offset`）で
表します。時刻演算・タイムゾーン変換・他の日時 crate への変換は持ちません。
`Datetime` は参照実装と同じく date / time / offset の組み合わせ 1 型で 4 種を表し、
区別は `Datetime::kind` で見ます。フィールドは非公開でコンストラクタが範囲を
検証するため、暦として存在しない日付を組み立てて不正な TOML を出力することは
できません。小数秒は 9 桁まで保持し、10 桁目以降は切り捨てます。オフセットは
分単位の数値だけを持つので、`Z` と `+00:00` は同じ値になり、正規化ではどちらも
`Z` で出力されます。

0.x で対応範囲を段階的に広げ、TOML 1.0 の公式 toml-test の valid、invalid、
encoder テストをすべて通過することを `1.0.0` の条件にします。

## データモデル

- `Document` は原文を所有する
- `Document::parse(&str)` は原文をコピーし、`parse_owned(String)` は所有権を受け取る
- `Table` は入力順を保持する。検索用の内部索引は公開 API にしない
- `Value` は `#[non_exhaustive]` とする
- `Document`、`Table`、`Entry`、`Node` の内部フィールドは非公開とし、
  v0.1 では読み取り用アクセサーだけを公開する
- コメントは原文と span を参照できる形で保持するが、正規化出力には含めない
- `Span` は 0 始まり、終端を含まない UTF-8 バイト範囲とする
- 表示用の line / column は 1 始まり、Unicode スカラー値単位とする
- キー経路は文字列へ平坦化せず、各セグメントと span を持つ `KeyPath` とする

非破壊編集 API は v0.1 の対象外です。内部表現を直接変更できないようにし、将来、
コメントの所属規則と一緒に設計します。

## 型変換と診断

derive macro は作らず、手書きの `Decode` / `TableDecoder` と
`Encode` / `TableEncoder` を提供します。必須、任意、既定値、ネスト、検証、
未知キー検出をサポートし、文字列、整数、boolean、`Option`、`Vec`、
`BTreeMap` に標準実装を用意します。

通常利用向けに次の高レベル API も提供します。

```rust
from_str::<T>(&str) -> Result<DecodeReport<T>, ParseError>
from_str_with_options::<T>(&str, DecodeOptions)
    -> Result<DecodeReport<T>, ParseError>
to_string<T>(&T) -> Result<String, EncodeError>
```

- 構文エラーは最初の `ParseError` で停止する
- 型変換は可能な限り複数の診断を蓄積する
- エラーが 1 件でもあれば値を返さず、警告だけなら値を返す
- エンコードは最初の `EncodeError` で停止し、部分出力しない
- 利用者は decode 実装から独自のコード、メッセージ、重大度、キー経路を持つ
  診断を追加できる
- 診断は主 span の開始位置で安定ソートする。対象が存在しない必須キー欠落は
  所属テーブル末尾の長さ 0 の span に置く
- 型変換の診断は既定 100 件までとし、超過分を省略したことを最後に示す
- 解析深度は既定 128 とし、配列、テーブル、dotted key の超過を位置付きで返す
- 組み込みのエラー文は英語にし、利用側で翻訳できる構造化された種別、コード、
  `KeyPath`、span を返す。ライブラリ自身はログを出力しない

未知キーは `Warn`（既定）、`Deny`、`Ignore` の 3 方針を提供します。

## 正規化出力

同じ値から常に同じバイト列を生成します。

- 空配列は `[]`、スカラーだけの配列は 1 行で出力する
- 配列を含むネスト配列は、スペース 2 個のインデントと末尾カンマを使う
- 行幅による自動折り返しは行わない
- 改行は LF とし、末尾にも改行を付ける
- 文字列は常に basic string とし、引用符、バックスラッシュ、制御文字だけを
  エスケープする。表示可能な Unicode はそのまま出力する
- `[A-Za-z0-9_-]+` に一致するキーは bare key、それ以外は basic string で引用する
- 親テーブルの値を子テーブルより先に出力し、各グループでは encoder への追加順を
  維持する
- parsed document のコメントは正規化出力へ引き継がない

## yuzu への統合

- 設定ファイルは `yuzu.toml` だけをサポートする
- JSONC の互換読み込み、フォールバック、変換コマンドは作らない
- キーは `snake_case` に変更するが、現行設定の階層と意味は維持する
- `yuzu new` は主要な既定値と任意項目の説明を含む、注釈付き `yuzu.toml` を生成する
- yuzu は未知キーを `Deny` とし、設定エラーとして停止する
- `.yuzu/settings.json` は代替ファイルを作らず廃止する
- `jsonc-parser`、`serde`、`serde_json`、`thiserror`、`tracing` を
  `yuzu-config` の依存から外す

## v0.1 の非スコープ

- ファイル I/O と設定ファイル探索
- 複数設定の merge
- derive macro
- 非破壊編集
- compact 出力と行幅ベースの整形
- JSON / JSONC の互換機能

## 検証ゲート

- 単体テスト、位置・エラー種別テスト
- value → TOML → value の round-trip
- 正規化出力の snapshot
- dev 依存の `toml` crate との対応範囲内の差分テスト
- 公式 toml-test と、v0.1 で未対応にするケースの明示的な一覧
- panic、hang、不正な UTF-8 境界を検出する fuzzing
- Rust 1.85 と現在の stable の CI
- `no_std + alloc` ビルド
- `cargo package` 後の依存ゼロ検査
