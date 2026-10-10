# tankan のテスト用コーパス

図種ごとのディレクトリに、`corpus_test.rs` が全件受理を確かめる Mermaid 記法の入力（`.mmd`）を
置いている。代表例は insta スナップショット（`tests/snapshots/`）で SVG の出力も確かめる。

## 出所

- tankan の開発（2026-07 以降）でテスト用に書いたもの。各図種の構文を網羅するため、
  一部は Mermaid 公式ドキュメントの例文に倣っている（同じ構文・似た題材）
- Mermaid（<https://github.com/mermaid-js/mermaid>）は MIT ライセンス
  （Copyright (c) 2014 - 2022 Knut Sveidqvist）。例文に倣った入力もこの許諾の範囲で使っている
- どのファイルがどの例文に倣ったかの記録は無い。新しく足す入力は、公式の例文を写す場合は
  ファイル先頭の `%%` コメントに出典を書く

## 配布

このディレクトリ・`corpus_test.rs`・`tests/snapshots/` は crates.io の配布物に含めない
（`Cargo.toml` の `exclude`）。リポジトリ内のテスト専用のデータ。
