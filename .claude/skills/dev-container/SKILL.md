---
name: dev-container
description: yuzu の開発コンテナ（apple container）操作の運用レシピ。scripts/dev-container.sh の使い方・yuzu 固有の罠（ホットリロード・dev.host・メモリ返却）・環境定義変更時の検証手順を扱うときに使う。apple container CLI 自体の汎用リファレンスはユーザスキル `apple-container` を参照。
---

# yuzu 開発コンテナの運用

このマシンには **docker CLI は無い**（apple container 一本化済み）。
開発コンテナの環境定義・不変条件は `.devcontainer/README.md` が正。
apple container CLI の汎用リファレンス（構文・罠・k8s プラグイン等）はユーザスキル `apple-container`。

## 原則

- **開発コンテナの操作は生 CLI ではなく `scripts/dev-container.sh` を使う**（build / up / shell / down / clean / status）。volume・ポート・リソースの配線が揃っているため
- `container` CLI はサンドボックス内から実行すると XPC 通信が **Operation not permitted** になる。**サンドボックス外での実行が必要**（`container <sub> --help` の**ヘルプ表示すら**プラグイン探索の失敗でルートヘルプに化けるので、ヘルプ確認もサンドボックス外で行う）
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

- **ホスト編集 → コンテナ内 `yuzu dev` のホットリロードは効かない**（virtiofs の inotify 制限）。**`yuzu dev` はホスト実行が既定運用**。コンテナ内の Claude Code が編集する場合はゲスト内 inotify が効くので動く
- **コンテナ内 `yuzu dev/preview` にホストから繋ぐには** `--host 0.0.0.0` を付けて起動する（v0.3 で追加。設定ファイルなら `dev.host` — **JSONC の重複キーは後勝ち**なので既存の `dev` セクションに追記する。重複時は build が警告を出す）
- **メモリ圧が上がったら** `scripts/dev-container.sh down && up` で作り直す（ゲストで解放したメモリが macOS に返らない制限。キャッシュは volume なので失われない）

## 環境定義を変えたときの検証

1. `.devcontainer/README.md` の不変条件表と devcontainer.json・dev-container.sh の三者を同時更新
2. `scripts/dev-container.sh build && down && up && shell` で最低限: whoami=vscode / pwd=/workspaces/yuzu / `cargo build` が warm / `container exec yuzu-dev bash -lc 'bash .devcontainer/post-create.sh'` が冪等
3. Docker 経路は手元で検証できない（docker CLI 無し）— push 後の `.github/workflows/container.yml` が肩代わりする
