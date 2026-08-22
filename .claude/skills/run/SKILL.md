---
name: run
description: yuzu をサンプルプロジェクトで起動して変更を実機確認する。テーマ（テンプレート / CSS / JS）・scaffold・検索 UI・SSR 図の変更後に、実際の配信で動作を見るときに使う。
---

# yuzu 実機起動手順

## 準備

```bash
cargo build -p yuzu-cli          # scaffold・テンプレート変更は再ビルド必須
./target/debug/yuzu new "<scratchpad>/run-docs"
cd "<scratchpad>/run-docs" && <repo>/target/debug/yuzu build
```

- テーマアセット（debug ビルド）は rust-embed が FS から読むため、テーマ編集は CLI 再コンパイル不要。ただし**サイトの再ビルドは必要**（`yuzu build` し直すか、`yuzu dev` なら content を touch）。
- 検証用に「見出しの多いページ（h2×5・h3×5 程度）」「見出しなしページ」を content に足しておくと TOC・ナビの確認がしやすい。

## 起動（ポートの罠に注意）

**罠: 既定ポート 5173 は別プロジェクト（order-system-design 等）の yuzu が使用中のことがある。** 先に確認し、使用中なら別ポートを使う。**他プロジェクトの稼働中プロセスを kill しないこと。**

```bash
lsof -nP -i :5173 | head -3        # 使用中か確認（サンドボックス外）
<repo>/target/debug/yuzu preview --port 5199   # TCP バインドはサンドボックス外で
```

バックグラウンド起動 → curl / ブラウザで確認 → 自分が起動したプロセスだけを PID 指定で停止する。

よく使うオプション（`preview` / `dev` 共通）:

- `--host 0.0.0.0` — 開発コンテナ内で起動してホストのブラウザから見るときに必要
  （既定は 127.0.0.1 バインドなので publish しても届かない）
- `--drafts` — `draft: true` のページも出す。draft プレビューの確認手段
- `dev --force` — キャッシュを捨てて起動。キャッシュ起因を疑うときに

## 機能別の設定メモ

- **検索 UI**: `file://` では fetch が動かないため、必ず preview / dev 経由で確認。
- **Mermaid SSR**: 生成された yuzu.toml では `[markdown.mermaid]` の `# backend = "ssr"` が**コメントアウト行**として入っている。有効化は行頭の `# ` を外す。SSR 成功の確認は「対象ページの `<svg` 数」と「vendor/mermaid のロードが 0 箇所」。
- **ライブリロード**: `yuzu dev` は WS（/__livereload）。md 保存から約 1 秒で反映。

## 確認観点チェックリスト

- ダークモード切替（◐ ボタン）でテキスト・SVG・ハイライトが追従するか
- 右 TOC（幅 >72rem）とモバイル TOC（≤72rem の `<details>`）の両方
- `build.base_url` にサブパス（例 `/docs/`）を設定してリンク・アセット参照が壊れないか
- JS 無効時に表示が崩れないか（テーマ JS はすべてプログレッシブエンハンスメント）
- **折りたたみの自動展開**: 閉じた `<details>` の中にある見出しへ検索結果・目次・図表参照から
  飛んだとき、祖先が開いて該当箇所が見えるか
- **サイドバーのスクロール位置維持**: 下の方までスクロールしてリンクを踏み、遷移後も位置が
  保たれるか（**トップで描画されてから飛ぶちらつきが無いこと**。Slow 3G で差が出やすい）。
  ウィンドウを低くしてサイドバーが溢れる状態で見る。狭幅（≤50rem）では働かないのが正
- **図表番号**: `markdown.crossref.numbering: "site"` にしたとき、サイドバー表示順の
  通し番号になり、先行ページに図を足すと後続の番号が追随するか
