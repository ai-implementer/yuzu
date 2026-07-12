---
name: apple-container
description: apple container（v1.1.0）の CLI リファレンスと運用レシピ（汎用）。コンテナの起動・ビルド・デバッグ、v1.1.0 固有の構文と罠（初回起動・buildkit 常駐・inotify・exec）を扱うときに使う。yuzu の開発コンテナ操作は末尾の「このプロジェクト固有の運用」を参照。
---

# apple container（v1.1.0）リファレンス

macOS 26 / Apple Silicon 前提（公式: https://github.com/apple/container）。
このファイルは汎用リファレンスで、**yuzu 固有の内容は末尾の「このプロジェクト固有の運用」にまとめてある**。

## 基本情報

- Apple Silicon Mac 専用（Intel Mac 不可）。macOS 26 推奨、macOS 15 でも動作するが制限あり
- Linux コンテナを「軽量 VM 1 つ = コンテナ 1 個」方式で起動。OCI 互換イメージ対応（Docker / podman のイメージが流用可能）
- 対応アーキテクチャ: `linux/arm64`（ネイティブ）/ `linux/amd64`（Rosetta 経由）
- **macOS 15 の制限**: コンテナ間ネットワーク通信不可・`container network` コマンド利用不可・サブネット固定 `192.168.64.1/24`

## インストール・管理

| 操作 | コマンド |
|---|---|
| インストール | `brew install container` → `container system start` |
| アップグレード | `container system stop` → `brew upgrade container` → `container system start` |
| 起動確認 | `container system status` |
| アンインストール | `container system stop` → `brew uninstall container` |

- **初回の `container system start` はカーネル導入プロンプトが出る** → `--enable-kernel-install` で非対話化（`container system kernel set --recommended` でも可）

## CLI グループと 1 文字 alias

- **コンテナ**: run / create / start / stop / kill / delete(rm) / list(ls) / exec / logs / inspect / stats / cp / export / prune
- **イメージ** `i`: build / pull / push / list / inspect / tag / save / load / delete / prune
- **ビルダー**: start / status / stop / delete
- **ネットワーク** `n`（macOS 26+）: create / list / inspect / delete / prune
- **ボリューム** `v`: create / list / inspect / delete / prune
- **レジストリ** `r`: login / logout / list
- **マシン** `m`: create / run / list / inspect / set / set-default / logs / stop / delete
- **システム** `s`: start / stop / status / version / logs / df / dns / kernel / property

## `container run` 主要オプション

| 用途 | フラグ |
|---|---|
| 名前付け | `--name <id>` |
| バックグラウンド | `-d` / `--detach` |
| 自動削除 | `--rm` |
| 対話 | `-it` |
| 環境変数 | `-e KEY=VAL` / `--env-file <path>` |
| 作業ディレクトリ | `-w /path` |
| ユーザー | `-u name\|uid[:gid]` / `--uid` / `--gid` |
| リソース上限 | `--ulimit <type>=<soft>[:<hard>]` |
| エントリーポイント | `--entrypoint <cmd>` |
| プラットフォーム | `--platform <os/arch[/variant]>`（`--os` / `--arch` より優先） |
| 読み取り専用 FS | `--read-only` |
| DNS | `--dns <ip>` / `--dns-domain` / `--dns-search` / `--dns-option` / `--no-dns` |
| UNIX ソケット公開 | `--publish-socket <host_path>:<container_path>` |
| init プロセス | `--init`（シグナル転送・ゾンビ回収）/ `--init-image <image>` |
| ネスト仮想化 | `--virtualization --kernel /path/to/vmlinux-kvm` |

リソース指定:

```bash
--cpus 8 --memory 32g              # 既定: 4 CPU / 1 GiB（コンテナ = 軽量 VM なので明示推奨）
--shm-size 1G                      # /dev/shm サイズ
--tmpfs /tmp                       # tmpfs マウント
```

- 既定値は `~/.config/container/config.toml` の `[container]` で恒久変更可

ファイル共有:

- `--volume` と `--mount type=<>,source=<>,target=<>,readonly` は同機能の別表記
- **匿名ボリューム（`-v /path` 形式）は `--rm` でも自動削除されない**（docker と異なる）

