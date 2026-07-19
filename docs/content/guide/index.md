---
title: ガイド
order: 2
description: yuzu のインストールとクイックスタート
---

# クイックスタート

## インストール

[GitHub Releases](https://github.com/ai-implementer/yuzu/releases/latest) から
お使いのプラットフォームのバイナリ（macOS arm64 / x64・Linux x64・Windows x64）を
ダウンロードして展開し、`yuzu` を PATH の通った場所へ置きます。
各リリースには検証用の `SHA256SUMS` も添付されています。

Rust ツールチェイン（1.85 以降）があれば、ソースからもインストールできます:

```bash
cargo install --git https://github.com/ai-implementer/yuzu yuzu-cli
```

## プロジェクトを作る

```bash
yuzu new my-docs
cd my-docs
```

`yuzu new` は次の構成を生成します:

    my-docs/
    ├─ yuzu.jsonc                    # 設定（JSONC: コメント・トレーリングカンマ可）
    ├─ content/                      # Markdown 原稿（ディレクトリ階層 = ナビ階層）
    │  ├─ index.md
    │  └─ guide/getting-started.md
    ├─ public/                       # 静的ファイル（そのまま配信される）
    │  └─ images/yuzu-logo.svg
    ├─ theme/                        # テーマ上書き（同じ相対パスのファイルを置くだけ）
    └─ .github/workflows/deploy.yml  # GitHub Pages 自動デプロイ

## 書く・確認する・出力する

```bash
yuzu dev      # 開発サーバ（監視 + 自動再ビルド + ライブリロード）
yuzu build    # dist/ に静的サイトを出力
yuzu preview  # ビルド済みの dist/ を http://127.0.0.1:5173/ で確認
```

執筆中は `yuzu dev` の 1 コマンドで十分です。`content/` と `theme/` を監視して
自動再ビルドし、WebSocket でブラウザを即リロードします（保存から約 1 秒）。
WebSocket が使えない環境では `yuzu build --watch`（ポーリング式）が退避先です。

> [!NOTE]
> `yuzu build` は常時インクリメンタルです。2 回目以降は変更したページだけを
> 再計算するので、ページ数が増えても再ビルドは高速なままです
> （詳細は[インクリメンタルビルドの内部設計](../development/internals-build.md)）。

## 品質チェックと公開

```bash
yuzu fmt        # Markdown を正規形へ整形（--check で CI 用の差分検出）
yuzu lint --fix # 表記ゆれ（全角英数字・半角カナ・用語・長音符）を自動修正
yuzu check      # lint + リンク切れ + fmt 差分の統合チェック（CI 用）
```

GitHub に push すると、同梱の `.github/workflows/deploy.yml` が
GitHub Pages へ自動デプロイします（リポジトリの Settings \> Pages \> Source を
「GitHub Actions」にするだけ。詳細は[配信とデプロイ](deploy.md)）。

## 次に読む

- [執筆の基本](writing.md) — ページ・ナビ・frontmatter・Admonition・画像
- [コードと数式](code-and-math.md) — ハイライトと KaTeX
- [図（Mermaid / SSR）](diagrams.md) — ビルド時 SVG 化される 9 図種
- [API 仕様の描画](api-spec.md) — OpenAPI / JSON Schema
- [全文検索](search.md) — フレーズ検索・同義語・コード検索
- [LLM 連携](llms.md) — llms.txt とページ Markdown 配信
- [品質チェック](quality.md) — fmt / lint / check
- [配信とデプロイ](deploy.md) — baseUrl・GitHub Pages・テーマ上書き
