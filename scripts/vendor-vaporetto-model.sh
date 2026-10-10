#!/usr/bin/env bash
# vaporetto の学習済みモデル（辞書なし SUW、MIT OR Apache-2.0）を
# mikan の assets へ取得する。
# 更新するときは MODEL / VERSION と VAPORETTO_ARCHIVE_SHA256 を**セットで**
# 書き換えて実行し、crates/mikan/assets/model/README.md の記録も更新すること。
set -euo pipefail

VERSION="${VAPORETTO_MODELS_VERSION:-v0.5.0}"
MODEL="${VAPORETTO_MODEL:-bccwj-suw_c1.0}"
# **展開する前**にアーカイブ自体を照合する（展開後の検証では、悪意ある
# アーカイブの展開そのものを防げない）。トークナイザは index 時（ネイティブ）と
# query 時（wasm）で同一モデルバイトを使う契約なので、取り違えは検索結果の
# 不整合に直結する
VAPORETTO_ARCHIVE_SHA256="${VAPORETTO_ARCHIVE_SHA256:-bf90e5c25bbb9db013c2f077fc08be5e8b68b3b8f6555cf936ee784cca0ec6aa}"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
DEST_DIR="$ROOT/crates/mikan/assets/model"
TMP="$(mktemp -d)"
# 差し替えは DEST の隣（同一ファイルシステム）への cp ＋ mv で行う。
# DEST へ直接 cp すると中断時に部分ファイルが残る
STAGING="$DEST_DIR/${MODEL}.model.zst.new"
trap 'rm -rf "$TMP" "$STAGING"' EXIT

curl -fL "https://github.com/daac-tools/vaporetto-models/releases/download/${VERSION}/${MODEL}.tar.xz" \
  -o "$TMP/model.tar.xz"

ACTUAL="$(shasum -a 256 "$TMP/model.tar.xz" | cut -d' ' -f1)"
if [ "$ACTUAL" != "$VAPORETTO_ARCHIVE_SHA256" ]; then
  echo "アーカイブの sha256 が一致しません（期待 $VAPORETTO_ARCHIVE_SHA256 / 実際 ${ACTUAL}）" >&2
  exit 1
fi

tar xJf "$TMP/model.tar.xz" -C "$TMP"
MODEL_FILE="$(find "$TMP" -name '*.model.zst' | head -1)"

# モデルと同梱のライセンス文（MIT OR Apache-2.0）。mikan の crate と、dist の
# `_search/THIRD-PARTY-LICENSES.txt`（scripts/build-search-wasm.sh が組む）の両方が使う
LICENSE_MIT="$(find "$TMP" -name 'LICENSE-MIT' | head -1)"
LICENSE_APACHE="$(find "$TMP" -name 'LICENSE-APACHE' | head -1)"
if [ -z "$LICENSE_MIT" ] || [ -z "$LICENSE_APACHE" ]; then
  echo "アーカイブに LICENSE-MIT / LICENSE-APACHE が見つかりません" >&2
  exit 1
fi

mkdir -p "$DEST_DIR"
cp "$MODEL_FILE" "$STAGING"
mv "$STAGING" "$DEST_DIR/${MODEL}.model.zst"
cp "$LICENSE_MIT" "$DEST_DIR/LICENSE-MIT"
cp "$LICENSE_APACHE" "$DEST_DIR/LICENSE-APACHE"

echo "vendored: $DEST_DIR/${MODEL}.model.zst（+ LICENSE-MIT / LICENSE-APACHE）"
echo "archive:  $ACTUAL"
echo "model:    $(shasum -a 256 "$DEST_DIR/${MODEL}.model.zst" | cut -d' ' -f1)"
echo "size:     $(du -h "$DEST_DIR/${MODEL}.model.zst" | cut -f1)"
