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

## 機能別の設定メモ

- **検索 UI**: `file://` では fetch が動かないため、必ず preview / dev 経由で確認。
- **Mermaid SSR**: 生成された yuzu.jsonc では `"backend": "ssr"` が**コメントアウト行**として入っている。有効化は `"enabled": true` にカンマを足し、`// "backend": "ssr"` の `//` を外す。SSR 成功の確認は「対象ページの `<svg` 数」と「vendor/mermaid のロードが 0 箇所」。
- **ライブリロード**: `yuzu dev` は WS（/__livereload）。md 保存から約 1 秒で反映。

## 確認観点チェックリスト

- ダークモード切替（◐ ボタン）でテキスト・SVG・ハイライトが追従するか
- 右 TOC（幅 >72rem）とモバイル TOC（≤72rem の `<details>`）の両方
- `build.baseUrl` にサブパス（例 `/docs/`）を設定してリンク・アセット参照が壊れないか
- JS 無効時に表示が崩れないか（テーマ JS はすべてプログレッシブエンハンスメント）
