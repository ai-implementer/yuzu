#!/usr/bin/env bash
# mermaid.min.js をテーマの vendor ディレクトリへ取得する。
# 更新するときはバージョンを上げてから実行し、
# crates/yuzu-theme/assets/static/vendor/README.md の記録も更新すること。
set -euo pipefail

MERMAID_VERSION="${MERMAID_VERSION:-11}"
DEST="$(cd "$(dirname "$0")/.." && pwd)/crates/yuzu-theme/assets/static/vendor/mermaid.min.js"

curl -fL "https://cdn.jsdelivr.net/npm/mermaid@${MERMAID_VERSION}/dist/mermaid.min.js" -o "$DEST"
echo "vendored: $DEST ($(du -h "$DEST" | cut -f1))"
