#!/usr/bin/env bash
# yuzu の開発コンテナを apple container / docker のどちらでも同じ体験で扱うラッパー。
# 環境の定義は .devcontainer/Dockerfile が唯一（このスクリプトは配線のみ）。
#
# この経路は「ホスト同一パス」構成: ユーザ名・uid・HOME・リポジトリパスをホストと
# 一致させてイメージを焼き、~/.claude / ~/.codex / ~/.config/gh を同一パスへ bind mount
# する（skills の絶対 symlink・hooks・codex の projects トラストが無傷で動く）。
# devcontainer.json の Docker 経路は ARG 既定値（vscode/1000）のまま。詳細は
# .devcontainer/README.md「認証の仕組み」。
#
# 使い方:
#   scripts/dev-container.sh build   # イメージをビルド（--no-cache 可）
#   scripts/dev-container.sh up      # 長寿命コンテナを起動（キャッシュ volume 付き）
#   scripts/dev-container.sh shell   # コンテナ内の bash に入る（shell -c '...' で単発実行）
#   scripts/dev-container.sh claude  # コンテナ内で Claude Code を起動
#   scripts/dev-container.sh codex   # コンテナ内で Codex CLI を起動
#   scripts/dev-container.sh down    # コンテナを停止・削除（volume は保持）
#   scripts/dev-container.sh clean   # down ＋ キャッシュ volume も削除
#   scripts/dev-container.sh status  # 状態表示
#
# 環境変数:
#   YUZU_CONTAINER_ENGINE   使用エンジン（既定: Darwin は container、他は docker）
#   YUZU_CONTAINER_MEMORY   apple container の VM メモリ（既定: 8g）
#   YUZU_CONTAINER_CPUS     apple container の VM CPU 数（既定: ホスト CPU 数）
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"

IMAGE="${YUZU_CONTAINER_IMAGE:-yuzu-dev:latest}"
NAME="${YUZU_CONTAINER_NAME:-yuzu-dev}"

# ホスト同一パス化のための build ARG 値。gid はホスト mac の staff=20 が Debian の
# 既存 gid と衝突し得るため uid と同値にする（virtiofs が所有権をコンテナユーザへ
# マッピングするので実害なし。ホスト側では従来どおり uid:staff で見える）
DEV_USER="$(id -un)"
DEV_UID="$(id -u)"
DEV_GID="$DEV_UID"

# 不変条件: volume 名は .devcontainer/devcontainer.json と一致させること
# （マウント先はどちらも「$DEV_HOME/.cargo/registry」と「/cargo-target」。
# ~/.claude / ~/.codex / ~/.config/gh はこの経路では volume ではなくホストの
# 実ディレクトリを bind mount する — devcontainer 経路は volume で各自ログイン）
VOLUMES=(
  "yuzu-cargo-registry:$HOME/.cargo/registry"
  "yuzu-target:/cargo-target"
)

if [ -n "${YUZU_CONTAINER_ENGINE:-}" ]; then
  ENGINE="$YUZU_CONTAINER_ENGINE"
elif [ "$(uname -s)" = "Darwin" ]; then
  ENGINE="container"
else
  ENGINE="docker"
fi

if ! command -v "$ENGINE" >/dev/null 2>&1; then
  echo "error: コンテナエンジン '$ENGINE' が見つかりません" >&2
  echo "  YUZU_CONTAINER_ENGINE で明示指定するか、apple container / docker を導入してください" >&2
  exit 2
fi

# apple container はサービス（API サーバ）の起動が前提。冪等に確認する。
# --enable-kernel-install: 初回は既定カーネル（kata）の導入プロンプトが出るため、
# 非対話でも通るように明示する
ensure_engine_running() {
  if [ "$ENGINE" = "container" ]; then
    if ! container system status >/dev/null 2>&1; then
      echo "container services を起動します..."
      container system start --enable-kernel-install
    fi
  fi
}

# エンジン差分 1/3: named volume の事前作成（docker は暗黙作成されるが挙動を揃える）
ensure_volumes() {
  local spec name
  for spec in "${VOLUMES[@]}"; do
    name="${spec%%:*}"
    if ! "$ENGINE" volume inspect "$name" >/dev/null 2>&1; then
      "$ENGINE" volume create "$name" >/dev/null
    fi
  done
}

