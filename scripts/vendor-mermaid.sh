#!/usr/bin/env bash
# mermaid.min.js をテーマの vendor ディレクトリへ取得する。
# 更新するときは MERMAID_VERSION と MERMAID_SHA256 を**セットで**書き換えて実行し、
# crates/yuzu-theme/assets/static/vendor/README.md の記録も更新すること
# （新しい sha256 はこのスクリプトの不一致エラーが実測値を表示する）。
set -euo pipefail

# メジャーだけの指定（`11`）にすると実行時期で中身が変わって再現性が無くなるため、
# vendor-katex.sh / vendor-vaporetto-model.sh と同じくパッチまで固定する
MERMAID_VERSION="${MERMAID_VERSION:-11.16.0}"
MERMAID_SHA256="${MERMAID_SHA256:-74d7c46dabca328c2294733910a8aa1ed0c37451776e8d5295da38a2b758fb9b}"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
DEST="$ROOT/crates/yuzu-theme/assets/static/vendor/mermaid.min.js"
TMP="$(mktemp -d)"
# 差し替えは DEST の隣（同一ファイルシステム）への cp ＋ mv で行う。
# DEST へ直接 cp すると中断時に部分ファイルが残る
STAGING="$DEST.new"
trap 'rm -rf "$TMP" "$STAGING"' EXIT

curl -fL "https://cdn.jsdelivr.net/npm/mermaid@${MERMAID_VERSION}/dist/mermaid.min.js" \
  -o "$TMP/mermaid.min.js"

ACTUAL="$(shasum -a 256 "$TMP/mermaid.min.js" | cut -d' ' -f1)"
if [ "$ACTUAL" != "$MERMAID_SHA256" ]; then
  echo "sha256 が一致しません（期待 $MERMAID_SHA256 / 実際 ${ACTUAL}）" >&2
  exit 1
fi

cp "$TMP/mermaid.min.js" "$STAGING"
mv "$STAGING" "$DEST"
echo "vendored: $DEST (mermaid ${MERMAID_VERSION}, $(du -h "$DEST" | cut -f1))"
echo "sha256:   $ACTUAL"
