#!/usr/bin/env bash
# mermaid.min.js をテーマの vendor ディレクトリへ取得し、ライセンス文を組む。
# 更新するときは MERMAID_VERSION と MERMAID_ARCHIVE_SHA256 を**セットで**書き換えて実行し、
# crates/yuzu-theme/assets/static/vendor/README.md の記録も更新すること
# （新しい sha256 はこのスクリプトの不一致エラーが実測値を表示する）。
#
# mermaid.min.js は約 70 の npm パッケージ（d3・dagre・cytoscape・DOMPurify 等）を束ねた
# ものだが、mermaid のパッケージには束ねた側のライセンス文が入っていない。npm の tarball に
# 同梱の source map（dist/mermaid.min.js.map）の sources から束ねたパッケージ名と版を取り出し、
# 各パッケージの tarball の LICENSE を集めて crates/yuzu-theme/licenses/mermaid.txt を書く。
# 最後に scripts/third-party-licenses.sh vendor が dist 用の通知
# （static/vendor/THIRD-PARTY-LICENSES.txt）を組み直す。
#
# 前提: curl / tar / jq / openssl / shasum
set -euo pipefail
# 並び順（sort・awk）を呼び出し元のロケールに左右させない（生成物はコミットするので、
# 実行環境ごとに意味のない差分が出ないようにする）
export LC_ALL=C

# メジャーだけの指定（`11`）にすると実行時期で中身が変わって再現性が無くなるため、
# vendor-katex.sh / vendor-vaporetto-model.sh と同じくパッチまで固定する
MERMAID_VERSION="${MERMAID_VERSION:-11.17.2}"
# **展開する前**にアーカイブ自体を照合する（KaTeX と同じ）
MERMAID_ARCHIVE_SHA256="${MERMAID_ARCHIVE_SHA256:-6ad2f42c3fc26bbf9e45cbb6d11898972573ea52b33a5f4ff51952899f950ffd}"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
DEST="$ROOT/crates/yuzu-theme/assets/static/vendor/mermaid.min.js"
NOTICE="$ROOT/crates/yuzu-theme/licenses/mermaid.txt"
TMP="$(mktemp -d)"
# 差し替えは DEST の隣（同一ファイルシステム）への cp ＋ mv で行う。
# DEST へ直接 cp すると中断時に部分ファイルが残る
STAGING="$DEST.new"
trap 'rm -rf "$TMP" "$STAGING" "$NOTICE.new"' EXIT

for cmd in curl tar jq openssl shasum; do
  command -v "$cmd" >/dev/null || { echo "$cmd が必要です" >&2; exit 1; }
done

curl -fsSL "https://registry.npmjs.org/mermaid/-/mermaid-${MERMAID_VERSION}.tgz" -o "$TMP/mermaid.tgz"
ACTUAL="$(shasum -a 256 "$TMP/mermaid.tgz" | cut -d' ' -f1)"
if [ "$ACTUAL" != "$MERMAID_ARCHIVE_SHA256" ]; then
  echo "アーカイブの sha256 が一致しません（期待 $MERMAID_ARCHIVE_SHA256 / 実際 ${ACTUAL}）" >&2
  exit 1
fi
mkdir "$TMP/mermaid"
tar xzf "$TMP/mermaid.tgz" -C "$TMP/mermaid" \
  package/LICENSE package/dist/mermaid.min.js package/dist/mermaid.min.js.map

