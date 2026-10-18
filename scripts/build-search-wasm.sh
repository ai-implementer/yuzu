#!/usr/bin/env bash
# mikan-wasm をビルドして yuzu-index の assets へ vendor する。
#
# 前提:
#   rustup target add wasm32-unknown-unknown
#   cargo install wasm-bindgen-cli --version <crates/mikan-wasm の wasm-bindgen と同一>
#   binaryen（wasm-opt）が PATH にあること（例: brew install binaryen）
#
#   cargo-about（wasm に入る crate のライセンス文を作る。版は scripts/third-party-licenses.sh の
#   ABOUT_VERSION と同一）
#
# 実行後、crates/yuzu-index/assets/search/README.md にサイズを記録すること。
# wasm と一緒に、dist の _search/ へ出るライセンス文（THIRD-PARTY-LICENSES.txt）も作り直す
# （wasm を変えずに文だけ作り直すなら scripts/third-party-licenses.sh wasm）。
#
#   scripts/build-search-wasm.sh --check
#     wasm の作り直し忘れを照合する（CI が呼ぶ。ビルドのツールは要らない）。wasm の入力の
#     ハッシュ（wasm_inputs_hash）がこのスクリプトの記録（wasm-inputs.sha256）と同じか、
#     手書きの JS が vendor 先とバイト単位で同じかを見る。CI の検索の照合はネイティブの
#     `yuzu search` を通り、third-party-licenses.sh check は crate と版の集合しか比べないので、
#     版を上げない mikan の変更で wasm が古いままでも通ってしまうため（Phase 89）。
#     wasm 自体をバイトで比べないのは、ビルドした環境（rustc・wasm-opt の版）で変わるため
set -euo pipefail
# 並び順（find・sort）を呼び出し元のロケールに左右させない（ハッシュが環境で変わらないように）
export LC_ALL=C

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
DEST="$ROOT/crates/yuzu-index/assets/search"
RECORD="$DEST/wasm-inputs.sha256"

# 検索 wasm の入力のハッシュ。記録を書くのはこのスクリプトだけ（記録だけを更新する手段は
# 作らない = 作り直し忘れの抜け道になる）。入れないもの: Cargo.lock の全体とルート Cargo.toml の
# 全体（wasm に関係しない依存の更新や版上げでも落ちる）・mikan/assets（分かち書きのモデルは
# wasm では埋め込まない）・rustc と wasm-opt の版（stable で固定していない）
wasm_inputs_hash() {
  {
    # mikan・mikan-wasm のソースと manifest（ファイル名も入れる = 追加・削除・改名も変化になる）
    for c in mikan mikan-wasm; do
      (cd "$ROOT/crates/$c" && find src Cargo.toml -type f | sort | while read -r f; do
        printf '== %s/%s\n' "$c" "$f"
        cat "$f"
      done)
    done
    # wasm32 向けの依存の木と有効な features（wasm-bindgen の版の違いもここに出る）。
    # path 依存の行は絶対パスと版（ワークスペースの版・公開前の版上げ）を外して名前と features だけ
    echo "== cargo tree"
    cargo tree --color never --manifest-path "$ROOT/Cargo.toml" --locked -p mikan-wasm \
        --target wasm32-unknown-unknown -e normal --prefix none --format '{p} [{f}]' \
      | sed 's/ (\*)$//; s/ v[0-9][^ ]* (\/[^)]*)/ (path)/' | sort -u
    # ルート Cargo.toml のうち wasm のビルドに効くプロファイル
    echo "== profiles"
    awk '/^\[profile\.(release|wasm-release)\]$/ { p = 1; print; next } /^\[/ { p = 0 } p' \
      "$ROOT/Cargo.toml"
    # このスクリプト自身（ビルドの手順とフラグ）
    echo "== script"
    cat "$ROOT/scripts/build-search-wasm.sh"
  } | if command -v sha256sum >/dev/null; then sha256sum; else shasum -a 256; fi | cut -d' ' -f1
}

if [ "${1:-}" = "--check" ]; then
  status=0
  if [ ! -f "$RECORD" ] || [ "$(wasm_inputs_hash)" != "$(cat "$RECORD")" ]; then
    echo "検索 wasm の入力（mikan・mikan-wasm のソース、wasm32 向けの依存の木、wasm のプロファイル、" >&2
    echo "scripts/build-search-wasm.sh）が、コミットした wasm を作ったときと違います。" >&2
    echo "scripts/build-search-wasm.sh で wasm を作り直してください（vendor-update スキル）" >&2
    status=1
  fi
  for js in "$ROOT/crates/mikan-wasm/js/"*.js; do
    if ! cmp -s "$js" "$DEST/$(basename "$js")"; then
      echo "crates/mikan-wasm/js/$(basename "$js") が vendor 先（assets/search/）と違います。" >&2
      echo "scripts/build-search-wasm.sh で作り直してください" >&2
      status=1
    fi
  done
  [ "$status" -eq 0 ] && echo "検索 wasm と同梱の JS は今の入力から作られています"
  exit "$status"
fi

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

# ビルドした人の $CARGO_HOME（~/.cargo/registry の絶対パス。依存 crate の panic の位置などに
# 入る）を wasm に残さない。wasm はすべての利用者のサイトの _search/ に配られる（Phase 89。
# 以前は /Users/<名前>/.cargo/registry が 45 か所入っていた）。ワークスペースの crate は相対パス
CARGO_HOME_DIR="${CARGO_HOME:-$HOME/.cargo}"
RUSTFLAGS="${RUSTFLAGS:-} --remap-path-prefix=$CARGO_HOME_DIR=/cargo" \
  cargo build --locked -p mikan-wasm --profile wasm-release --target wasm32-unknown-unknown

# 開発コンテナは CARGO_TARGET_DIR=/cargo-target（.devcontainer）なので、成果物の場所はそれに従う
TARGET_DIR="${CARGO_TARGET_DIR:-$ROOT/target}"
wasm-bindgen --target web --no-typescript --out-name search --out-dir "$DEST" \
  "$TARGET_DIR/wasm32-unknown-unknown/wasm-release/mikan_wasm.wasm"

wasm-opt -Oz --strip-debug -o "$DEST/search_bg.wasm" "$DEST/search_bg.wasm"

# 手書きの JS クライアント（フェッチ ＋ OPFS キャッシュ ＋ wasm 起動）は
# crates/mikan-wasm/js/ に同居しており、生成物と同じ vendor 先へコピーする
cp "$ROOT/crates/mikan-wasm/js/"*.js "$DEST/"

"$ROOT/scripts/third-party-licenses.sh" wasm

# 作り直し忘れの照合（--check）の記録。作り終えた後に書く（途中で失敗したら古い記録のまま
# = CI が作り直しを求め続ける）
wasm_inputs_hash > "$RECORD"

echo "vendored:"
ls -lh "$DEST"/search.js "$DEST"/search_bg.wasm "$DEST"/search-client.js "$DEST"/opfs-cache.js \
  | awk '{print "  " $9 ": " $5}'