# gh のトークンは macOS Keychain 保存でファイルに無いため、ホストで取り出して
# 値なしの -e GH_TOKEN（ホスト環境から継承）で注入する — argv（ps で見える）に値を出さない。
# git identity も ~/.gitconfig をマウントしない（lfs filter 事故回避）ため同じ形で渡す。
# 空の値は注入しない（空の GIT_AUTHOR_NAME は git がエラーにする）
export_host_env() {
  GH_TOKEN=$(gh auth token 2>/dev/null || true)
  GIT_AUTHOR_NAME=$(git config user.name 2>/dev/null || true)
  GIT_AUTHOR_EMAIL=$(git config user.email 2>/dev/null || true)
  GIT_COMMITTER_NAME="$GIT_AUTHOR_NAME"
  GIT_COMMITTER_EMAIL="$GIT_AUTHOR_EMAIL"
  export GH_TOKEN GIT_AUTHOR_NAME GIT_AUTHOR_EMAIL GIT_COMMITTER_NAME GIT_COMMITTER_EMAIL
  GIT_ENV_FLAGS=()
  HOST_ENV_FLAGS=()
  local key
  for key in GIT_AUTHOR_NAME GIT_AUTHOR_EMAIL GIT_COMMITTER_NAME GIT_COMMITTER_EMAIL; do
    if [ -n "${!key}" ]; then
      GIT_ENV_FLAGS+=(-e "$key")
    fi
  done
  HOST_ENV_FLAGS=(${GIT_ENV_FLAGS[@]+"${GIT_ENV_FLAGS[@]}"})
  if [ -n "$GH_TOKEN" ]; then
    HOST_ENV_FLAGS+=(-e GH_TOKEN)
  else
    echo "warn: gh auth token が取得できませんでした（gh は未認証になります）" >&2
  fi
}

container_exists() {
  "$ENGINE" inspect "$NAME" >/dev/null 2>&1
}

container_alive() {
  "$ENGINE" exec "$NAME" true >/dev/null 2>&1
}

# 停止中なら start、それも不可なら削除して false を返す（up が作り直す）
revive_or_remove() {
  if "$ENGINE" start "$NAME" >/dev/null 2>&1 && container_alive; then
    return 0
  fi
  "$ENGINE" rm "$NAME" >/dev/null 2>&1 || true
  return 1
}

cmd_build() {
  ensure_engine_running
  "$ENGINE" build "$@" \
    --build-arg "DEV_USER=$DEV_USER" \
    --build-arg "DEV_UID=$DEV_UID" \
    --build-arg "DEV_GID=$DEV_GID" \
    --build-arg "DEV_HOME=$HOME" \
    --build-arg "DEV_WORKSPACE=$ROOT" \
    -t "$IMAGE" -f "$ROOT/.devcontainer/Dockerfile" "$ROOT/.devcontainer"
}

cmd_up() {
  ensure_engine_running
  ensure_volumes
  export_host_env

  if container_exists; then
    if container_alive; then
      echo "既に起動しています: ${NAME}（shell で入れます）"
      return 0
    fi
    # エンジン差分 3/3: 停止コンテナの再開。start で戻せれば volume 未接続の
    # 作り直しを避けられる（不可なら削除して下で作り直す）
    if revive_or_remove; then
      echo "停止中のコンテナを再開しました: $NAME"
      return 0
    fi
  fi

  local args=(-d --name "$NAME" -v "$ROOT:$ROOT")
  local spec
  for spec in "${VOLUMES[@]}"; do
    args+=(-v "$spec")
  done
  # ホスト設定の共有（同一パス bind mount）。無いものはスキップする
  # （存在しないパスを bind mount すると root 所有の空ディレクトリが作られる事故を防ぐ）
  local dir
  for dir in "$HOME/.claude" "$HOME/.codex" "$HOME/.config/gh"; do
    if [ -d "$dir" ]; then
      args+=(-v "$dir:$dir")
    else
      echo "warn: $dir が無いためマウントしません" >&2
    fi
  done
  # ~/.claude/skills が symlink なら実体（dotfiles 等）も同一パスでマウントして symlink を生かす
  if [ -L "$HOME/.claude/skills" ]; then
    local skills
    skills="$(readlink -f "$HOME/.claude/skills" || true)"
    if [ -d "$skills" ]; then
      args+=(-v "$skills:$skills")
    fi
  fi
  # git identity は run 時にも焼いておく（ラッパーを介さない素の `container exec` 用。
  # ラッパーの shell/claude/codex は exec 時にも注入する。GH_TOKEN は inspect の env に
  # 残さないよう run には焼かず exec 時のみ）
  args+=(${GIT_ENV_FLAGS[@]+"${GIT_ENV_FLAGS[@]}"})
  args+=(-p "127.0.0.1:5173:5173")
  # エンジン差分 2/3: リソースと SSH agent フォワードの渡し方。
  # - apple container はコンテナ = 軽量 VM で既定リソースが小さく、rustc の並列ビルドで
  #   メモリ不足になり得るため明示する。SSH は専用の --ssh フラグ（秘密鍵 ~/.ssh は
  #   マウントせず agent フォワードで git push/fetch を通す）
  # - docker の run に --ssh は無いため、ホストの agent ソケットを同一パスで
  #   bind mount して代替する（無ければスキップ = git は https か手動設定で）
  if [ "$ENGINE" = "container" ]; then
    args+=(--ssh)
    args+=(--memory "${YUZU_CONTAINER_MEMORY:-8g}")
    args+=(--cpus "${YUZU_CONTAINER_CPUS:-$(sysctl -n hw.ncpu)}")
  elif [ -n "${SSH_AUTH_SOCK:-}" ] && [ -S "$SSH_AUTH_SOCK" ]; then
    args+=(-v "$SSH_AUTH_SOCK:$SSH_AUTH_SOCK" -e SSH_AUTH_SOCK)
  fi

  "$ENGINE" run "${args[@]}" "$IMAGE"
  # 共通フック（volume 所有権の正規化・Claude Code 導入の fallback）。devcontainer 経路の
  # postCreateCommand と同一スクリプトを使う
  "$ENGINE" exec "$NAME" bash "$ROOT/.devcontainer/post-create.sh"
  echo "起動しました: ${NAME}（scripts/dev-container.sh shell で入れます）"
}