# --- 束ねたパッケージの一覧（pnpm の配置 `node_modules/.pnpm/<spec>/node_modules/<name>/` から）
# spec は `<name の / を + にしたもの>@<版>` に、peer 依存やパッチの印（`_…`）が続く
jq -r '.sources[]' "$TMP/mermaid/package/dist/mermaid.min.js.map" |
  sed -nE 's#.*node_modules/\.pnpm/([^/]+)/node_modules/((@[^/]+/)?[^/]+)/.*#\1 \2#p' |
  sort -u |
  while read -r spec name; do
    rest="${spec#"${name//\//+}"@}"
    echo "$name ${rest%%_*}"
  done | sort -u > "$TMP/packages.txt"
echo "束ねたパッケージ: $(wc -l < "$TMP/packages.txt" | tr -d ' ') 個"

# --- 各パッケージのライセンス文を集める（npm の integrity を照合してから展開する）
mkdir "$TMP/pkgs" "$TMP/texts"
: > "$TMP/entries.tsv"   # 本文のハッシュ \t 名前 \t 版 \t package.json の license
while read -r name version; do
  base="${name##*/}"
  meta="$(curl -fsSL "https://registry.npmjs.org/${name}/${version}")"
  url="$(jq -r '.dist.tarball' <<<"$meta")"
  integrity="$(jq -r '.dist.integrity' <<<"$meta")"
  license="$(jq -r 'if (.license|type) == "string" then .license else (.license.type // "package.json に記載なし") end' <<<"$meta")"
  tgz="$TMP/pkgs/${base}-${version}.tgz"
  curl -fsSL "$url" -o "$tgz"
  got="sha512-$(openssl dgst -sha512 -binary "$tgz" | openssl base64 -A)"
  if [ "$got" != "$integrity" ]; then
    echo "$name@$version の integrity が一致しません" >&2
    exit 1
  fi
  dir="$TMP/pkgs/${base}-${version}"
  mkdir -p "$dir"
  tar xzf "$tgz" -C "$dir"
  # パッケージ直下の LICENSE / LICENCE / COPYING（大文字小文字を問わない）
  file="$(find "$dir"/*/ -maxdepth 1 -type f \( -iname 'licen[cs]e*' -o -iname 'copying*' \) | sort | head -1)"
  text="$TMP/text.tmp"
  if [ -n "$file" ]; then
    # 改行コードを揃え、末尾の空行を落とす（同じ本文を 1 回にまとめるため）
    tr -d '\r' < "$file" | sed -e :a -e '/^\n*$/{$d;N;ba' -e '}' > "$text"
  else
    # LICENSE ファイルが無いパッケージは SPDX の本文を載せ、その旨を書く（著作権行は無い）
    { echo "（このパッケージに LICENSE ファイルは無い。package.json の license は ${license}。"
      echo "  以下は SPDX のライセンス本文）"
      echo
      curl -fsSL "https://raw.githubusercontent.com/spdx/license-list-data/main/text/${license}.txt"
    } > "$text"
  fi
  hash="$(shasum -a 256 "$text" | cut -d' ' -f1)"
  [ -f "$TMP/texts/$hash" ] || cp "$text" "$TMP/texts/$hash"
  printf '%s\t%s\t%s\t%s\n' "$hash" "$name" "$version" "$license" >> "$TMP/entries.tsv"
done < "$TMP/packages.txt"

# --- 組み立て: mermaid 自身 → 同じ本文ごとに、その本文で配布されているパッケージを列挙
{
  echo "mermaid ${MERMAID_VERSION}（https://github.com/mermaid-js/mermaid。npm の mermaid@${MERMAID_VERSION}）"
  echo "mermaid のモノレポ内のパッケージ（@mermaid-js/parser）も同じライセンスで含まれている。"
  echo
  tr -d '\r' < "$TMP/mermaid/package/LICENSE"
  echo
  echo "mermaid.min.js が束ねている npm パッケージ（source map の sources から抽出。"
  echo "$(wc -l < "$TMP/packages.txt" | tr -d ' ') 個）のライセンス文。同じ本文は 1 回だけ載せ、その本文で配布されている"
  echo "パッケージを列挙する。"
  # 本文の並びは、その本文を使う最初のパッケージ名の順（実行ごとに同じ出力にする）
  sort -t$'\t' -k2,2 -k3,3 "$TMP/entries.tsv" | cut -f1 | awk '!seen[$0]++' |
    while read -r hash; do
      echo
      echo "--------------------------------------------------------------------------------"
      awk -F'\t' -v h="$hash" '$1 == h { printf "  - %s %s（%s）\n", $2, $3, $4 }' "$TMP/entries.tsv" | sort
      echo "--------------------------------------------------------------------------------"
      echo
      cat "$TMP/texts/$hash"
    done
} > "$NOTICE.new"
mv "$NOTICE.new" "$NOTICE"

cp "$TMP/mermaid/package/dist/mermaid.min.js" "$STAGING"
mv "$STAGING" "$DEST"
"$ROOT/scripts/third-party-licenses.sh" vendor

echo "vendored: $DEST (mermaid ${MERMAID_VERSION}, $(du -h "$DEST" | cut -f1))"
echo "archive:  $ACTUAL"
echo "sha256:   $(shasum -a 256 "$DEST" | cut -d' ' -f1)"
echo "notice:   $NOTICE（$(wc -l < "$NOTICE" | tr -d ' ') 行）"
