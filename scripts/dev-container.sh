#!/usr/bin/env bash
# yuzu の開発コンテナを apple container / docker のどちらでも同じ体験で扱うラッパー。
# 環境の定義は .devcontainer/Dockerfile が唯一（このスクリプトは配線のみ）。
#
# 使い方:
#   scripts/dev-container.sh build   # イメージをビルド（--no-cache 可）
#   scripts/dev-container.sh up      # 長寿命コンテナを起動（キャッシュ volume 付き）
#   scripts/dev-container.sh shell   # コンテナ内の bash に入る
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
# 不変条件: volume 名とマウント先は .devcontainer/devcontainer.json と一致させること
VOLUMES=(
  "yuzu-cargo-registry:/home/vscode/.cargo/registry"
  "yuzu-target:/cargo-target"
  "yuzu-claude:/home/vscode/.claude"
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
  "$ENGINE" build "$@" -t "$IMAGE" -f "$ROOT/.devcontainer/Dockerfile" "$ROOT/.devcontainer"
}

cmd_up() {
  ensure_engine_running
  ensure_volumes

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

  local args=(-d --name "$NAME" -v "$ROOT:/workspaces/yuzu")
  local spec
  for spec in "${VOLUMES[@]}"; do
    args+=(-v "$spec")
  done
  args+=(-p "127.0.0.1:5173:5173")
  # エンジン差分 2/3: apple container はコンテナ = 軽量 VM で既定リソースが小さく、
  # rustc の並列ビルドでメモリ不足になり得るため明示する
  if [ "$ENGINE" = "container" ]; then
    args+=(--memory "${YUZU_CONTAINER_MEMORY:-8g}")
    args+=(--cpus "${YUZU_CONTAINER_CPUS:-$(sysctl -n hw.ncpu)}")
  fi

  "$ENGINE" run "${args[@]}" "$IMAGE"
  # 共通フック（volume 所有権の正規化・Claude Code 導入）。devcontainer 経路の
  # postCreateCommand と同一スクリプトを使う
  "$ENGINE" exec "$NAME" bash /workspaces/yuzu/.devcontainer/post-create.sh
  echo "起動しました: ${NAME}（scripts/dev-container.sh shell で入れます）"
}

cmd_shell() {
  if ! container_exists; then
    echo "コンテナが見つかりません。up から起動します..."
    cmd_up
  elif ! container_alive; then
    revive_or_remove || cmd_up
  fi
  exec "$ENGINE" exec -it "$NAME" bash
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
  shell) cmd_shell ;;
  down) cmd_down ;;
  clean) cmd_clean ;;
  status) cmd_status ;;
  *)
    sed -n '2,17p' "$0" | sed 's/^# \{0,1\}//'
    exit 2
    ;;
esac
