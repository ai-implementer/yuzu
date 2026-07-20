#!/usr/bin/env bash
# vaporetto の学習済みモデル（辞書なし SUW、MIT OR Apache-2.0）を
# mikan の assets へ取得する。
# 更新するときは MODEL/VERSION を変えて実行し、
# crates/mikan/assets/model/README.md の記録も更新すること。
set -euo pipefail

VERSION="${VAPORETTO_MODELS_VERSION:-v0.5.0}"
MODEL="${VAPORETTO_MODEL:-bccwj-suw_c1.0}"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
DEST_DIR="$ROOT/crates/mikan/assets/model"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

curl -fL "https://github.com/daac-tools/vaporetto-models/releases/download/${VERSION}/${MODEL}.tar.xz" \
  -o "$TMP/model.tar.xz"
tar xJf "$TMP/model.tar.xz" -C "$TMP"

mkdir -p "$DEST_DIR"
MODEL_FILE="$(find "$TMP" -name '*.model.zst' | head -1)"
cp "$MODEL_FILE" "$DEST_DIR/${MODEL}.model.zst"

echo "vendored: $DEST_DIR/${MODEL}.model.zst"
echo "sha256:   $(shasum -a 256 "$DEST_DIR/${MODEL}.model.zst" | cut -d' ' -f1)"
echo "size:     $(du -h "$DEST_DIR/${MODEL}.model.zst" | cut -f1)"
find "$TMP" -iname 'LICENSE*' -o -iname 'README*' | head -5
