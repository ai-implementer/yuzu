---
name: dependency-update
description: 依存の更新と監視の扱い（dependabot の PR・deps.yml の定期実行の失敗・cargo-deny の失敗）。CI の結果から追随作業（スナップショット・CACHE_FORMAT_VERSION・検索 wasm の作り直し・ライセンスの記録）を見分ける。dependabot の PR が来たとき、勧告の通知が来たとき、依存を手で上げるときに使う。
---

# 依存の更新と監視

依存の監視は 3 つある（Phase 82）。どれも失敗の扱いはこのスキルに従う。

| 仕組み | いつ | 見るもの |
| --- | --- | --- |
| dependabot（`.github/dependabot.yml`） | 月次 | cargo の直接依存（2 組）と github-actions（1 組）の更新 PR |
| ci.yml の `deny` ジョブ | PR ごと | cargo-deny の licenses・bans・sources（`deny.toml`） |
| deps.yml | 週次・依存を変える PR・手動 | Rust の依存の勧告・vendor 資産の勧告・アーカイブ用の通知の生成 |

## dependabot の PR

組は 3 つ。PR のタイトルに組の名前が出る。組に入るのは minor・patch の更新だけで、major の
更新は依存ごとに 1 本ずつの PR になる（API の変更で 1 つが壊れても組を止めないため）。
0.x の版上げ（0.29 → 0.30）も major に数えられる。組を閉じて作り直したときは、元の組に
入っていた major の更新は「処理済み」として後回しになり、次の実行まで個別の PR が来ない
（すぐ欲しければ https://github.com/ai-implementer/yuzu/network/updates から手動で実行する）。

- **render-output**（comrak・syntect・two-face）— 本文 HTML が変わりうる。別の PR にしてあるので、
  ほかの組のマージを止めない
- **cargo**（それ以外の cargo の直接依存）
- **actions**（github-actions。フル SHA と版のコメントを一緒に上げる）

wasm-bindgen は ignore してある（下の「手で上げる依存」）。間接依存は dependabot が上げない
（勧告が出たら deps.yml が知らせるので、そのときに `cargo update -p <crate>`）。

### CI の結果から追随作業を見分ける

PR のブランチを取ってきて、落ちた検査ごとに直してから同じブランチへ push する
（dependabot は人が push した PR を以後リベースしなくなるが、問題ない）。

| 落ちた検査・出た差分 | 原因 | やること |
| --- | --- | --- |
| insta スナップショットの差分 | comrak・syntect・two-face 等で本文 HTML・ハイライトが変わった | 差分を目視 → `INSTA_UPDATE=always cargo test -p <crate>` → `git diff` で確認。本文 HTML が変わるなら `yuzu-core/src/cache.rs` の `CACHE_FORMAT_VERSION` を上げる（上げ忘れは CI で検出できない） |
| `third-party-licenses.sh check`: two-face の版 | two-face を上げた | `licenses/README.md` の手順で two-face の一覧を作り直し、スクリプトの `TWO_FACE_VERSION` を更新 |
| `third-party-licenses.sh check`: 検索 wasm の通知が今の依存と合わない | 検索 wasm に入る crate（serde_json・fst・vaporetto 等）の版が Cargo.lock で変わった・増えた・外れた | `scripts/build-search-wasm.sh` で wasm と通知を作り直す（vendor-update スキル。wasm-bindgen-cli・binaryen・cargo-about が要る）。`yuzu build` → `yuzu search` で整合を確かめる。vaporetto の更新はトークナイザが変わるので特に必須 |
| deps.yml の `licenses-notice` | `licenses/about.toml` の clarify の checksum が合わない・crate の本文が無く雛形に戻った | `licenses/README.md`「雛形に戻る crate の扱い」 |
| `deny` の licenses | 許可していないライセンスの crate が入った | 配布してよいライセンスか確かめ、`licenses/about.toml` の `accepted` と `deny.toml` の `allow` の**両方**に足す（`third-party-licenses.sh check` が一致を照合する） |
| `deny` の bans | 禁止した crate（onig・TLS・HTTP クライアント）が入った | feature の指定を見直す。CLAUDE.md の凍結判断に反するので、入れる方向で直さない |
| deps.yml の `advisories` | 上げた版に勧告がある | 下の「勧告が出たとき」 |
| コンパイルエラー | 0.x の破壊的変更（`0.53` → `0.54` など Cargo.toml の要件ごと上がる更新） | 手で直す。すぐ直せないなら PR を閉じ、`dependabot.yml` の `ignore` に版を足して見送る（理由を書く） |
| MSRV のジョブ | 依存の新しい版が MSRV（ワークスペース 1.87・kabosu と tankan 1.85）より新しい rustc を要求した | 依存を上げないか、MSRV を上げる判断をする（README・release.yml・docs も直す。CLAUDE.md） |
| ci.yml の `scaffold actions pins` | actions の PR が `.github/workflows/` だけを上げ、雛形 `crates/yuzu-cli/scaffold/deploy.yml` が古い SHA のまま | 雛形の `uses:` を同じ SHA・版のコメントに揃える（dependabot は `.github/` の外を見ない） |

