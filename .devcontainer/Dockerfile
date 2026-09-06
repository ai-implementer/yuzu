# yuzu 開発環境の唯一の定義。
# devcontainer.json（Docker 系）と scripts/dev-container.sh（apple container / docker）の
# 両経路がこのファイルを参照する。環境の実体（env・ユーザ・作業ディレクトリ・常駐コマンド）は
# すべてここに集約し、各経路では再定義しない（経路間の食い違い防止）。
#
# 方針:
# - CI 相当の軽量構成（Rust stable + rustfmt/clippy + wasm32 target + cargo-insta）に加え、
#   エージェント実行環境（Claude Code / Codex CLI / gh）を焼き込む。
#   vendor 更新ツール（wasm-bindgen-cli / binaryen）は焼かない（頻度が低く重いため）
# - ユーザ・HOME・作業ディレクトリは ARG でパラメタ化する。既定値 = Docker 経路
#   （devcontainer.json。vscode/1000 + /workspaces/yuzu）。apple container 経路は
#   scripts/dev-container.sh がホスト値（ユーザ名・uid・$HOME・リポジトリ実パス）を渡し、
#   ホストと同一パスの環境を作る（~/.claude / ~/.codex / ~/.config/gh を同一パスへ
#   bind mount して設定・認証を共有するため。詳細は README.md「認証の仕組み」）
# - toolchain は「stable」という名前でインストールする — rust-toolchain.toml の
#   channel = "stable" と一致させ、初回 cargo 実行時の二重ダウンロードを防ぐ。
#   このため rust:slim / devcontainers/rust（バージョン固定 toolchain 入り）は使わない
# - stable を追従させたいときは scripts/dev-container.sh build --no-cache で焼き直す
FROM debian:bookworm-slim

ARG DEV_USER=vscode
ARG DEV_UID=1000
ARG DEV_GID=1000
ARG DEV_HOME=/home/vscode
ARG DEV_WORKSPACE=/workspaces/yuzu

