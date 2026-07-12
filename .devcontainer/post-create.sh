#!/usr/bin/env bash
# devcontainer（postCreateCommand）と scripts/dev-container.sh up の両経路から
# 呼ばれる共通フック。何度実行しても安全（冪等）。
set -euo pipefail

# named volume のマウント点は、エンジンによっては fresh 作成時に root 所有に
# なることがあるため vscode に正規化する（このために sudo を入れている）
for dir in "$HOME/.cargo/registry" /cargo-target "$HOME/.claude"; do
  if [ -d "$dir" ] && [ ! -w "$dir" ]; then
    sudo chown "$(id -u):$(id -g)" "$dir"
  fi
done

# Claude Code はイメージに焼かず、ユーザ領域（~/.local/bin）へ導入する。
# 設定・認証は CLAUDE_CONFIG_DIR（= yuzu-claude volume）で永続化される
if ! command -v claude >/dev/null 2>&1; then
  echo "Claude Code をインストールします（~/.local/bin）..."
  curl -fsSL https://claude.ai/install.sh | bash
fi

echo "準備完了: $(rustc --version)"
echo "  cargo-insta: $(cargo insta --version 2>/dev/null || echo 未導入)"
echo "  claude: $(claude --version 2>/dev/null || echo '未導入（初回は claude 実行で認証）')"
