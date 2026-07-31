# yuzu 開発環境の唯一の定義。
# devcontainer.json（Docker 系）と scripts/dev-container.sh（apple container / docker）の
# 両経路がこのファイルを参照する。環境の実体（env・ユーザ・作業ディレクトリ・常駐コマンド）は
# すべてここに集約し、各経路では再定義しない（経路間の食い違い防止）。
#
# 方針:
# - CI 相当の軽量構成（Rust stable + rustfmt/clippy + wasm32 target + cargo-insta）。
#   vendor 更新ツール（wasm-bindgen-cli / binaryen）は焼かない（頻度が低く重いため）
# - toolchain は「stable」という名前でインストールする — rust-toolchain.toml の
#   channel = "stable" と一致させ、初回 cargo 実行時の二重ダウンロードを防ぐ。
#   このため rust:slim / devcontainers/rust（バージョン固定 toolchain 入り）は使わない
# - stable を追従させたいときは scripts/dev-container.sh build --no-cache で焼き直す
FROM debian:bookworm-slim

# gcc/libc6-dev: rustc のリンカ用（ネイティブ C 依存 crate は無いので g++ は不要）
# curl/xz-utils: scripts/*.sh（vendor 系）と各種インストーラ用
# sudo: post-create.sh が named volume マウント点の所有権を直すため（vscode に NOPASSWD）
RUN apt-get update && apt-get install -y --no-install-recommends \
        ca-certificates curl git gcc libc6-dev make procps sudo xz-utils \
    && rm -rf /var/lib/apt/lists/*

# devcontainer 慣例の vscode ユーザ（uid/gid 1000。VS Code の updateRemoteUserUID 対象）
RUN groupadd --gid 1000 vscode \
    && useradd --uid 1000 --gid 1000 -m -s /bin/bash vscode \
    && echo 'vscode ALL=(root) NOPASSWD:ALL' > /etc/sudoers.d/vscode \
    && chmod 0440 /etc/sudoers.d/vscode

# named volume（yuzu-target）のマウント点。bind mount（virtiofs）上の target/ は
# 遅いためワークスペース外に置き、CARGO_TARGET_DIR で差し替える
RUN mkdir /cargo-target && chown vscode:vscode /cargo-target

USER vscode

# 両経路が継承する唯一の env 定義場所（devcontainer.json の containerEnv や
# ラッパーの -e でこれらを再定義・上書きしないこと）
ENV PATH=/home/vscode/.cargo/bin:/home/vscode/.local/bin:$PATH \
    CARGO_TARGET_DIR=/cargo-target \
    CLAUDE_CONFIG_DIR=/home/vscode/.claude \
    CARGO_TERM_COLOR=always

# インストーラは「取得したスクリプトをそのまま shell へ渡す」形にしない。
# バージョン固定の成果物を一時ファイルへ落とし、**公開 sha256 と照合してから**実行する
# （更新するときは ARG のバージョンとハッシュをセットで書き換える。
# ハッシュは rustup が `.../rustup-init.sha256`、cargo-binstall は release 資産の実測値）。
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

WORKDIR /workspaces/yuzu

# 長寿命コンテナとして常駐させる（ラッパーは -d で起動、devcontainer は
# overrideCommand 既定によりこの CMD と同等の待機をする）
CMD ["sleep", "infinity"]
