---
name: dev-container
description: yuzu の開発コンテナ（apple container）操作の運用レシピ。scripts/dev-container.sh の使い方・yuzu 固有の罠（ホットリロード・dev.host・メモリ返却）・環境定義変更時の検証手順を扱うときに使う。apple container CLI 自体の汎用リファレンスはユーザスキル `apple-container` を参照。
---

# yuzu 開発コンテナの運用

このマシンには **docker CLI は無い**（apple container 一本化済み）。
開発コンテナの環境定義・不変条件は `.devcontainer/README.md` が正。
apple container CLI の汎用リファレンス（構文・罠・k8s プラグイン等）はユーザスキル `apple-container`。

## 原則

- **開発コンテナの操作は生 CLI ではなく `scripts/dev-container.sh` を使う**（build / up / shell / claude / codex / down / clean / status）。volume・bind mount・認証注入・ポート・リソースの配線が揃っているため
- `container` CLI はサンドボックス内から実行すると XPC 通信が **Operation not permitted** になる。**サンドボックス外での実行が必要**（`container <sub> --help` の**ヘルプ表示すら**プラグイン探索の失敗でルートヘルプに化けるので、ヘルプ確認もサンドボックス外で行う）
- コンテナ内の CLI 実機確認は `"$CARGO_TARGET_DIR/debug/yuzu"`（`./target/debug/yuzu` は存在しない）

## 基本操作

```bash
scripts/dev-container.sh build    # イメージビルド（ホスト値の ARG 付き。--no-cache 可）
scripts/dev-container.sh up       # 長寿命コンテナ起動（volume 2 本＋同一パス bind mount＋ -p 127.0.0.1:5173:5173）
scripts/dev-container.sh shell    # bash で入る（shell -c '...' で単発実行）
scripts/dev-container.sh claude   # コンテナ内で Claude Code（~/.claude 共有・初回のみコンテナ内 OAuth）
scripts/dev-container.sh codex    # コンテナ内で Codex CLI（~/.codex 共有・認証もそのまま使える）
container exec yuzu-dev bash -lc '<コマンド>'   # ワンショット実行（-lc で ENV/PATH が効く）
scripts/dev-container.sh down     # 停止・削除（volume 保持）
```

- ラッパー経路は**ホスト同一パス構成**: ホストのユーザ名・uid・HOME・リポジトリ実パスで
  イメージを焼き、リポジトリ・`~/.claude`・`~/.codex`・`~/.config/gh`・skills symlink の実体を
  同一パスへ bind mount する。認証の渡し方（claude=初回 OAuth / codex=ファイル共有 /
  gh=exec 時 GH_TOKEN 注入 / git=env＋--ssh）は `.devcontainer/README.md`「認証の仕組み」参照
- **gh を使うコマンドはラッパーの shell / claude / codex 経由で入る**こと。素の
  `container exec` は GH_TOKEN が注入されない（up 時の git identity だけは run の env に焼いてある）

## yuzu 固有の罠

- **ホスト編集 → コンテナ内 `yuzu dev` のホットリロードは効かない**（virtiofs の inotify 制限）。**`yuzu dev` はホスト実行が既定運用**。コンテナ内の Claude Code が編集する場合はゲスト内 inotify が効くので動く
- **コンテナ内 `yuzu dev/preview` にホストから繋ぐには** `--host 0.0.0.0` を付けて起動する（v0.3 で追加。設定ファイルなら `yuzu.toml` の `[dev]` に `host` を書く — TOML の重複キーは構文エラーになるので既存セクションへ追記する。なお watch 中の `dev.host` 変更は起動時固定＝警告のみ）
- **メモリ圧が上がったら** `scripts/dev-container.sh down && up` で作り直す（ゲストで解放したメモリが macOS に返らない制限。キャッシュは volume なので失われない）

## 環境定義を変えたときの検証

1. `.devcontainer/README.md` の不変条件表と devcontainer.json・dev-container.sh の三者を同時更新
2. `scripts/dev-container.sh build && down && up && shell` で最低限: whoami=ホストのユーザ名 / pwd=ホストと同一のリポジトリパス / `cargo build` が warm / `gh auth status` が通る / `container exec yuzu-dev bash -lc 'bash /path/to/repo/.devcontainer/post-create.sh'` が冪等
3. Docker 経路は手元で検証できない（docker CLI 無し）— push 後の `.github/workflows/container.yml` が肩代わりする（ARG 既定値 = vscode/1000 でのビルドと、codex の x86_64 ハッシュ・gh の arch 分岐はここで初めて検証される）