マージ前に `verify` スキルの一式を通す。

### actions の PR

- `uses:` の SHA と行末のコメント（`# v7.0.0`）がそろっているかを見る
- 同じ action はすべてのワークフローで同じ SHA にする（docs-links.yml・docs.yml・release.yml も）。
  雛形 `crates/yuzu-cli/scaffold/deploy.yml` も同じ SHA に揃える（CI の `scaffold actions pins` が照合する）
- `EmbarkStudios/cargo-deny-action` を上げたら、action の `Dockerfile` の `deny_version` を見て、
  手元の cargo-deny と verify スキルの版の記載をそろえる
- `dtolnay/rust-toolchain` はタグではなくブランチ（`stable`・`1.85` 等）を SHA で固定している。
  どのワークフローも `toolchain:` を明示しているので、どのコミットに上がっても動く

## 勧告が出たとき（deps.yml の失敗）

定期実行の失敗は、ワークフローの cron を最後に編集した人へメールで届く。

### Rust の依存（`advisories`）

1. 出力の `Solution` を見る。`cargo update -p <crate>` で上がるならそれで直す（MSRV に注意。
   上げた版が MSRV より新しい rustc を要求するときは上がらない）
2. 直接依存の feature で入っているだけなら、使っていない feature を外して依存ごと消せないか見る
   （Phase 82 で syntect の `default-fancy` をやめ、plist 経由の quick-xml・time と yaml-rust を外した）
3. 直せず影響もしないなら、`deny.toml` の `[advisories] ignore` に ID と理由・見直す条件を書く

### vendor 資産（`vendor-advisories`）

1. 上流の更新で直るなら vendor-update スキルで更新する（mermaid は 11 系に留めている。
   12 系は tankan の互換対象と合わせて v0.20 以降）
2. 影響しないなら、`scripts/vendor-advisories.sh` 冒頭の `IGNORE` に「GHSA の ID と
   パッケージ@版」の組と理由を書く。版が変われば除外は外れて再び照合される
3. 影響するのに直せないなら、SECURITY.md の対応の目安（修正はパッチ版で 90 日以内）で扱う

## 手で上げる依存

- **wasm-bindgen** — Cargo.toml の `=` 固定と wasm-bindgen-cli の版を一緒に上げ、検索 wasm を
  作り直す（vendor-update スキルの 1）
- **cargo-deny・cargo-about** — cargo-deny は action の版、cargo-about は
  `scripts/third-party-licenses.sh` の `ABOUT_VERSION`。上げたら生成物の差分を確かめる

## 定期実行が止まる条件

公開リポジトリでは、60 日間リポジトリに動きが無いと GitHub が定期実行を止める（docs-links.yml・
deps.yml）。dependabot の月次の PR をマージしていれば動きは続く。止まったら Actions の画面から
有効にし直す。GitHub の Dependabot alerts を有効にしておくと、定期実行が止まっていても
勧告の通知は届く。
