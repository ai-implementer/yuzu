#!/usr/bin/env bash
# KaTeX（katex.min.js / katex.min.css / fonts、MIT）をテーマの vendor へ取得する。
# 更新するときは KATEX_VERSION と KATEX_ARCHIVE_SHA256 を**セットで**書き換えて実行し、
# crates/yuzu-theme/assets/static/vendor/README.md の記録も更新すること
# （新しい sha256 はこのスクリプトの不一致エラーが実測値を表示する）。
#
# fonts は woff2 のみ同梱する（katex.min.css は woff2 → woff → ttf の順で
# 参照するが、モダンブラウザは woff2 しか取得しないため ≈500KB 削減できる）。
set -euo pipefail

KATEX_VERSION="${KATEX_VERSION:-0.18.11}"
# **展開する前**にアーカイブ自体を照合する。中身のファイル単位で検証しても、
# 悪意あるアーカイブの展開そのものは防げない（パストラバーサル等）。
# アーカイブが一致すれば中身は一意に決まるので、fonts 20 ファイルもこれで覆える
KATEX_ARCHIVE_SHA256="${KATEX_ARCHIVE_SHA256:-a11d6ab44180c6e25a09108c6c391866d0600b49a61ef725368d567f4918ffc3}"
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

# ライセンス文: KaTeX 本体（tarball の LICENSE）と、フォントの生成元 katex-fonts の LICENSE
# （コミットで固定。フォントは katex-fonts のスクリプトで作られ、同じく MIT）。
# scripts/third-party-licenses.sh vendor が dist 用の通知へ組み込む
NOTICE="$ROOT/crates/yuzu-theme/licenses/katex.txt"
KATEX_FONTS_LICENSE_URL="https://raw.githubusercontent.com/KaTeX/katex-fonts/feee984b451fea029d921ea0d41b917f56c8b7f6/LICENSE"
curl -fsSL "$KATEX_FONTS_LICENSE_URL" -o "$TMP/katex-fonts-LICENSE"
{
  echo "KaTeX ${KATEX_VERSION}（https://github.com/KaTeX/KaTeX。npm の katex@${KATEX_VERSION}）"
  echo
  tr -d '\r' < "$TMP/package/LICENSE"
  echo
  echo "--------------------------------------------------------------------------------"
  echo "KaTeX のフォント（fonts/*.woff2）は katex-fonts（https://github.com/KaTeX/katex-fonts）の"
  echo "スクリプトで作られている。katex-fonts の LICENSE:"
  echo "--------------------------------------------------------------------------------"
  echo
  tr -d '\r' < "$TMP/katex-fonts-LICENSE"
} > "$TMP/katex-notice.txt"

# ここまで成功して初めて既存の同梱物を置き換える
rm -rf "$DEST"
mv "$STAGING" "$DEST"
mv "$TMP/katex-notice.txt" "$NOTICE"
"$ROOT/scripts/third-party-licenses.sh" vendor

echo "vendored: ${DEST} (KaTeX ${KATEX_VERSION})"
echo "archive:  $ACTUAL"
echo "size:     $(du -sh "$DEST" | cut -f1)"
echo "fonts:    $(ls "$DEST/fonts" | wc -l | tr -d ' ') files (woff2)"
