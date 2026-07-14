# yuzu 開発コンテナ

CI 相当（Rust stable + rustfmt / clippy + wasm32 target + cargo-insta）の Linux 環境を、
ホストを汚さずに使うためのコンテナ定義。検証の隔離実行・Claude Code の実行環境・
（必要なら）エディタ接続に使う。

**環境の実体は `Dockerfile` が唯一の定義**。`devcontainer.json`（Docker 系）と
`../scripts/dev-container.sh`（apple container / docker）はどちらもこれを参照する配線にすぎない。

## クイックスタート

### mac（apple container）

[apple/container](https://github.com/apple/container) v1.0 以降と Apple Silicon が前提。

```bash
scripts/dev-container.sh build   # イメージをビルド
scripts/dev-container.sh up      # 長寿命コンテナを起動（初回はカーネル導入で少し待つ）
scripts/dev-container.sh shell   # bash に入る → cargo test 等をそのまま実行
scripts/dev-container.sh down    # 停止・削除（ビルドキャッシュ volume は残る）
```

コンテナ内で Claude Code を使う場合は `shell` で入って `claude` を実行するだけ
（初回のみブラウザ認証。認証情報は volume に永続化され `down`/`up` を越えて残る）。

VS Code から接続したい場合: 設定で `"dev.containers.experimentalAppleContainerSupport": true`
を有効にし、`up` 済みの状態でコマンドパレットから **「Dev Containers: Attach to Running
Apple Container...」** → `yuzu-dev` → `/workspaces/yuzu` を開く
（**Reopen in Container は使えない** — Docker/Podman 前提のため）。

### Linux / Docker（VS Code・IntelliJ・Codespaces）

`.devcontainer/devcontainer.json` を通常どおり使う（VS Code なら「Reopen in Container」）。
CLI 派は同じラッパーが docker でも動く:

```bash
YUZU_CONTAINER_ENGINE=docker scripts/dev-container.sh up   # Linux では既定で docker
```

## docker + colima からの移行（mac）

1. `colima stop`（未練がなければ `brew uninstall colima docker` も可）
2. [apple/container の releases](https://github.com/apple/container/releases) から pkg を導入
3. `scripts/dev-container.sh build && scripts/dev-container.sh up`
   （`container system start` はラッパーが自動実行する。初回のみ既定カーネルの導入が走る）

docker 時代の named volume・イメージは引き継がれない（初回ビルドはコールドスタート）。

## コンテナ内での検証（verify 相当）

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace          # yuzu-server の serve テストも通る（TCP 制約なし）
cargo check -p yuzu-search-wasm --target wasm32-unknown-unknown
cargo check -p tankan --target wasm32-unknown-unknown

# CLI 実機（注意: target/ ではなく $CARGO_TARGET_DIR 配下に出る）
cargo build -p yuzu-cli
"$CARGO_TARGET_DIR/debug/yuzu" new /tmp/e2e-docs
```

## 落とし穴

- **`./target/debug/yuzu` は存在しない**: コンテナ内は `CARGO_TARGET_DIR=/cargo-target`
  （bind mount 上の `target/` は virtiofs で遅い＋ホスト mac の成果物と混ざるため）。
  CLI 実機確認は `"$CARGO_TARGET_DIR/debug/yuzu"` を使う。ホスト側 `target/` は無傷
- **ホスト編集 → コンテナ内 `yuzu dev` のホットリロードは効かない**（apple container）:
  virtiofs はホスト側で発生した変更の inotify をゲストへ伝播しない。
  **`yuzu dev` はホストで動かすのが既定運用**。例外として、コンテナ内の Claude Code が
  編集する場合はゲスト内 inotify が効くので、`yuzu dev --host 0.0.0.0` で起動すれば
  コンテナ内 dev ＋ ホストブラウザ http://127.0.0.1:5173 で動く
  （publish 経由の疎通は実機確認済み）
- **stable の追従**: イメージ内の toolchain はビルド時点の stable で固定。CI（常に最新
  stable）と clippy 結果がズレたら `scripts/dev-container.sh build --no-cache` で焼き直す
- **メモリ**: apple container はコンテナ = 軽量 VM。ラッパーが既定 8g を割り当てる
  （不足したら `YUZU_CONTAINER_MEMORY=12g scripts/dev-container.sh up`）
- **`buildkit` コンテナが常駐する**（apple container）: `container build` を一度でも
  実行すると、apple container がビルダー VM（`container ls` に `buildkit` として表示、
  2 CPU / 2GB）を自動起動し、以後のビルドを速くするため**ビルド後も残り続ける**仕様。
  yuzu のスクリプトが作ったものではない。気になるなら `container builder stop` で
  停止してよい（次の build で自動再開する）
- **Linux ホストで uid ≠ 1000 の場合**: ラッパー経路は uid 1000（vscode）固定のため
  bind mount の権限が合わない。VS Code の devcontainer 経路（updateRemoteUserUID が
  自動調整する）を使うこと

## 不変条件（devcontainer.json ⇔ scripts/dev-container.sh）

どちらかを変えるときは**必ず両方とこの表を同時に更新**する。

| 項目 | 値 | 定義場所 |
|---|---|---|
| イメージ定義 | `.devcontainer/Dockerfile` | 両者が build 参照 |
| workspace | `/workspaces/yuzu` | Dockerfile の WORKDIR ＋ 両者のマウント指定 |
| ユーザ | `vscode`（1000:1000） | Dockerfile の USER |
| env | `PATH` / `CARGO_TARGET_DIR` / `CLAUDE_CONFIG_DIR` / `CARGO_TERM_COLOR` | Dockerfile の ENV のみ（containerEnv / `-e` で再定義しない） |
| volume | `yuzu-cargo-registry:/home/vscode/.cargo/registry` / `yuzu-target:/cargo-target` / `yuzu-claude:/home/vscode/.claude` | devcontainer.json の mounts ＝ ラッパーの VOLUMES |
| ポート | 5173（devcontainer は forwardPorts、ラッパーは `-p 127.0.0.1:5173:5173`） | 意味差あり: forward は動的トンネル、publish は静的公開 |
| ライフサイクル | `post-create.sh`（冪等） | postCreateCommand ＝ ラッパー up 内の exec |
| 常駐 | `sleep infinity` | Dockerfile の CMD |
