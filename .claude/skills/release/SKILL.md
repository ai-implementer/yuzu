---
name: release
description: yuzu の新バージョンをリリースする（README 更新 → バンプコミット → push → CI green → 注釈付きタグ → GitHub Release 公開）。マイナー / パッチの違いと release.yml の検証条件を含む。バージョンを上げるときに使う。
---

# yuzu リリース手順

`.github/workflows/release.yml` が **タグ push をトリガ**に 4 プラットフォームのバイナリを
ビルドし、draft Release へ集約して SHA256SUMS を添付してから公開する。

**順序を守ること。** release.yml は次の 2 つを検証して落とすので、後述の 1→4 以外の順番は必ず失敗する。

- タグ名 `vX.Y.Z` と ルート Cargo.toml の `workspace.package.version` が一致すること
- タグのコミットが `origin/main` の祖先であること（= CI を通っていないコミットにタグを打たせない）

## 0. 事前検証

`verify` スキルの一式を通す。**ホストの stable が CI（常に最新 stable）より古いと
clippy 結果がズレる**ので、着手時に確認する:

```bash
rustup check           # サンドボックス外。マイナーがズレていたら rustup update stable
```

マイナーがズレたまま進めるなら開発コンテナ（`scripts/dev-container.sh up`）で検証する。

## 1. README ロードマップの更新

**マイナーとパッチで書き方が違う。**

- **マイナー（0.X.0）**: 該当行を「…は完了・リリース済み。」へ書き換え、直後に
  `<details><summary>完了済み: v0.X（Phase A〜B）の内訳</summary>` の表を新設する。
  独立コミット `docs: vX.Y（Phase A〜B）を完了済みへ整理` にする
- **パッチ（0.X.Y）**: **内訳表は作らない**。既存の v0.X 行の末尾に 1 文足すだけ
  （例「v0.9.1 でサイドバーのスクロール位置をページ遷移をまたいで維持する改善を追加。」）。
  前例は v0.4.1 と v0.9.1 の 2 件

## 2. バンプコミット

```bash
# ルート Cargo.toml の workspace.package.version を書き換えてから
cargo build            # Cargo.lock を追随させる
git add Cargo.toml Cargo.lock
```

- コミットメッセージは `リリース: ワークスペースバージョンを X.Y.Z へ`
- **変更はこの 2 ファイルちょうど**（過去 11 回すべてこの粒度）。他のファイルが混ざったら分ける
- 本文には何のリリースか（Phase 概要）と、tankan / mikan が対象外である旨を書く

## 3. push して CI green を確認

```bash
git push origin main
gh run list --branch main --limit 2   # CI と docs の両方が success になるまで待つ
```

## 4. 注釈付きタグを push

```bash
git tag -a vX.Y.Z -m "yuzu vX.Y.Z — <日本語概要>"
git push origin vX.Y.Z
```

- **注釈付き（`-a`）必須**。メッセージ形式は過去 11 タグすべて `yuzu vX.Y.Z — <概要>`（em dash）で統一
- 公開の確認:
  ```bash
  gh run list --workflow release.yml --limit 1
  gh release view vX.Y.Z --json isDraft,assets
  ```
  アセットは 4 バイナリ ＋ SHA256SUMS の 5 件、`isDraft: false` になる

## 罠

- **タグは打った時点の main を切り出す**。機能コミットをタグより後に積むと配布バイナリに入らない。
  一方 docs サイトは main から作られるので**反映済みに見えて気づきにくい**
  （v0.9.1 はまさにこれ = サイドバー改善が v0.9.0 のバイナリに入っていなかったため切ったパッチリリース）
- 一部ジョブだけ失敗したら Actions の「**Re-run failed jobs**」だけで復旧できる
  （アップロードは `--clobber` で上書き・公開まで draft なので外部には見えない）
- `cargo package --locked -p tankan -p mikan` は**作業ツリーが dirty だと拒否される**。
  バンプコミット後に走らせること（CI はコミット済みなので通る）

## crates.io（tankan / mikan）は非同期

yuzu のリリースとは切り離して、変更が溜まったときだけ行う（**実行回数はまだ 0 回**）。
手順は CLAUDE.md「汎用ライブラリの crates.io 公開」を参照。
