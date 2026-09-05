# yuzu 開発コンテナ

CI 相当（Rust stable + rustfmt / clippy + wasm32 target + cargo-insta）＋エージェント実行環境
（Claude Code / Codex CLI / gh）の Linux 環境を、ホストを汚さずに使うためのコンテナ定義。
検証の隔離実行・Claude Code / Codex の実行環境・（必要なら）エディタ接続に使う。

**環境の実体は `Dockerfile` が唯一の定義**。`devcontainer.json`（Docker 系）と
`../scripts/dev-container.sh`（apple container / docker）はどちらもこれを参照する配線にすぎない。
ユーザ・HOME・作業ディレクトリは Dockerfile の ARG でパラメタ化されており、
devcontainer 経路は既定値（vscode/1000 + `/workspaces/yuzu`）、ラッパー経路は**ホスト値**
（ユーザ名・uid・`$HOME`・リポジトリ実パス）で焼く「ホスト同一パス」構成になる。

## クイックスタート

### mac（apple container）

[apple/container](https://github.com/apple/container) v1.0 以降と Apple Silicon が前提。

```bash
scripts/dev-container.sh build   # イメージをビルド（ホスト値の ARG 付き）
scripts/dev-container.sh up      # 長寿命コンテナを起動（初回はカーネル導入で少し待つ）
scripts/dev-container.sh shell   # bash に入る → cargo test 等をそのまま実行
scripts/dev-container.sh claude  # コンテナ内で Claude Code を起動
scripts/dev-container.sh codex   # コンテナ内で Codex CLI を起動
scripts/dev-container.sh down    # 停止・削除（ビルドキャッシュ volume は残る）
```

ラッパー経路はリポジトリと `~/.claude` / `~/.codex` / `~/.config/gh`（＋ `~/.claude/skills` が
symlink ならその実体）を**ホストと同一の絶対パスへ bind mount** する。これにより skills の
絶対 symlink・hooks の絶対パス・codex の projects トラスト・Claude / Codex のプロジェクト
履歴がホストとそのまま共有される。

### 認証の仕組み（ラッパー経路）

| 対象 | ホストでの保存場所 | コンテナへの渡し方 |
|---|---|---|
| Claude Code | macOS Keychain（ファイルなし） | **初回のみコンテナ内で OAuth ログイン** → `~/.claude/.credentials.json` に永続（マウント先＝ホスト側に残る）。以後不要 |
| Codex | `~/.codex/auth.json`（平文） | マウントだけで完結 |
| gh | macOS Keychain | ラッパーが exec のたびに `gh auth token` で取り出し `GH_TOKEN` を値なし `-e` で注入（argv に値を出さない） |
| git identity | `~/.gitconfig`（マウントしない） | `GIT_AUTHOR_*` / `GIT_COMMITTER_*` を env 注入（lfs filter 事故回避のため gitconfig 自体は共有しない） |
| git push/fetch | SSH agent | `container run --ssh` の agent フォワード（`~/.ssh` はマウントしない） |

旧構成（`~/.claude` を `yuzu-claude` volume にしていた頃）から移行したら、
`build` → `down` → `up` で作り直したうえで `container volume rm yuzu-claude` で残骸を消してよい
（`clean` にも掃除が入っている）。

VS Code から接続したい場合: 設定で `"dev.containers.experimentalAppleContainerSupport": true`
を有効にし、`up` 済みの状態でコマンドパレットから **「Dev Containers: Attach to Running
Apple Container...」** → `yuzu-dev` → **ホストと同一のリポジトリ実パス**を開く
（ラッパー経路の workspace はホスト同一パス。旧構成の `/workspaces/yuzu` はもう存在しない）
（**Reopen in Container は使えない** — Docker/Podman 前提のため）。

### Linux / Docker（VS Code・IntelliJ・Codespaces）

`.devcontainer/devcontainer.json` を通常どおり使う（VS Code なら「Reopen in Container」）。
この経路はホスト同一パス化せず、claude / codex / gh の設定は volume（`yuzu-claude` /
`yuzu-codex` / `yuzu-gh`）で永続化する — コンテナ内で各自ログインする（`down`/`up` を越えて残る）。
CLI 派は同じラッパーが docker でも動く（この場合はラッパー経路 = ホスト同一パス構成になる）:

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
cargo check -p mikan-wasm --target wasm32-unknown-unknown
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
  （toolchain 名 `stable` は `rust-toolchain.toml` と一致させる意図。版を固定するなら対で変える）
- **インストーラはハッシュ検証つき**: rustup-init・cargo-binstall・codex は
  バージョン固定の成果物を落として sha256 を照合してから実行する（`curl | sh` にしない。
  例外は Claude Code — 固定版配布が無くネイティブインストーラを使う）。
  更新は Dockerfile の `ARG` のバージョンとハッシュを**セットで**書き換える。
  ハッシュは rustup が `https://static.rust-lang.org/rustup/archive/<版>/<target>/rustup-init.sha256`、
  cargo-binstall / codex は release 資産の実測値。`cargo-insta` は `Cargo.lock` の `insta` と版を
  揃え、codex はホストの `codex --version` と揃える
- **`~/.claude` は rw で共有される**（ラッパー経路）: コンテナ内のプロセスはホストの
  Claude Code 設定・hooks・skills を書き換えられる。VM による隔離はホスト認証・設定には
  及ばない前提で使う（GH_TOKEN も `container inspect` の env には出ないが exec へは渡る）
- **`~/.claude.json` は共有しない**: `CLAUDE_CONFIG_DIR=~/.claude` により Linux 側の
  状態ファイルは `~/.claude/` 配下に入り、ホスト mac の `~/.claude.json` と書き込み競合しない
- **メモリ**: apple container はコンテナ = 軽量 VM。ラッパーが既定 8g を割り当てる
  （不足したら `YUZU_CONTAINER_MEMORY=12g scripts/dev-container.sh up`）
- **Codex のサンドボックス（Landlock）がコンテナのカーネルで動かない場合**は
  `codex -c sandbox_mode="danger-full-access"` にフォールバックする
  （コンテナ＝軽量 VM の境界自体が隔離になっている）
- **`buildkit` コンテナが常駐する**（apple container）: `container build` を一度でも
  実行すると、apple container がビルダー VM（`container ls` に `buildkit` として表示、
  2 CPU / 2GB）を自動起動し、以後のビルドを速くするため**ビルド後も残り続ける**仕様。
  yuzu のスクリプトが作ったものではない。気になるなら `container builder stop` で
  停止してよい（次の build で自動再開する）
- **Linux ホストの uid**: ラッパー経路はホストの uid でイメージを焼くため bind mount の
  権限は常に一致する（旧構成の「uid 1000 固定」制限は解消済み）。devcontainer 経路は
  従来どおり updateRemoteUserUID が調整する

## 不変条件（devcontainer.json ⇔ scripts/dev-container.sh）

どちらかを変えるときは**必ず両方とこの表を同時に更新**する。

| 項目 | 値 | 定義場所 |
|---|---|---|
| イメージ定義 | `.devcontainer/Dockerfile` | 両者が build 参照 |
| ユーザ / HOME / workspace | ARG `DEV_USER` / `DEV_UID` / `DEV_GID` / `DEV_HOME` / `DEV_WORKSPACE`。既定 = devcontainer 経路（`vscode` 1000:1000 / `/home/vscode` / `/workspaces/yuzu`）、ラッパー経路 = ホスト値（gid は uid と同値） | Dockerfile の ARG（ラッパーが `--build-arg` で上書き） |
| env | `PATH` / `CARGO_TARGET_DIR` / `CLAUDE_CONFIG_DIR` / `CARGO_TERM_COLOR` | Dockerfile の ENV のみ（containerEnv で再定義しない。ラッパーの `-e` は認証・identity の**追加**のみ） |
| volume | `yuzu-cargo-registry:$DEV_HOME/.cargo/registry` / `yuzu-target:/cargo-target` | devcontainer.json の mounts ＝ ラッパーの VOLUMES（名前一致・マウント先は DEV_HOME 依存） |
| claude / codex / gh 設定 | devcontainer 経路 = volume（`yuzu-claude` / `yuzu-codex` / `yuzu-gh`）、ラッパー経路 = ホストの実ディレクトリを同一パスへ bind mount | devcontainer.json の mounts / ラッパー `cmd_up` |
| ポート | 5173（devcontainer は forwardPorts、ラッパーは `-p 127.0.0.1:5173:5173`） | 意味差あり: forward は動的トンネル、publish は静的公開 |
| ライフサイクル | `post-create.sh`（冪等） | postCreateCommand ＝ ラッパー up 内の exec |
| 常駐 | `sleep infinity` | Dockerfile の CMD |