ポート公開: `[host-ip:]host-port:container-port[/protocol]`

```bash
-p 127.0.0.1:8080:8000             # IPv4 + host-ip（実機確認済み）
-p '[::1]:8080:8000'               # IPv6
-p 8080:80/udp                     # UDP
```

停止の挙動: `container stop` は SIGTERM → **5 秒後に SIGKILL**（`-s SIGINT -t 30` で変更可）

## イメージビルド（`container build`）

```bash
container build -t <tag> -f <Dockerfile> <context>
--arch arm64 --arch amd64          # マルチプラットフォーム
--build-arg KEY=VAL                # ビルド引数
--target production                # ステージ指定
--secret id=token,src=./token.txt  # ビルドシークレット
--output type=tar,dest=./img.tar   # 出力形式（oci / tar / local）
--pull                             # ベースイメージ再取得
--no-cache                         # キャッシュ無効
-c 8 -m 16g                        # ビルダーリソース指定
```

- BuildKit ベース。Dockerfile 探索順: `Dockerfile` → `Containerfile`
- **ビルダー VM（buildkit）が常駐する**: `container build` 初回に自動起動（既定 2 CPU / 2 GiB、`container ls` に `buildkit` として表示）し、**ビルド後も残り続ける**仕様。消してよい（`container builder stop`、次の build で自動再開）
- ビルダーのリソース変更: `container builder stop && container builder delete` → `container builder start --cpus 8 --memory 32g`
- ビルダーの Rosetta 無効化: config.toml に `[build] rosetta = false`

## イメージ管理

```bash
container image list               # ローカル一覧
container image pull <ref>         # 取得
container image push <ref>         # 送出
container image tag <src> <dst>    # 別名付与
container image save -o img.tar    # tar 保存
container image load -i img.tar    # tar 読み込み
container image delete <id>        # 削除
container image prune -a           # 未使用削除
```

## レジストリ認証

```bash
container registry login <host>                                  # 対話入力
echo $TOKEN | container registry login --password-stdin -u me <host>
container registry list
container registry logout <host>
```

- 既定レジストリ: config.toml の `[registry] domain`
- `--scheme auto` で HTTPS / HTTP を自動判定（http / https の明示も可）

## コンテナ管理

```bash
container ls                       # 実行中のみ（-a で停止中も）。IP 列にコンテナ直 IP（192.168.64.x）が出る
container inspect <id>             # JSON 詳細（マウント・ネットワークの実際の値）
container logs <id>                # 標準出力ログ
container logs --boot <id>         # VM ブートログ（起動自体に失敗するとき）
container logs -f -n 100 <id>      # tail -f 相当
container exec -it <id> sh         # シェルで入る
container cp <src> <dst>           # ファイルコピー
container stop <id>                # SIGTERM（5 秒後 SIGKILL）
container kill <id>                # 即時 SIGKILL
container rm <id>                  # 停止後削除（-f で強制）
container prune                    # 停止中のコンテナ削除
container stats                    # 全コンテナを top 風表示（--no-stream で単発）
container export -o <file> <id>    # FS を tar エクスポート
```

## ネットワーク（macOS 26+）

```bash
container network create <name>
container network create <name> --subnet 192.168.100.0/24 --subnet-v6 fd00:1234::/64
container network ls / inspect <name> / delete <name> / prune
```

- `default`（vmnet）ネットワークが自動作成される。コンテナには直 IP が付くため、ポート公開なしでもホストから `http://<コンテナIP>:<port>` で到達できる
- `container run --network <name>[,mac=XX:..][,mtu=VALUE]` で接続。MAC 指定時は第 1 オクテットの最下位 2 bit を `10`（ローカル管理）にする
- 既定サブネット変更: config.toml の `[network] subnet` / `subnetv6`（新規・既存 default の両方に起動時適用）

## ローカル DNS

```bash
sudo container system dns create <domain>
sudo container system dns create host.container.internal --localhost 203.0.113.113
container system dns list
sudo container system dns delete <domain>
```

- `--localhost` 使用時は iCloud Private Relay が無効化される
- **DNS ルールは macOS 再起動で消える**

## ボリューム

