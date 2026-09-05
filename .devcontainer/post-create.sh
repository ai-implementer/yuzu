#!/usr/bin/env bash
# devcontainer（postCreateCommand）と scripts/dev-container.sh up の両経路から
# 呼ばれる共通フック。何度実行しても安全（冪等）。
set -euo pipefail

# named volume のマウント点は、エンジンによっては fresh 作成時に root 所有に
# なることがあるため開発ユーザに正規化する（このために sudo を入れている）。
# ラッパー経路の bind mount（~/.claude 等）は書き込み可のまま来るので no-op で素通りする
for dir in "$HOME/.cargo/registry" /cargo-target \
           "$HOME/.claude" "$HOME/.codex" "$HOME/.config" "$HOME/.config/gh"; do
  if [ -d "$dir" ] && [ ! -w "$dir" ]; then
    sudo chown "$(id -u):$(id -g)" "$dir"
  fi
done

# Claude Code はイメージ焼き込み済み（Dockerfile 末尾）。ここは焼き込み前の
# イメージや導入失敗時の fallback として残す（冪等）
if ! command -v claude >/dev/null 2>&1; then
  echo "Claude Code をインストールします（~/.local/bin）..."
  curl -fsSL https://claude.ai/install.sh | bash
fi

echo "準備完了: $(rustc --version)"
echo "  cargo-insta: $(cargo insta --version 2>/dev/null || echo 未導入)"
echo "  claude: $(claude --version 2>/dev/null || echo '未導入（初回は claude 実行で認証）')"
echo "  codex: $(codex --version 2>/dev/null || echo 未導入)"
echo "  gh: $(gh --version 2>/dev/null | head -1 || echo 未導入)"
