# 第三者ライセンスの通知の材料

yuzu が配るもの（リリースのアーカイブ・利用者のサイトの dist）に入れる、第三者の
ソフトウェアのライセンス表記（THIRD-PARTY-LICENSES）を組み立てるための設定と記録。
組み立ては `scripts/third-party-licenses.sh` が行う（Phase 81）。

| ファイル | 中身 | 更新のきっかけ |
| --- | --- | --- |
| `about.toml` | cargo-about の設定（使ってよいライセンスの一覧と、crate のファイルを cargo-about が認識できないときの `clarify`） | 新しいライセンスの crate が入ったとき（リポジトリ直下の `deny.toml` の `allow` にも同じものを足す。`third-party-licenses.sh check` が一致を照合する）・clarify した crate のファイルが変わったとき（生成が失敗して知らせる。deps.yml が依存を変える PR で生成を試す） |
| `supplements.tsv` ＋ `supplements/` | crate にライセンスの本文が無いもの（ruzstd・vaporetto・vaporetto_rules・siphasher）の原文。取得元は版・コミットで固定 | 生成が「雛形のプレースホルダーが残っている」で失敗したとき |
| `two-face-<版>-acknowledgements.md` | two-face が同梱する構文定義・テーマのライセンス一覧 | two-face の版が変わったとき（CI の `third-party-licenses.sh check` が知らせる） |
| `syntect-default-themes.txt` | syntect の既定テーマのうち two-face の一覧に無いもの（InspiredGitHub・base16）のライセンス文 | syntect の既定テーマが変わったとき |

## 生成物と置き場

| 通知 | 置き場 | 作るもの | コミット |
| --- | --- | --- | --- |
| テーマの vendor 資産（mermaid と束ねたパッケージ・KaTeX） | `crates/yuzu-theme/assets/static/vendor/THIRD-PARTY-LICENSES.txt`（dist の `_assets/vendor/`） | `scripts/vendor-mermaid.sh`・`scripts/vendor-katex.sh`（材料は `crates/yuzu-theme/licenses/`） | する |
| 検索 wasm の依存 crate と分かち書きモデル | `crates/yuzu-index/assets/search/THIRD-PARTY-LICENSES.txt`（dist の `_search/`） | `scripts/build-search-wasm.sh`（文だけなら `third-party-licenses.sh wasm`） | する |
| バイナリ（依存 crate ＋ 上の 2 つ ＋ two-face ＋ syntect） | リリースのアーカイブの `THIRD-PARTY-LICENSES` | release.yml の `licenses` ジョブ（`third-party-licenses.sh binary`） | しない |

dist 用の 2 枚はバイナリに埋め込むのでコミットする（build の既定経路にネットワーク I/O を
入れない）。アーカイブ用は Cargo.lock が動くたびに変わるので、リリースのたびに作る。

## 雛形に戻る crate の扱い

cargo-about は、crate にライセンスの本文が無いか、ファイルを認識できない（複数の本文が
1 ファイルに入っている・字下げされている等）と、SPDX の雛形（著作権表示が
`<year> <copyright holders>`）に戻り、`--fail` でも止まらない（PR #31 のレビュー指摘）。
`scripts/third-party-licenses.sh` は cargo-about の JSON を jq で描き、雛形の
プレースホルダーが残っていたら失敗する。失敗したら次のどちらかで補う。

- crate にファイルがある（windows 系の `license-mit`・comrak の `COPYING` 等）→ `about.toml` に
  `[<crate>.clarify]` を足し、ファイルのパスと sha256 を書く
- crate に本文が無い → 公開コミット（crate の `.cargo_vcs_info.json` の `sha1`）の原文を取って
  `supplements/` に置き、`supplements.tsv` に 1 行足す。上流にも本文が無いときは、著作権表示に
  ライセンスの許諾文を添え、その旨を書く（siphasher）

mermaid が束ねる npm パッケージは `scripts/vendor-mermaid.sh` が扱う。LICENSE ファイルが無ければ
README の License 節から取り、著作権表示と許諾文がそろわなければ止まる。

## two-face の一覧を作り直す

two-face は一覧を実行時の API（`two_face::acknowledgement::listing().to_md()`）でしか出さないので、
使い捨てのプログラムで書き出してコミットする。版を上げたら次のとおり作り直し、
`scripts/third-party-licenses.sh` の `TWO_FACE_VERSION` も合わせる。

```bash
mkdir -p /tmp/twoface-ack/src && cd /tmp/twoface-ack
cat > Cargo.toml <<'EOF'
[package]
name = "twoface-ack"
version = "0.0.0"
edition = "2021"
publish = false

[dependencies]
two-face = { version = "=<版>", default-features = false, features = ["syntect-fancy"] }

[workspace]
EOF
echo 'fn main() { print!("{}", two_face::acknowledgement::listing().to_md()); }' > src/main.rs
cargo run -q --release > <リポジトリ>/licenses/two-face-<版>-acknowledgements.md
```

## 必要なツール

- cargo-about（版は `scripts/third-party-licenses.sh` の `ABOUT_VERSION`）:
  `cargo install cargo-about --version <版> --locked --features cli`（0.9 系は `cli` feature が要る）
- jq（`scripts/vendor-mermaid.sh` が source map と npm のメタデータを読む）
