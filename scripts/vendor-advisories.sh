#!/usr/bin/env bash
# 同梱している vendor 資産（mermaid・mermaid が束ねた npm パッケージ・KaTeX）に、
# GitHub の advisory database で既知の勧告が出ていないかを照合する（Phase 82。
# deps.yml が週次で呼ぶ。dependabot は vendor したファイルを見ないため）。
#
# 照合する版は、vendor スクリプトが書いたライセンスの記録から読む（版の一覧を別に持たない）:
#   crates/yuzu-theme/licenses/mermaid.txt … 1 行目の「npm の mermaid@x.y.z」と、束ねた
#                                            パッケージの行「  - <名前> <版>（<ライセンス>）」
#   crates/yuzu-theme/licenses/katex.txt   … 1 行目の「npm の katex@x.y.z」
# mermaid のモノレポ内のパッケージ（@mermaid-js/parser）は版の記録が無いので照合しない。
#
#   scripts/vendor-advisories.sh [<パッケージ>@<版> ...]
#     引数は追加で照合するパッケージ（検出できることの確認用。例: katex@0.16.0）
#
# 前提: gh（認証済み。CI では GH_TOKEN）と jq。
# 終了コード: 0 = 勧告なし / 1 = 勧告あり / 2 = 実行エラー（記録が読めない・照会できない）
set -euo pipefail
export LC_ALL=C

# 影響しないと判断した勧告。「GHSA の ID と <パッケージ>@<版>」の組で書く（版が変われば
# 除外は外れ、改めて照合される）。足すときは、影響しない理由と見直す条件をコメントに書く
IGNORE=(
  # --- mermaid 11.17.2 が束ねているパッケージ（10-10 に確認。11 系は 11.17.2 が最新で、
  # 11 系の更新では直せない。mermaid 12 へ上げるとき（v0.20 の候補）にすべて見直す）---
  # DOMPurify の IN_PLACE モードの不具合。束ねたコードで mermaid は IN_PLACE を渡していない
  # （設定の既定値の読み取り 1 か所だけ）
  "GHSA-55q2-fjhq-7xh7 dompurify@3.4.12"
  "GHSA-6688-9rhm-gjv2 dompurify@3.4.12"
  # js-yaml のマージキー・!!omap による DoS。mermaid の読み込み 4 か所はすべて JSON_SCHEMA で、
  # どちらの型も解釈しない。入力は原稿の図のソース（SECURITY.md で信頼する前提のもの）
  "GHSA-2883-xcg3-v3hh js-yaml@4.3.0"
  "GHSA-5p4m-2wfm-xmqj js-yaml@4.3.0"
  # mermaid 内の KaTeX。ページに既存のプロトタイプ汚染があることが前提（low）で、0.16 系に
  # 修正版が無い。yuzu が同梱する KaTeX 0.18.11 は対象外
  "GHSA-238p-pmpm-9mq7 katex@0.16.47"
)

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
LIC="$ROOT/crates/yuzu-theme/licenses"

fail() {
  echo "error: $*" >&2
  exit 2
}

command -v gh >/dev/null || fail "gh が見つかりません"
command -v jq >/dev/null || fail "jq が見つかりません"

# 記録の 1 行目「… npm の <パッケージ>@<版>）」から照合対象を取り出す
top_package() {
  local got
  got="$(sed -n '1s/.*npm の \([^）]*\)）.*/\1/p' "$1")"
  if ! [[ "$got" =~ ^[@a-z0-9._/-]+@[0-9][0-9A-Za-z.+-]*$ ]]; then
    fail "$1 の 1 行目から npm の版を読めません: $(head -1 "$1")"
  fi
  echo "$got"
}

mermaid="$(top_package "$LIC/mermaid.txt")"
katex="$(top_package "$LIC/katex.txt")"
packages=("$mermaid" "$katex")

# 束ねたパッケージ。記録の見出しにある個数と、読み取れた行の数が合わなければ形式が
# 変わったとみなして止める（黙って一部だけを照合しない）
bundled="$(sed -n 's/^  - \([^ ]*\) \([0-9][^（ ]*\)（.*/\1@\2/p' "$LIC/mermaid.txt")"
expected="$(grep -o '[0-9][0-9]* 個）' "$LIC/mermaid.txt" | head -1 | grep -o '[0-9]*' || true)"
got_count="$(printf '%s\n' "$bundled" | grep -c . || true)"
if [ -z "$expected" ] || [ "$got_count" != "$expected" ]; then
  fail "mermaid.txt の束ねたパッケージを読めません（見出しの個数: ${expected:-なし} / 読み取れた行: $got_count）"
