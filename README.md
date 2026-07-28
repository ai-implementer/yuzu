# yuzu 🍊

[![CI](https://github.com/ai-implementer/yuzu/actions/workflows/ci.yml/badge.svg)](https://github.com/ai-implementer/yuzu/actions/workflows/ci.yml)
![MSRV](https://img.shields.io/badge/MSRV-1.85-orange)
![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue)

Markdown で書いた設計書を、プロダクション品質の静的 HTML ドキュメントサイトに
変換する **Rust 製のドキュメント生成ツール**。

**ドキュメントサイト**: https://ai-implementer.github.io/yuzu/ — yuzu 自身で
ビルドして GitHub Pages へ公開している実例サイト（原稿は [docs/](docs/)、
デプロイは [.github/workflows/docs.yml](.github/workflows/docs.yml)）。

## できること

- **書くことに集中できる**: `content/**/*.md` を置くだけでナビ・目次・前後ページ
  リンク・パンくずが付く。`yuzu dev` は保存から約 1 秒で自動リロード
- **設計書のための表現力**: シンタックスハイライト・数式（KaTeX）・Mermaid 互換の図
  （sequence / flowchart / class / state / ER / gantt / pie / mindmap / timeline の
  9 図種をビルド時に SVG 化）・OpenAPI / JSON Schema の静的レンダリング
- **実ソースの埋め込み**: コードブロックに `file="src/api.rs" lines=10-25` と書くと
  実ファイルを取り込む（設計書とコードの乖離を防ぐ）
- **日本語のための検索**: 分かち書き＋BM25 の全文検索が静的ホスティングだけで動く。
  誤字に寛容で、フレーズ検索・同義語展開・コードブロック検索にも対応
- **品質を保つ道具**: `yuzu fmt`（決定的整形）・`yuzu lint`（表記ゆれの検出と自動修正）・
  `yuzu check`（リンク切れ検査）。診断は `--format json` / `github` で CI にそのまま乗る
- **LLM 連携**: llms.txt / llms-full.txt と、ページ単位の Markdown 配信・コピーボタン
- **速い**: インクリメンタルビルド＋ページ並列化で、再ビルドは変更ページ分だけ
- **クライアント JS はほぼゼロ**: 図もコードも API 仕様もビルド時に HTML 化する
  （検索とテーマ切替だけが JS。無効でも本文は読める）

## クイックスタート

[GitHub Releases](https://github.com/ai-implementer/yuzu/releases/latest) の
プラットフォーム別バイナリ（macOS arm64/x64・Linux x64・Windows x64）を
PATH の通った場所へ置くか、Rust 1.85 以降でソースからインストールする:

```bash
cargo install --git https://github.com/ai-implementer/yuzu yuzu-cli
# リポジトリ内での開発中は: cargo install --path crates/yuzu-cli

yuzu new my-docs
cd my-docs
yuzu dev            # 開発サーバ（監視 + 自動再ビルド + WS ライブリロード）
yuzu build          # dist/ に静的サイトを出力
yuzu preview        # http://127.0.0.1:5173/ で確認
yuzu fmt            # Markdown を正規形へ整形（--check で差分検出・--diff で差分表示）
yuzu lint --fix     # 表記ゆれ（全角英数字・半角カナ・用語・長音符）を自動修正
yuzu check          # lint + リンク切れ + fmt 差分の統合チェック（CI 用）
# GitHub に push すると Pages へ自動デプロイ（.github/workflows/deploy.yml 同梱。
# リポジトリの Settings > Pages > Source を「GitHub Actions」にするだけ）
```

設定はプロジェクトルートの `yuzu.jsonc` 1 枚（すべてのキーが省略可能）。
終了コードは全コマンド共通で **0 = 成功 / 1 = 違反あり / 2 = 実行エラー**。

## ドキュメント

| 知りたいこと | 参照先 |
| --- | --- |
| インストールから公開まで | [ガイド](https://ai-implementer.github.io/yuzu/guide/) |
| 記法（Admonition・図表番号・折りたたみ・インクルード） | [執筆の基本](https://ai-implementer.github.io/yuzu/guide/writing/) |
| 設定キー・CLI・診断ルールの一覧 | [リファレンス](https://ai-implementer.github.io/yuzu/reference/) |
| 内部設計・凍結した設計判断・ワークスペース構成 | [開発](https://ai-implementer.github.io/yuzu/development/) |
| 開発計画とこれまでの内訳 | [ROADMAP.md](ROADMAP.md) |

## 開発

```bash
cargo build --workspace
cargo test --workspace                                  # insta スナップショットを含む
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
```

クレート構成・依存方向・凍結した設計判断は
[開発ドキュメント](https://ai-implementer.github.io/yuzu/development/)にまとまっている。

## ライセンス

MIT または Apache-2.0 のデュアルライセンス（お好きな方でどうぞ）。

- [LICENSE-MIT](LICENSE-MIT)
- [LICENSE-APACHE](LICENSE-APACHE)
