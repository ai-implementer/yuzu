#!/usr/bin/env bash
# yuzu-search-wasm をビルドして yuzu-index の assets へ vendor する。
#
# 前提:
#   rustup target add wasm32-unknown-unknown
#   cargo install wasm-bindgen-cli --version <crates/yuzu-search-wasm の wasm-bindgen と同一>
#   binaryen（wasm-opt）が PATH にあること（例: brew install binaryen）
#
# 実行後、crates/yuzu-index/assets/search/README.md にサイズを記録すること。
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
DEST="$ROOT/crates/yuzu-index/assets/search"

# wasm-bindgen crate と CLI は完全同一バージョンが必須（不一致は実行時に即エラー）
WB_VERSION="$(grep -oE 'wasm-bindgen = "=([0-9.]+)"' "$ROOT/Cargo.toml" | grep -oE '[0-9.]+' | head -1)"
if [ -z "$WB_VERSION" ]; then
  echo "error: ルート Cargo.toml から wasm-bindgen のピンバージョンを取得できません" >&2
  exit 1
fi
if ! wasm-bindgen --version | grep -q "$WB_VERSION"; then
  echo "error: wasm-bindgen-cli のバージョンが crate（=$WB_VERSION）と一致しません:" >&2
  echo "  $(wasm-bindgen --version)" >&2
  echo "  cargo install wasm-bindgen-cli --version $WB_VERSION --force で揃えてください" >&2
  exit 1
fi

cargo build -p yuzu-search-wasm --profile wasm-release --target wasm32-unknown-unknown

wasm-bindgen --target web --no-typescript --out-name search --out-dir "$DEST" \
  "$ROOT/target/wasm32-unknown-unknown/wasm-release/yuzu_search_wasm.wasm"

wasm-opt -Oz --strip-debug -o "$DEST/search_bg.wasm" "$DEST/search_bg.wasm"

echo "vendored:"
ls -lh "$DEST"/search.js "$DEST"/search_bg.wasm | awk '{print "  " $9 ": " $5}'
