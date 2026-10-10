#!/usr/bin/env bash
# 第三者ライセンスの通知（THIRD-PARTY-LICENSES）を組み立てる。
#
#   vendor           テーマの vendor 資産（mermaid と束ねたパッケージ・KaTeX）の通知
#                    → crates/yuzu-theme/assets/static/vendor/THIRD-PARTY-LICENSES.txt
#                      （dist の _assets/vendor/ へ出る。材料は crates/yuzu-theme/licenses/ の
#                      mermaid.txt・katex.txt で、vendor-mermaid.sh / vendor-katex.sh が作る）
#   wasm             検索 wasm（mikan-wasm の wasm32 向け依存）と分かち書きモデルの通知
#                    → crates/yuzu-index/assets/search/THIRD-PARTY-LICENSES.txt
#                      （dist の _search/ へ出る。build-search-wasm.sh が wasm と一緒に呼ぶ）
#   binary <出力>    リリースのアーカイブに入れる通知（release.yml が呼ぶ）。バイナリの
#                    依存 crate（配布する 4 ターゲット）・two-face と syntect の同梱データ・
#                    バイナリに埋め込んだ上の 2 つの通知をまとめる
#   check            licenses/ に置いた記録が Cargo.lock の版と合っているか（CI が呼ぶ）
#
# vendor と wasm の生成物はコミットする（バイナリに埋め込むため。build の既定経路に
# ネットワーク I/O を入れない）。binary はリリースのたびに生成し、コミットしない。
#
# wasm / binary の前提: cargo-about（版は ABOUT_VERSION と一致）。--frozen で実行するので、
# 対象の crate を先に `cargo fetch --locked` しておくこと（ネットワークに出ない）
set -euo pipefail
# 並び順（sort・awk）を呼び出し元のロケールに左右させない（生成物はコミットするので、
# 実行環境ごとに意味のない差分が出ないようにする）
export LC_ALL=C

ABOUT_VERSION="0.9.2"
# licenses/ に記録した two-face の一覧の版（Cargo.lock と一致させる。check が照合する）
TWO_FACE_VERSION="0.5.1"
BINARY_TARGETS=(aarch64-apple-darwin x86_64-apple-darwin x86_64-unknown-linux-gnu x86_64-pc-windows-msvc)
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
RULE="================================================================================"

usage() {
  echo "usage: $0 vendor | wasm | binary <出力ファイル> | check" >&2
  exit 2
}

lock_version() {
  # Cargo.lock から crate の版を取り出す（同名の版が複数ある場合は最初の 1 つ）
  awk -v n="$1" '$0 == "name = \"" n "\"" { getline; gsub(/version = |"/, ""); print; exit }' "$ROOT/Cargo.lock"
}

check_pins() {
  local got
  got="$(lock_version two-face)"
  if [ "$got" != "$TWO_FACE_VERSION" ]; then
    echo "two-face の版が licenses/ の記録と違います（Cargo.lock: $got / 記録: $TWO_FACE_VERSION）。" >&2
    echo "licenses/README.md の手順で two-face の一覧を作り直し、TWO_FACE_VERSION を更新してください" >&2
    exit 1
  fi
  test -f "$ROOT/licenses/two-face-${TWO_FACE_VERSION}-acknowledgements.md"
}

require_about() {
  local got
  got="$(cargo about --version 2>/dev/null | awk '{print $2}')" || true
  if [ "$got" != "$ABOUT_VERSION" ]; then
    echo "cargo-about $ABOUT_VERSION が必要です（実際: ${got:-なし}）。" >&2
    echo "  cargo install cargo-about --version $ABOUT_VERSION --locked --features cli" >&2
    exit 1
  fi
}

# cargo-about で crate の一覧を出す（引数は -m と --target）
about() {
  NO_COLOR=1 cargo about generate -c "$ROOT/licenses/about.toml" --frozen --fail \
    "$@" "$ROOT/licenses/about.hbs"
}

vendor_notice() {
  local out="$ROOT/crates/yuzu-theme/assets/static/vendor/THIRD-PARTY-LICENSES.txt"
  local parts="$ROOT/crates/yuzu-theme/licenses"
  local part
  for part in mermaid katex; do
    if [ ! -f "$parts/$part.txt" ]; then
      echo "$parts/$part.txt がありません。scripts/vendor-$part.sh を実行してください" >&2
      exit 1
    fi
  done
  {
    echo "yuzu が利用者のサイトへ配る第三者の資産（_assets/vendor/）のライセンス表記"
    echo
    echo "- mermaid.min.js（図のブラウザ描画）と、mermaid が束ねている npm パッケージ"
    echo "- katex/（数式のブラウザ描画。JS・CSS・フォント）"
    echo
    echo "生成: scripts/vendor-mermaid.sh・scripts/vendor-katex.sh・scripts/third-party-licenses.sh"
    echo
    echo "$RULE"
    echo
    cat "$parts/mermaid.txt"
    echo
    echo "$RULE"
    echo
    cat "$parts/katex.txt"
  } > "$out.new"
  mv "$out.new" "$out"
  echo "notice: $out"
}