fi
while IFS= read -r p; do
  packages+=("$p")
done <<<"$bundled"

for p in "$@"; do
  packages+=("$p")
done

affects="$(IFS=,; echo "${packages[*]}")"
pages="$(gh api -X GET /advisories -f ecosystem=npm -f "affects=$affects" -f per_page=100 --paginate)" \
  || fail "GitHub の advisory database を照会できません"

ours_json="$(printf '%s\n' "${packages[@]}" | jq -R . | jq -s .)"
ignore_json="$(printf '%s\n' ${IGNORE[@]+"${IGNORE[@]}"} | jq -R . | jq -s 'map(select(. != ""))')"

# 取り下げ済みの勧告と除外した組を除き、勧告ごとに「どの同梱パッケージのどの版が当たったか」を
# 出す。照会は「いずれかのパッケージが影響を受ける勧告」を返すので、同名で複数の版を持つ
# パッケージ（同梱の katex と mermaid が束ねた katex など）は版の範囲で絞る。範囲の条件を
# 読めないときは当たったとみなし、どの版にも絞れなかった勧告は同名の版をすべて出す
# （照会が返した勧告を黙って落とさない）
report="$(printf '%s\n' "$pages" | jq -rs --argjson ours "$ours_json" --argjson ignore "$ignore_json" '
  def name: split("@") | .[:-1] | join("@");
  def ver: split("-")[0] | split("+")[0] | split(".") | map(tonumber? // 0) | (. + [0, 0, 0])[:3];
  # $p（name@版）が範囲（例 ">= 0.11.0, < 0.18.2"）に入るか
  def affected($p; $range):
    ($p | split("@") | last | ver) as $v
    | $range | split(",") | map(gsub("^ +| +$"; ""))
    | all(.[];
        (capture("^(?<op>[<>=]+) *(?<x>.+)$")? // {op: "?", x: "0"}) as $c
        | ($c.x | ver) as $x
        | if $c.op == ">=" then $v >= $x
          elif $c.op == ">" then $v > $x
          elif $c.op == "<=" then $v <= $x
          elif $c.op == "<" then $v < $x
          elif $c.op == "=" then $v == $x
          else true end);
  def ignored($id): ("\($id) \(.)" | IN($ignore[]));
  (reduce $ours[] as $p ({}; .[$p | name] += [$p])) as $by_name
  | add // []
  | .[]
  | select(.withdrawn_at == null)
  | . as $adv
  | [.vulnerabilities[]
      | . as $vuln
      | [($by_name[.package.name] // [])[] | select(affected(.; $vuln.vulnerable_version_range))]
      | select(. != [])
      | {vuln: $vuln, hits: .}] as $rows
  | (if $rows == [] then
       # 照会は該当と返したのに版の範囲で絞れなかった（範囲の書き方が想定外など）
       [[.vulnerabilities[] | ($by_name[.package.name] // [])[] | select(ignored($adv.ghsa_id) | not)]
        | unique | select(. != [])
        | "  対象（版の範囲で絞れなかった）: \(join(", "))"]
     else
       [$rows[]
        | [.hits[] | select(ignored($adv.ghsa_id) | not)] as $kept
        | select($kept != [])
        | "  対象: \($kept | join(", "))（影響: \(.vuln.vulnerable_version_range) / 修正: \(.vuln.first_patched_version // "なし")）"]
     end) as $lines
  | select($lines != [])
  | "\($adv.ghsa_id)（\($adv.severity)）\($adv.summary)", $lines[], "  \($adv.html_url)"
')" || fail "照会の結果を読めません"

if [ -n "$report" ]; then
  echo "vendor 資産に既知の勧告があります（照合したパッケージ: ${#packages[@]}）:" >&2
  printf '%s\n' "$report" >&2
  echo "更新するなら vendor-update スキル、影響しないなら理由を書いて IGNORE に足す" >&2
  exit 1
fi
echo "vendor 資産 ${#packages[@]} パッケージに既知の勧告はありません（$mermaid・$katex ほか束ねた $got_count）"
