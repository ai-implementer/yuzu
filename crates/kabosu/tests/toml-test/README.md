# toml-test（vendor）

公式 [toml-test](https://github.com/toml-lang/toml-test)（MIT）から
**TOML 1.0.0 が対象とするケースだけ**を取り込んだもの。
`scripts/vendor-toml-test.sh` が生成するので、手で編集しない。

- タグ: `v2.2.0`
- アーカイブ sha256: `fdab2779b3902eb08030f389a5d53e95c5b49404149ac6f2eda5227a5363c232`
- 取り込んだファイル: 884 件（valid 205 ケース / invalid 474 ケース）
- 選別: 上流の `tests/files-toml-1.0.0`

タグはテストスイートの版であって TOML 仕様の版ではない。
仕様の版で選んでいるのは `files-toml-1.0.0` のほうで、TOML 1.1 専用の
ケースは入っていない（kabosu は TOML 1.0 のパーサなので、1.1 のケースを
入れると「仕様どおり拒否したのに落ちる」テストになる）。

ハーネスは `crates/kabosu/tests/toml_test.rs`。
配布物には含めない（`crates/kabosu/Cargo.toml` の `exclude`）。