# gcc/libc6-dev: rustc のリンカ用（ネイティブ C 依存 crate は無いので g++ は不要）
# curl/xz-utils: scripts/*.sh（vendor 系）と各種インストーラ用
# sudo: post-create.sh が named volume マウント点の所有権を直すため（DEV_USER に NOPASSWD）
# openssh-client: `container run --ssh` の agent フォワードでの git push/fetch 用
# jq: JSON の整形・取り出し（gh api の結果確認、scripts/*.sh やエージェントの作業用）
RUN apt-get update && apt-get install -y --no-install-recommends \
        ca-certificates curl git gcc jq libc6-dev make openssh-client procps sudo xz-utils \
    && rm -rf /var/lib/apt/lists/*

# GitHub の公式 SSH ホスト鍵（https://api.github.com/meta の ssh_keys）を焼き込み。
# container run --ssh の agent フォワードと合わせ、初回から非対話で fetch/push できる
RUN mkdir -p /etc/ssh \
    && { \
        echo "github.com ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIOMqqnkVzrm0SdG6UOoqKLsabgH5C9okWi0dh2l9GKJl"; \
        echo "github.com ecdsa-sha2-nistp256 AAAAE2VjZHNhLXNoYTItbmlzdHAyNTYAAAAIbmlzdHAyNTYAAABBBEmKSENjQEezOmxkZMy7opKgwFB9nkt5YRrYMjNuG5N87uRgg6CLrbo5wAdT/y6v0mKV0U2w0WZ2YB/++Tpockg="; \
        echo "github.com ssh-rsa AAAAB3NzaC1yc2EAAAADAQABAAABgQCj7ndNxQowgcQnjshcLrqPEiiphnt+VTTvDP6mHBL9j1aNUkY4Ue1gvwnGLVlOhGeYrnZaMgRK6+PKCUXaDbC7qtbW8gIkhL7aGCsOr/C56SJMy/BCZfxd1nWzAOxSDPgVsmerOBYfNqltV9/hWCqBywINIR+5dIg6JTJ72pcEpEjcYgXkE2YEFXV1JHnsKgbLWNlhScqb2UmyRkQyytRLtL+38TGxkxCflmO+5Z8CSSNY7GidjMIZ7Q4zMjA2n1nGrlTDkzwDCsw+wqFPGQA179cnfGWOWRVruj16z6XyvxvjJwbz0wQZ75XK5tKSb7FNyeIEs4TT4jk+S4dhPeAUC5y+bDYirYgM4GC7uEnztnZyaVWQ7B381AK4Qdrwt51ZqExKbQpTUNn+EjqoTwvqNj4kqx5QUCI0ThS/YkOxJCXmPUWZbhjpCg56i+2aB6CmK2JGhn57K5mj0MNdBXA4/WnwH6XoPWJzK5Nyu2zB3nAZp+S5hpQs+p1vN1/wsjk="; \
    } > /etc/ssh/ssh_known_hosts

# gh: 公式 apt リポジトリから（トークンは実行時に GH_TOKEN で注入する。README.md 参照）
RUN curl -fsSL https://cli.github.com/packages/githubcli-archive-keyring.gpg \
        -o /usr/share/keyrings/githubcli-archive-keyring.gpg \
    && echo "deb [arch=$(dpkg --print-architecture) signed-by=/usr/share/keyrings/githubcli-archive-keyring.gpg] https://cli.github.com/packages stable main" \
        > /etc/apt/sources.list.d/github-cli.list \
    && apt-get update && apt-get install -y --no-install-recommends gh \
    && rm -rf /var/lib/apt/lists/*

# Codex CLI: GitHub releases の musl 静的バイナリ（バージョン固定＋sha256 照合）。
# バージョンはホストの codex と揃えて上げる（ハッシュは release 資産の実測値）。
# codex-code-mode-host は codex 本体がツール実行時に spawn する相棒バイナリ（無いと exec が失敗する）
ARG CODEX_VERSION=0.147.0
ARG CODEX_SHA256_X86_64=0246e2e773834e07f0fb5249ed6ebad12e4591e608f8c7bb97dd6a9690544c36
ARG CODEX_SHA256_AARCH64=eb677c80f666b1ab8b4b1d083b66e8d614b1281d960bb6f9fd8ca98f58b38b90
ARG CODEX_HOST_SHA256_X86_64=0146adfaac8363ec9fcdb5895f7624db5b2e8617a283887938b7fb97a1dd4356
ARG CODEX_HOST_SHA256_AARCH64=dfd4ff98ea4db30ed078af9c31b6f86e3da4836d0573aa87e225e5a5b54d3c7c
RUN set -eux; \
    case "$(uname -m)" in \
        x86_64)  arch=x86_64;  sha="$CODEX_SHA256_X86_64";  host_sha="$CODEX_HOST_SHA256_X86_64" ;; \
        aarch64) arch=aarch64; sha="$CODEX_SHA256_AARCH64"; host_sha="$CODEX_HOST_SHA256_AARCH64" ;; \
        *) echo "未対応のアーキテクチャ: $(uname -m)" >&2; exit 1 ;; \
    esac; \
    curl --proto '=https' --tlsv1.2 -fsSL -o /tmp/codex.tgz \
        "https://github.com/openai/codex/releases/download/rust-v${CODEX_VERSION}/codex-${arch}-unknown-linux-musl.tar.gz"; \
    echo "${sha}  /tmp/codex.tgz" | sha256sum -c -; \
    tar xzf /tmp/codex.tgz -C /usr/local/bin; \
    mv "/usr/local/bin/codex-${arch}-unknown-linux-musl" /usr/local/bin/codex; \
    curl --proto '=https' --tlsv1.2 -fsSL -o /tmp/codex-host.tgz \
        "https://github.com/openai/codex/releases/download/rust-v${CODEX_VERSION}/codex-code-mode-host-${arch}-unknown-linux-musl.tar.gz"; \
    echo "${host_sha}  /tmp/codex-host.tgz" | sha256sum -c -; \
    tar xzf /tmp/codex-host.tgz -C /usr/local/bin; \
    mv "/usr/local/bin/codex-code-mode-host-${arch}-unknown-linux-musl" /usr/local/bin/codex-code-mode-host; \
    chmod +x /usr/local/bin/codex /usr/local/bin/codex-code-mode-host; \
    rm -f /tmp/codex.tgz /tmp/codex-host.tgz

# 開発ユーザ（既定は devcontainer 慣例の vscode 1000:1000。VS Code の updateRemoteUserUID 対象。
# apple 経路はホストのユーザ名・uid が入る。gid はホスト mac の staff=20 が Debian の既存 gid と
# 衝突し得るため uid と同値にする — virtiofs がコンテナユーザへ所有権マッピングするので実害なし）
RUN groupadd --gid "$DEV_GID" "$DEV_USER" \
    && useradd --uid "$DEV_UID" --gid "$DEV_GID" -m -d "$DEV_HOME" -s /bin/bash "$DEV_USER" \
    && echo "$DEV_USER ALL=(root) NOPASSWD:ALL" > "/etc/sudoers.d/$DEV_USER" \
    && chmod 0440 "/etc/sudoers.d/$DEV_USER"

# named volume（yuzu-target）のマウント点。bind mount（virtiofs）上の target/ は
# 遅いためワークスペース外に置き、CARGO_TARGET_DIR で差し替える
RUN mkdir /cargo-target && chown "$DEV_UID:$DEV_GID" /cargo-target

USER ${DEV_USER}

# 両経路が継承する唯一の env 定義場所（devcontainer.json の containerEnv や
# ラッパーの -e でこれらを再定義・上書きしないこと）。
# CLAUDE_CONFIG_DIR: Linux 側の claude.json / .credentials.json を ~/.claude/ 配下へ
# 集約し、ホスト mac の ~/.claude.json と分離する（apple 経路は ~/.claude を共有マウント）
ENV PATH=${DEV_HOME}/.cargo/bin:${DEV_HOME}/.local/bin:$PATH \
    CARGO_TARGET_DIR=/cargo-target \
    CLAUDE_CONFIG_DIR=${DEV_HOME}/.claude \
    CARGO_TERM_COLOR=always

# インストーラは「取得したスクリプトをそのまま shell へ渡す」形にしない。
# バージョン固定の成果物を一時ファイルへ落とし、**公開 sha256 と照合してから**実行する
# （更新するときは ARG のバージョンとハッシュをセットで書き換える。
# ハッシュは rustup が `.../rustup-init.sha256`、cargo-binstall は release 資産の実測値）。
# 例外は末尾の Claude Code（安定した固定版配布が無く、従来 post-create.sh でも curl | bash だった）。
#
# ⚠️ toolchain 名は "stable" のまま = rust-toolchain.toml と一致させる意図的な追従
# （固定するなら rust-toolchain.toml と対で変えること。冒頭の方針コメント参照）
ARG RUSTUP_VERSION=1.29.0
ARG RUSTUP_SHA256_X86_64=4acc9acc76d5079515b46346a485974457b5a79893cfb01112423c89aeb5aa10
ARG RUSTUP_SHA256_AARCH64=9732d6c5e2a098d3521fca8145d826ae0aaa067ef2385ead08e6feac88fa5792
RUN set -eux; \
    case "$(uname -m)" in \
        x86_64)  target=x86_64-unknown-linux-gnu;  sha="$RUSTUP_SHA256_X86_64" ;; \
        aarch64) target=aarch64-unknown-linux-gnu; sha="$RUSTUP_SHA256_AARCH64" ;; \
        *) echo "未対応のアーキテクチャ: $(uname -m)" >&2; exit 1 ;; \
    esac; \
    curl --proto '=https' --tlsv1.2 -fsSL -o /tmp/rustup-init \
        "https://static.rust-lang.org/rustup/archive/${RUSTUP_VERSION}/${target}/rustup-init"; \
    echo "${sha}  /tmp/rustup-init" | sha256sum -c -; \
    chmod +x /tmp/rustup-init; \
    /tmp/rustup-init -y --profile minimal --default-toolchain stable \
        --component rustfmt --component clippy \
        --target wasm32-unknown-unknown; \
    rm -f /tmp/rustup-init

# cargo-insta（スナップショット更新用）はプレビルト取得を優先し、
# 失敗時のみソースビルドへフォールバック。
# バージョンは Cargo.lock の insta と揃える（スナップショット形式の食い違い防止）
ARG BINSTALL_VERSION=1.21.1
ARG BINSTALL_SHA256_X86_64=630c8f8803a686aa6779497f0f0fb51d49822fb5fc3c514d8ced33b34e338e6e
ARG BINSTALL_SHA256_AARCH64=1dc2979f3c83aade9a1b4344589d14fafa63459b759222528d11419f5cca9cc2
ARG CARGO_INSTA_VERSION=1.48.0
RUN set -eux; \
    case "$(uname -m)" in \
        x86_64)  target=x86_64-unknown-linux-musl;  sha="$BINSTALL_SHA256_X86_64" ;; \
        aarch64) target=aarch64-unknown-linux-musl; sha="$BINSTALL_SHA256_AARCH64" ;; \
        *) echo "未対応のアーキテクチャ: $(uname -m)" >&2; exit 1 ;; \
    esac; \
    curl --proto '=https' --tlsv1.2 -fsSL -o /tmp/binstall.tgz \
        "https://github.com/cargo-bins/cargo-binstall/releases/download/v${BINSTALL_VERSION}/cargo-binstall-${target}.tgz"; \
    echo "${sha}  /tmp/binstall.tgz" | sha256sum -c -; \
    tar xzf /tmp/binstall.tgz -C "$HOME/.cargo/bin" cargo-binstall; \
    rm -f /tmp/binstall.tgz; \
    cargo binstall -y "cargo-insta@${CARGO_INSTA_VERSION}" \
        || cargo install "cargo-insta@${CARGO_INSTA_VERSION}" --locked

# Claude Code: ネイティブインストーラ（Node 不要）で ~/.local/bin へ。実行時マウント
# （~/.claude 等）に隠れない位置なのでイメージ側の本体がそのまま使える。
# down && up の作り直し（メモリ返却の運用）ごとに post-create で再取得しないための焼き込みで、
# post-create.sh の導入処理は fallback として残る（command -v ガード済み・冪等）
RUN curl -fsSL https://claude.ai/install.sh | bash

WORKDIR ${DEV_WORKSPACE}

# 長寿命コンテナとして常駐させる（ラッパーは -d で起動、devcontainer は
# overrideCommand 既定によりこの CMD と同等の待機をする）
CMD ["sleep", "infinity"]