```bash
container volume create <name>
container volume create --opt journal=ordered <name>   # ext4 ジャーナル（ordered/writeback/journal）
container volume create -s 10g <name>                  # サイズ指定
container volume ls / inspect <name> / rm <name> / prune
```

## コンテナマシン（`container machine` / alias `m`）

永続化された Linux 開発環境。OCI イメージから作成し、ホスト用ユーザーを自動作成、
`$HOME` を `/Users/<username>` にマウント、`/sbin/init` 起動（systemd の常駐サービス可）。

```bash
container machine create <image> --name <id>
container machine create --cpus 4 --memory 8G --set-default <image>
container machine create --no-boot <image> --name <id>   # 起動せず作成
container machine set-default <id>
m run                              # 対話シェル（既定マシン）
m run -n <id> -- <cmd>             # コマンド実行
m ls / inspect <id> / stop <id> / rm <id>
m set -n <id> cpus=4 memory=8G
m set -n <id> home-mount=ro        # ro / rw / none
```

- 既定リソース: CPU = ホストコア数の半分（最低 4）、メモリ = ホスト物理メモリの半分（最低 1 GiB）
- **ネスト仮想化**（Apple Silicon M3 以降 + macOS 15 以降 + `CONFIG_KVM=y` カーネル）:
  `m create --virtualization --kernel /path/to/vmlinux-kvm --name <id> <image>` →
  `m run -n <id> -- ls -l /dev/kvm`。切替は `m set -n <id> virtualization=true kernel=<path>`、
  解除は `m set -n <id> kernel=`
- **独自イメージ**: `/sbin/init` を含む任意の Linux イメージ対応。プロビジョニングは
  イメージ内 `/etc/machine/create-user.sh`（env: `CONTAINER_USER` / `CONTAINER_UID` /
  `CONTAINER_GID` / `CONTAINER_HOME` / `CONTAINER_MACHINE_ID`）

## システム

```bash
container system start / stop / status / version
container system logs -f                    # サービスログ（リアルタイム）
container system logs --last 1h
container system df                         # ディスク使用量
container system property list              # システムプロパティ（TOML。--format json 可）
```

カーネル管理:

```bash
container system kernel set --recommended
container system kernel set --tar https://…kata.tar.zst --binary opt/kata/…/vmlinux
container system kernel set --binary ./vmlinux --arch arm64 --force
```

- `--arch` は `arm64`（既定）/ `amd64` のみ。amd64 ゲストは Rosetta で動作

## 設定ファイル（`~/.config/container/config.toml`）

| セクション | 主な用途 |
|---|---|
| `[build]` | ビルダー VM の CPU / メモリ / Rosetta / イメージ |
| `[container]` | run / create の既定 CPU・メモリ |
| `[dns]` | コンテナ名に補完されるドメイン |
| `[kernel]` | カーネルのパス・URL |
| `[network]` | 既定 subnet / subnetv6 |
| `[registry]` | イメージ参照の既定レジストリドメイン |
| `[vminit]` | vminitd イメージ |
| `[plugin.<id>]` | プラグイン固有設定 |

- メモリ表記は二進系（1024 基数）: `b` / `k|kb|kib` / `m|mb|mib` / `g|gb|gib` / `t|tb|tib` / `p|pb|pib`。裸の整数はバイト扱い
- CIDR 記法: IPv4 `"192.168.100.0/24"` / IPv6 `"fd00:abcd::/64"`

## capability 管理

```bash
container run --cap-add NET_ADMIN <image>
container run --cap-add ALL <image>
container run --cap-drop ALL --cap-add SETUID --cap-add SETGID <image>
```

- 既定は制限セット（`CAP_NET_BIND_SERVICE` 等）。`CAP_` 接頭辞・大文字小文字は不問
- `--cap-drop` が `--cap-add` より先に処理される

## シェル補完

```bash
container --generate-completion-script zsh > ~/.oh-my-zsh/completions/_container
container --generate-completion-script bash > /opt/homebrew/etc/bash_completion.d/container
container --generate-completion-script fish > ~/.config/fish/completions/container.fish
```

## 罠・既知の制限（汎用）

