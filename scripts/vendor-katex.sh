#!/usr/bin/env bash
# KaTeX（katex.min.js / katex.min.css / fonts、MIT）をテーマの vendor へ取得する。
# 更新するときは KATEX_VERSION を変えて実行し、
# crates/yuzu-theme/assets/static/vendor/README.md の記録も更新すること。
#
# fonts は woff2 のみ同梱する（katex.min.css は woff2 → woff → ttf の順で
# 参照するが、モダンブラウザは woff2 しか取得しないため ≈500KB 削減できる）。
set -euo pipefail

KATEX_VERSION="${KATEX_VERSION:-0.17.0}"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
DEST="$ROOT/crates/yuzu-theme/assets/static/vendor/katex"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

curl -fL "https://registry.npmjs.org/katex/-/katex-${KATEX_VERSION}.tgz" -o "$TMP/katex.tgz"
tar xzf "$TMP/katex.tgz" -C "$TMP"

rm -rf "$DEST"
mkdir -p "$DEST/fonts"
cp "$TMP/package/dist/katex.min.js" "$TMP/package/dist/katex.min.css" "$DEST/"
cp "$TMP"/package/dist/fonts/*.woff2 "$DEST/fonts/"

echo "vendored: ${DEST} (KaTeX ${KATEX_VERSION})"
echo "size:     $(du -sh "$DEST" | cut -f1)"
echo "fonts:    $(ls "$DEST/fonts" | wc -l | tr -d ' ') files (woff2)"
