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

RUN curl --proto '=https' --tlsv1.2 -fsSL https://sh.rustup.rs | sh -s -- -y \
        --profile minimal --default-toolchain stable \
        --component rustfmt --component clippy \
        --target wasm32-unknown-unknown

# cargo-insta（スナップショット更新用）はプレビルト取得を優先し、
# 失敗時のみソースビルドへフォールバック
RUN curl -fsSL https://raw.githubusercontent.com/cargo-bins/cargo-binstall/main/install-from-binstall-release.sh | bash \
    && (cargo binstall -y cargo-insta || cargo install cargo-insta --locked)

WORKDIR /workspaces/yuzu

# 長寿命コンテナとして常駐させる（ラッパーは -d で起動、devcontainer は
# overrideCommand 既定によりこの CMD と同等の待機をする）
CMD ["sleep", "infinity"]