- **virtiofs はホスト側編集の inotify をゲストへ伝播しない**: ホストで編集したファイルの変更をコンテナ内のファイル監視（ホットリロード等）が検知できない。ゲスト内で発生した書き込みには inotify が効く
- **ゲストで解放したメモリが macOS に返らないことがある**: 長期稼働コンテナは定期的な再作成を推奨
- **`container exec` 内の `pkill -f` は自爆する**: exec の bash コマンドライン自体がパターンにマッチして自プロセスを殺し、exec がハングする。**`pkill -x <プロセス名>` を使う**
- **コンテナローカル領域（/tmp 等）はコンテナ再作成で消える**: 残したいものは volume か bind mount 上に置く
- **ポート疎通しないときは bind アドレスを疑う**: サーバが 127.0.0.1 バインドだと publish 経由は「TCP は繋がるが空応答」になる。0.0.0.0 バインドに変更するか、コンテナ直 IP へ繋ぐ。バインド確認はコンテナ内で `grep ":<16進port> " /proc/net/tcp`（例: 5173 = 0x1435。`0100007F`=127.0.0.1 / `00000000`=0.0.0.0）
- **匿名 volume は `--rm` でも自動削除されない**（docker と挙動が違う）: `container volume ls` で確認し `prune` で掃除
- **compose 非対応**
- **VS Code 連携は attach のみ**: `dev.containers.experimentalAppleContainerSupport: true` →「Attach to Running Apple Container」。Reopen in Container は不可（Docker/Podman 前提）。JetBrains の DevContainers も Docker/Podman 前提で不可
- **SSH エージェント**: `--ssh` のソケットパスは `/var/host-services/ssh-auth.sock`（公式 docs の `/run/…` 記載は誤り）

---

# このプロジェクト（yuzu）固有の運用

このマシンには **docker CLI は無い**（apple container 一本化済み）。
開発コンテナの環境定義・不変条件は `.devcontainer/README.md` が正。

## 原則

- **開発コンテナの操作は生 CLI ではなく `scripts/dev-container.sh` を使う**（build / up / shell / down / clean / status）。volume・ポート・リソースの配線が揃っているため
- `container` CLI はサンドボックス内から実行すると XPC 通信が **Operation not permitted** になる。**サンドボックス外での実行が必要**
- コンテナ内の CLI 実機確認は `"$CARGO_TARGET_DIR/debug/yuzu"`（`./target/debug/yuzu` は存在しない）

## 基本操作

```bash
scripts/dev-container.sh build    # イメージビルド（--no-cache 可）
scripts/dev-container.sh up       # 長寿命コンテナ起動（volume 3 本＋ -p 127.0.0.1:5173:5173）
scripts/dev-container.sh shell    # bash で入る
container exec yuzu-dev bash -lc '<コマンド>'   # ワンショット実行（-lc で ENV/PATH が効く）
scripts/dev-container.sh down     # 停止・削除（volume 保持）
```

## yuzu 固有の罠

- **ホスト編集 → コンテナ内 `yuzu dev` のホットリロードは効かない**（上記 inotify 制限）。**`yuzu dev` はホスト実行が既定運用**。コンテナ内の Claude Code が編集する場合はゲスト内 inotify が効くので動く
- **コンテナ内 `yuzu dev/preview` にホストから繋ぐには** `yuzu.jsonc` の**既存の** `dev` セクションに `"host": "0.0.0.0"` を追加する（`--host` フラグは無い。**JSONC の重複キーは後勝ち** — 新しい `dev` セクションを別に足しても無視される）
- **メモリ圧が上がったら** `scripts/dev-container.sh down && up` で作り直す（上記メモリ返却の制限。キャッシュは volume なので失われない）

## 環境定義を変えたときの検証

1. `.devcontainer/README.md` の不変条件表と devcontainer.json・dev-container.sh の三者を同時更新
2. `scripts/dev-container.sh build && down && up && shell` で最低限: whoami=vscode / pwd=/workspaces/yuzu / `cargo build` が warm / `container exec yuzu-dev bash -lc 'bash .devcontainer/post-create.sh'` が冪等
3. Docker 経路は手元で検証できない（docker CLI 無し）— push 後の `.github/workflows/container.yml` が肩代わりする
