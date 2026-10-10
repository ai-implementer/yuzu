# 第三者ライセンスの通知の材料

yuzu が配るもの（リリースのアーカイブ・利用者のサイトの dist）に入れる、第三者の
ソフトウェアのライセンス表記（THIRD-PARTY-LICENSES）を組み立てるための設定と記録。
組み立ては `scripts/third-party-licenses.sh` が行う（Phase 81）。

| ファイル | 中身 | 更新のきっかけ |
| --- | --- | --- |
| `about.toml` | cargo-about の設定（使ってよいライセンスの一覧） | 新しいライセンスの crate が入ったとき（生成が失敗して知らせる） |
| `about.hbs` | cargo-about のテンプレート（テキスト） | 体裁を変えるとき |
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