model_section() {
  local model="$ROOT/crates/mikan/assets/model"
  echo "$RULE"
  echo "分かち書きモデル（_search/model.zst）"
  echo
  echo "vaporetto の学習済みモデル bccwj-suw_c1.0（https://github.com/daac-tools/vaporetto-models"
  echo "の v0.5.0）。ライセンスは MIT OR Apache-2.0 で、配布元のアーカイブに同梱の本文を以下に載せる。"
  echo "$RULE"
  echo
  echo "--- LICENSE-MIT ---"
  echo
  cat "$model/LICENSE-MIT"
  echo
  echo "--- LICENSE-APACHE ---"
  echo
  cat "$model/LICENSE-APACHE"
}

wasm_notice() {
  require_about
  local out="$ROOT/crates/yuzu-index/assets/search/THIRD-PARTY-LICENSES.txt"
  {
    echo "yuzu のブラウザ検索（_search/search_bg.wasm・search.js）と分かち書きモデル"
    echo "（_search/model.zst）に含まれる第三者のソフトウェアのライセンス表記"
    echo
    echo "- Rust の crate: mikan-wasm の wasm32-unknown-unknown 向けの通常依存（cargo-about $ABOUT_VERSION）"
    echo "- 分かち書きモデル（末尾の節）"
    echo
    echo "生成: scripts/third-party-licenses.sh wasm"
    echo
    (cd "$ROOT" && about -m crates/mikan-wasm/Cargo.toml --target wasm32-unknown-unknown)
    echo
    model_section
  } > "$out.new"
  mv "$out.new" "$out"
  echo "notice: $out"
}

binary_notice() {
  local out="$1"
  require_about
  check_pins
  local version targets=()
  version="$(grep -m1 '^version = ' "$ROOT/Cargo.toml" | cut -d'"' -f2)"
  for t in "${BINARY_TARGETS[@]}"; do targets+=(--target "$t"); done
  {
    echo "yuzu ${version} のバイナリに含まれる第三者のソフトウェアのライセンス表記"
    echo
    echo "1. Rust の crate（yuzu-cli の通常依存。配布する 4 ターゲットの和集合。cargo-about $ABOUT_VERSION）"
    echo "2. two-face ${TWO_FACE_VERSION} が同梱する構文定義・テーマ"
    echo "3. syntect が同梱する既定のテーマ・構文定義"
    echo "4. 利用者のサイトへ配る資産（mermaid・KaTeX。dist の _assets/vendor/THIRD-PARTY-LICENSES.txt と同じ）"
    echo "5. ブラウザ検索の wasm と分かち書きモデル（dist の _search/THIRD-PARTY-LICENSES.txt と同じ）"
    echo
    echo "生成: scripts/third-party-licenses.sh binary"
    echo
    echo "$RULE"
    echo "1. Rust の crate"
    echo "$RULE"
    (cd "$ROOT" && about -m crates/yuzu-cli/Cargo.toml "${targets[@]}")
    echo
    echo "$RULE"
    echo "2. two-face ${TWO_FACE_VERSION} が同梱する構文定義・テーマ"
    echo "（two_face::acknowledgement::listing().to_md() の出力）"
    echo "$RULE"
    echo
    cat "$ROOT/licenses/two-face-${TWO_FACE_VERSION}-acknowledgements.md"
    echo
    echo "$RULE"
    echo "3. $(head -1 "$ROOT/licenses/syntect-default-themes.txt")"
    echo "$RULE"
    tail -n +2 "$ROOT/licenses/syntect-default-themes.txt"
    echo
    echo "$RULE"
    echo "4. 利用者のサイトへ配る資産"
    echo "$RULE"
    echo
    cat "$ROOT/crates/yuzu-theme/assets/static/vendor/THIRD-PARTY-LICENSES.txt"
    echo
    echo "$RULE"
    echo "5. ブラウザ検索の wasm と分かち書きモデル"
    echo "$RULE"
    echo
    cat "$ROOT/crates/yuzu-index/assets/search/THIRD-PARTY-LICENSES.txt"
  } > "$out"
  echo "notice: $out（$(wc -c < "$out" | tr -d ' ') bytes）"
}

case "${1:-}" in
  vendor) vendor_notice ;;
  wasm) wasm_notice ;;
  binary) [ $# -eq 2 ] || usage; binary_notice "$2" ;;
  check) check_pins && echo "licenses/ の記録は Cargo.lock と一致しています" ;;
  *) usage ;;
esac
