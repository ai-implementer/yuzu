#!/usr/bin/env bash
# 公式 toml-test（https://github.com/toml-lang/toml-test、MIT）のケースを
# kabosu のテストへ取得する。
# 更新するときは TOML_TEST_VERSION と TOML_TEST_ARCHIVE_SHA256 を**セットで**
# 書き換えて実行し、生成される tests/toml-test/README.md の記録を確認すること
# （新しい sha256 はこのスクリプトの不一致エラーが実測値を表示する）。
#
# **タグはテストスイートの版であって TOML 仕様の版ではない。**
# 仕様の版で選ぶのは tests/files-toml-1.0.0（TOML 1.0.0 が対象とするケースの
# 一覧）で、ここに載っているファイルだけを取り込む = TOML 1.1 専用のケースは
# 入れない。kabosu は TOML 1.0 のパーサなので、1.1 のケースを入れると
# 「仕様どおり拒否したのに落ちる」テストになる。
set -euo pipefail

TOML_TEST_VERSION="${TOML_TEST_VERSION:-2.2.0}"
# **展開する前**にアーカイブ自体を照合する（vendor-katex.sh と同じ規律）。
# アーカイブが一致すれば中身は一意に決まるので、900 ファイルもこれで覆える
TOML_TEST_ARCHIVE_SHA256="${TOML_TEST_ARCHIVE_SHA256:-fdab2779b3902eb08030f389a5d53e95c5b49404149ac6f2eda5227a5363c232}"

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
DEST="$ROOT/crates/kabosu/tests/toml-test"
# 完成形は DEST の隣で組んでから差し替える（$TMPDIR は別ファイルシステムの
# ことがあり mv が跨げないため、staging は同一 FS に置く）
STAGING="$DEST.new"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP" "$STAGING"' EXIT

ARCHIVE="$TMP/toml-test.tar.gz"
curl -fL "https://github.com/toml-lang/toml-test/archive/refs/tags/v${TOML_TEST_VERSION}.tar.gz" -o "$ARCHIVE"

ACTUAL="$(shasum -a 256 "$ARCHIVE" | cut -d' ' -f1)"
if [ "$ACTUAL" != "$TOML_TEST_ARCHIVE_SHA256" ]; then
  echo "アーカイブの sha256 が一致しません（期待 $TOML_TEST_ARCHIVE_SHA256 / 実際 ${ACTUAL}）" >&2
  exit 1
fi

tar xzf "$ARCHIVE" -C "$TMP"
SRC="$TMP/toml-test-${TOML_TEST_VERSION}"
LIST="$SRC/tests/files-toml-1.0.0"
if [ ! -f "$LIST" ]; then
  echo "files-toml-1.0.0 が見つかりません（toml-test の構成が変わった可能性）" >&2
  exit 1
fi

rm -rf "$STAGING"
mkdir -p "$STAGING"
count=0
while IFS= read -r rel; do
  [ -n "$rel" ] || continue
  if [ ! -f "$SRC/tests/$rel" ]; then
    echo "一覧にあるファイルが見つかりません: $rel" >&2
    exit 1
  fi
  mkdir -p "$STAGING/$(dirname "$rel")"
  cp "$SRC/tests/$rel" "$STAGING/$rel"
  count=$((count + 1))
done < "$LIST"

cp "$SRC/LICENSE" "$STAGING/LICENSE"

valid=$(find "$STAGING/valid" -name '*.toml' | wc -l | tr -d ' ')
invalid=$(find "$STAGING/invalid" -name '*.toml' | wc -l | tr -d ' ')

{
  echo "# toml-test（vendor）"
  echo
  echo "公式 [toml-test](https://github.com/toml-lang/toml-test)（MIT）から"
  echo "**TOML 1.0.0 が対象とするケースだけ**を取り込んだもの。"
  echo "\`scripts/vendor-toml-test.sh\` が生成するので、手で編集しない。"
  echo
  echo "- タグ: \`v${TOML_TEST_VERSION}\`"
  echo "- アーカイブ sha256: \`${TOML_TEST_ARCHIVE_SHA256}\`"
  echo "- 取り込んだファイル: ${count} 件（valid ${valid} ケース / invalid ${invalid} ケース）"
  echo "- 選別: 上流の \`tests/files-toml-1.0.0\`"
  echo
  echo "タグはテストスイートの版であって TOML 仕様の版ではない。"
  echo "仕様の版で選んでいるのは \`files-toml-1.0.0\` のほうで、TOML 1.1 専用の"
  echo "ケースは入っていない（kabosu は TOML 1.0 のパーサなので、1.1 のケースを"
  echo "入れると「仕様どおり拒否したのに落ちる」テストになる）。"
  echo
  echo "ハーネスは \`crates/kabosu/tests/toml_test.rs\`。"
  echo "配布物には含めない（\`crates/kabosu/Cargo.toml\` の \`exclude\`）。"
} > "$STAGING/README.md"

# ここまで成功して初めて既存の同梱物を置き換える
rm -rf "$DEST"
mv "$STAGING" "$DEST"

echo "toml-test v${TOML_TEST_VERSION} を取り込みました: ${count} ファイル（valid ${valid} / invalid ${invalid}）"
echo "  -> $DEST"