ensure_up() {
  if ! container_exists; then
    echo "コンテナが見つかりません。up から起動します..."
    cmd_up
  elif ! container_alive; then
    revive_or_remove || cmd_up
  fi
}

# shell / claude / codex の共通形: exec のたびに GH_TOKEN 等を取り直して注入する
# （長寿命コンテナでもトークンが古くならない）。
# stdin が TTY なら -i、stdout も TTY のときだけ -t を付ける（TTY なしの -t は
# exec が「Operation not supported on socket」で失敗する。パイプ時の色・CRLF 混入も防ぐ）
# ※ macOS 標準の bash 3.2 は set -u で空配列展開がエラーになるため展開側で +"" を使う
cmd_exec_interactive() {
  ensure_up
  export_host_env
  local tty_flags=()
  if [ -t 0 ]; then
    tty_flags+=(-i)
    if [ -t 1 ]; then
      tty_flags+=(-t)
    fi
  fi
  exec "$ENGINE" exec ${tty_flags[@]+"${tty_flags[@]}"} \
    ${HOST_ENV_FLAGS[@]+"${HOST_ENV_FLAGS[@]}"} "$NAME" "$@"
}

cmd_down() {
  "$ENGINE" stop "$NAME" >/dev/null 2>&1 || true
  "$ENGINE" rm "$NAME" >/dev/null 2>&1 || true
  echo "停止・削除しました: ${NAME}（キャッシュ volume は保持）"
}

cmd_clean() {
  cmd_down
  local spec name
  for spec in "${VOLUMES[@]}"; do
    name="${spec%%:*}"
    "$ENGINE" volume rm "$name" >/dev/null 2>&1 || true
  done
  # 旧構成（~/.claude を volume にしていた頃）の残骸も掃除する
  "$ENGINE" volume rm yuzu-claude >/dev/null 2>&1 || true
  echo "キャッシュ volume も削除しました"
}

cmd_status() {
  echo "engine: $ENGINE"
  if container_exists; then
    "$ENGINE" ls | awk -v name="$NAME" 'NR==1 || index($0, name)'
  else
    echo "container: なし（up で起動）"
  fi
  "$ENGINE" volume ls 2>/dev/null | awk 'NR==1 || /yuzu-/'
}

case "${1:-}" in
  build) shift; cmd_build "$@" ;;
  up) cmd_up ;;
  shell) shift; cmd_exec_interactive bash "$@" ;;
  claude) shift; cmd_exec_interactive claude "$@" ;;
  codex) shift; cmd_exec_interactive codex "$@" ;;
  down) cmd_down ;;
  clean) cmd_clean ;;
  status) cmd_status ;;
  *)
    sed -n '2,24p' "$0" | sed 's/^# \{0,1\}//'
    exit 2
    ;;
esac
