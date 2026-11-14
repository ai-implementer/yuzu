#!/usr/bin/env bash
# 運用文書とコードのコメントが指す参照先が実在するかを照合する（v0.20 Phase 93。10-04 見直しの
# 「運用文書の古い記述」の再発防止。ci.yml と verify スキルが呼ぶ）。
#
# 照合するもの:
#   1. リポジトリ内のパス（`crates/…`・`scripts/…`・`.github/…` 等をバッククォートで囲んだもの）が実在する
#   2. 「<名前> スキル」（`<名前>` スキル）の .claude/skills/<名前>/SKILL.md が実在する
#   3. 「ROADMAP「…」」「CLAUDE.md「…」」で示した節の名前・文言が、そのファイルに実在する
#
# 見ないもの: ROADMAP.md・ROADMAP-history.md（過去の記録で、今は無いパスを書いている）、
# git 管理外の docs/design/、生成物（dist・target）、利用者のプロジェクトの例（content/・dist/ 等は
# リポジトリのパスではないので 1 の対象にしない）
#
#   scripts/check-doc-refs.sh
# 終了コード: 0 = すべて実在 / 1 = 見つからない参照がある
set -euo pipefail
export LC_ALL=C.UTF-8 2>/dev/null || export LC_ALL=C

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

# 照合の対象（git 管理下のファイルだけ）
mapfile -t FILES < <(git ls-files -- \
  'Cargo.toml' 'crates/*/Cargo.toml' '.github/*.yml' '.github/workflows/*.yml' 'scripts/*.sh' \
  'CLAUDE.md' 'README.md' 'SECURITY.md' '.claude/skills/*/SKILL.md' '.devcontainer/README.md' \
  'licenses/README.md' 'crates/*/README.md' 'crates/*/assets/*/README.md' 'crates/*/assets/*/*/README.md' \
  'crates/*/src/*.rs' 'crates/*/src/**/*.rs' 'crates/*/build.rs')

# ユーザスキル（~/.claude/skills。リポジトリには無い）
USER_SKILLS=" apple-container "
# 利用者のプロジェクトに作られるパス（`yuzu new` の雛形の出力先。このリポジトリのパスではない）
USER_PROJECT_PATHS=" .github/workflows/deploy.yml "

missing=0
report() {
  echo "$1: $2" >&2
  missing=$((missing + 1))
}

for file in "${FILES[@]}"; do
  # 1. リポジトリ内のパス。行番号（:12）・アンカー（#…）は外す。プレースホルダー・glob は飛ばす
  while IFS= read -r ref; do
    path="${ref%%:*}"
    path="${path%%#*}"
    # シェルの文字列の中のエスケープした \` の \ を外す
    path="${path%\\}"
    [[ "$USER_PROJECT_PATHS" == *" $path "* ]] && continue
    case "$path" in
      *'<'* | *'*'* | *'{'* | *'…'* | *'$'*) continue ;;
      docs/design/* | docs/dist/* | */dist/* | target/*) continue ;;
    esac
    [ -e "$path" ] || report "$file" "パス \`$path\` がありません"
  done < <(grep -oE '`(crates|scripts|\.github|\.claude|\.devcontainer|licenses|docs)/[^` ]+`' "$file" | tr -d '`' | sort -u)

  # 2. スキル
  while IFS= read -r name; do
    [[ "$USER_SKILLS" == *" $name "* ]] && continue
    [ -f ".claude/skills/$name/SKILL.md" ] || report "$file" "スキル \`$name\` がありません"
  done < <(grep -oE '`[a-z][a-z0-9-]*` ?スキル|\*\*`[a-z][a-z0-9-]*` ?スキル' "$file" \
    | grep -oE '`[a-z][a-z0-9-]*`' | tr -d '`' | sort -u)

  # 3. ROADMAP「…」・CLAUDE.md「…」の節の名前・文言
  while IFS= read -r ref; do
    target="${ref%%「*}"
    text="${ref#*「}"
    text="${text%」}"
    case "$target" in
      ROADMAP | ROADMAP.md) target=ROADMAP.md ;;
      ROADMAP-history.md) ;;
      CLAUDE.md) ;;
      *) continue ;;
    esac
    grep -qF -- "$text" "$target" || report "$file" "$target に「$text」がありません"
  done < <(grep -oE '(ROADMAP(-history)?(\.md)?|CLAUDE\.md)「[^」]+」' "$file" | sort -u)
done

if [ "$missing" -gt 0 ]; then
  echo "見つからない参照が $missing 件あります（参照先を直すか、参照を書き換えてください）" >&2
  exit 1
fi
echo "参照先はすべて実在します（${#FILES[@]} ファイル）"
