# kabosu corpus

TOML 1.0 仕様（https://toml.io/ja/v1.0.0）の例文を基に作成した自作コーパス。

- `valid/` — 対応範囲で受理される入力。値解釈は `toml` crate（参照実装）と
  一致することを toml_compat_test.rs が縛る
- `invalid/` — 書き間違い（受理してはいけない入力）。参照実装でもエラーに
  なることを縛る = 誤検出の防止
- `unsupported/` — **TOML としては正しいがまだ未対応**の構文
  （inline table / array of tables。float / 16,8,2 進整数 / 複数行文字列 /
  date-time は 0.2 で `valid/` へ移した）。`ParseErrorKind::Unsupported` で一般構文エラーと
  区別されることを corpus_test.rs が縛る。
  このディレクトリが「未対応にするケースの明示的な一覧」の実体

`1.0.0` の条件は公式 [toml-test](https://github.com/toml-lang/toml-test) の
valid / invalid / encoder テスト完全通過（kabosu.md「v0.1 の TOML 対応範囲」）。
その時点でこの自作コーパスは公式ハーネスへ置き換える。
