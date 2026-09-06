# kabosu corpus

TOML 1.0 仕様（https://toml.io/ja/v1.0.0）の例文を基に作成した自作コーパス。

- `valid/` — 対応範囲で受理される入力。値解釈は `toml` crate（参照実装）と
  一致することを toml_compat_test.rs が縛る
- `invalid/` — 書き間違い（受理してはいけない入力）。参照実装でもエラーに
  なることを縛る = 誤検出の防止
`unsupported/`（TOML としては正しいがまだ未対応の構文を並べたディレクトリ）は
0.2 で TOML 1.0 の全構文に対応したため無くなった。

**参照実装は `toml 0.9+spec-1.1.0` = TOML 1.1 の構文も受理する**ので、
kabosu が TOML 1.0 どおり拒否するもの（秒を省略した `07:32`・インラインテーブルの
改行と末尾カンマ）は `invalid/` に置けない（「invalid は参照実装でもエラー」の
照合が落ちる）。これらは crate 内のユニットテストで縛る。

`1.0.0` の条件は公式 [toml-test](https://github.com/toml-lang/toml-test) の
valid / invalid / encoder テスト完全通過（kabosu.md「v0.1 の TOML 対応範囲」）。
その時点でこの自作コーパスは公式ハーネスへ置き換える。
