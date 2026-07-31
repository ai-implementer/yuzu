#!/usr/bin/env bash
# KaTeX（katex.min.js / katex.min.css / fonts、MIT）をテーマの vendor へ取得する。
# 更新するときは KATEX_VERSION と KATEX_ARCHIVE_SHA256 を**セットで**書き換えて実行し、
# crates/yuzu-theme/assets/static/vendor/README.md の記録も更新すること
# （新しい sha256 はこのスクリプトの不一致エラーが実測値を表示する）。
#
# fonts は woff2 のみ同梱する（katex.min.css は woff2 → woff → ttf の順で
# 参照するが、モダンブラウザは woff2 しか取得しないため ≈500KB 削減できる）。
set -euo pipefail

KATEX_VERSION="${KATEX_VERSION:-0.17.0}"
# **展開する前**にアーカイブ自体を照合する。中身のファイル単位で検証しても、
# 悪意あるアーカイブの展開そのものは防げない（パストラバーサル等）。
# アーカイブが一致すれば中身は一意に決まるので、fonts 20 ファイルもこれで覆える
KATEX_ARCHIVE_SHA256="${KATEX_ARCHIVE_SHA256:-252efd48f892d178136fe3ba3530d3718b2b087ea81c3a40a877227bc61d5256}"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
DEST="$ROOT/crates/yuzu-theme/assets/static/vendor/katex"
# 完成形は DEST の隣で組んでから差し替える（$TMPDIR は別ファイルシステムの
# ことがあり mv が跨げないため、staging は同一 FS に置く）
STAGING="$DEST.new"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP" "$STAGING"' EXIT

curl -fL "https://registry.npmjs.org/katex/-/katex-${KATEX_VERSION}.tgz" -o "$TMP/katex.tgz"

ACTUAL="$(shasum -a 256 "$TMP/katex.tgz" | cut -d' ' -f1)"
if [ "$ACTUAL" != "$KATEX_ARCHIVE_SHA256" ]; then
  echo "アーカイブの sha256 が一致しません（期待 $KATEX_ARCHIVE_SHA256 / 実際 ${ACTUAL}）" >&2
  exit 1
fi

tar xzf "$TMP/katex.tgz" -C "$TMP"

rm -rf "$STAGING"
mkdir -p "$STAGING/fonts"
cp "$TMP/package/dist/katex.min.js" "$TMP/package/dist/katex.min.css" "$STAGING/"
cp "$TMP"/package/dist/fonts/*.woff2 "$STAGING/fonts/"

# ここまで成功して初めて既存の同梱物を置き換える
rm -rf "$DEST"
mv "$STAGING" "$DEST"

echo "vendored: ${DEST} (KaTeX ${KATEX_VERSION})"
echo "archive:  $ACTUAL"
echo "size:     $(du -sh "$DEST" | cut -f1)"
echo "fonts:    $(ls "$DEST/fonts" | wc -l | tr -d ' ') files (woff2)"
