# yuzu 🍊

[![CI](https://github.com/ai-implementer/yuzu/actions/workflows/ci.yml/badge.svg)](https://github.com/ai-implementer/yuzu/actions/workflows/ci.yml)
![MSRV](https://img.shields.io/badge/MSRV-1.85-orange)
![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue)

Markdown で書いた設計書を、プロダクション品質の静的 HTML ドキュメントサイトに
変換する **Rust 製のドキュメント生成ツール**。

## クイックスタート

```bash
cargo install --path crates/yuzu-cli   # または cargo run -p yuzu-cli --

yuzu new my-docs
cd my-docs
yuzu dev            # 開発サーバ（監視 + 自動再ビルド + WS ライブリロード）
yuzu build          # dist/ に静的サイトを出力
yuzu preview        # http://127.0.0.1:5173/ で確認
yuzu build --watch  # ポーリング式の簡易リロード（WS が使えない環境向け）
yuzu fmt            # Markdown を正規形へ整形（--check で CI 用の差分検出）
yuzu check          # lint + リンク切れ + fmt 差分の統合チェック（CI 用）
```

## できること

- `content/**/*.md`（GFM: 表・打ち消し線・autolink・タスクリスト）→ テーマ HTML
- **左サイドバーナビ**（ディレクトリ階層 ＝ ナビ階層。frontmatter `title` / `order` で制御）
- **ページ内 TOC**（h2/h3、アンカーは本文見出しと同期）
- **ダークモード切替**（localStorage 保存、OS 設定に追従、FOUC なし）
- **シンタックスハイライト**: syntect をビルド時に実行し **CSS クラス出力**
  （クライアント JS ゼロ、ライト/ダーク両対応）
- **Mermaid**: ` ```mermaid ` ブロック → 既定は同梱 mermaid.js でクライアント描画。
  **`backend: "ssr"` にすると自作の [tankan](crates/tankan/) がビルド時に SVG 化**
  （sequence・flowchart・state・ER・gantt 対応。未対応図種は自動でクライアント描画にフォールバックし、
  フォールバックが発生したページだけ mermaid.js を読み込む。SSR の SVG は
  CSS 変数経由でダークモードに**再描画なしで追従**）
- **日本語全文検索**（自前 BM25）: vaporetto の分かち書き＋fst の編集距離 1 の
  タイポトレランス。インデックスは静的ファイル（`dist/_search/`）で、ブラウザは
  wasm ＋ 2 段フェッチ（Pagefind 型）— サーバ不要、CDN/静的ホストだけで動く。
  `yuzu search <クエリ>` で同じエンジンをターミナルからも使える
- **llms.txt / llms-full.txt の自動生成**（[llms.txt 仕様](https://llmstxt.org/)準拠）:
  `dist/` 直下にリンク索引と全ページの正規化 Markdown 連結を出力。
  frontmatter `llms: false` で個別除外、`yuzu llms [--full]` で dist なしでも標準出力へ
- **fmt / lint / check**: `yuzu fmt` は AST 経由の決定的整形（frontmatter は
  バイト温存・冪等）。`yuzu lint` は文書規約（h1 重複・見出しレベル飛び・
  frontmatter 未知キー）、`yuzu check` はさらに**内部リンク・アンカー切れを
  行番号付きで報告**する統合 CI コマンド（終了コードは 0 = 違反なし / 1 = 違反あり / 2 = 実行エラー）
- `public/` の静的物パススルー（画像等）
- **base path 対応**: `baseUrl: "/docs/"` でリンク・アセット参照をサブパスへ解決（社内リバプロ配下の配信を想定）
- **`yuzu dev`**: notify で `content/`・`theme/` を監視 → 自動再ビルド →
  **WebSocket ライブリロード**（`/__livereload`。md 編集から約 1 秒で自動更新、
  サーバ再起動後はブラウザが自動再接続してリロード）。`dev.open: true` で起動時にブラウザを開く
- `yuzu build --watch`: 同じ監視ビルドを簡易オートリフレッシュ（build_id ポーリング）で。
  WS が通らない環境向けの退避先
- frontmatter（YAML）: `title` / `order` / `draft` / `description` / `llms`
- テーマ上書き: プロジェクトの `theme/` に同じ相対パスのファイルを置くだけ

## 設定（`yuzu.jsonc`）

JSONC（コメント可）。解決済み設定は `.yuzu/settings.json` に書き出されます。
`yuzu.jsonc` のあるディレクトリがプロジェクトルートです（cwd から上方向に探索）。

```jsonc
{
  "site": { "title": "My Docs", "description": "...", "lang": "ja", "baseUrl": "/docs/" },
  "input": { "dir": "content", "ignore": ["**/_drafts/**"] },
  "output": { "dir": "dist", "clean": true },
  "theme": { "name": "default", "dark": true },
  "nav": { "auto": true },
  "markdown": {
    "gfm": true,
    "highlight": { "enabled": true, "themeLight": "InspiredGitHub", "themeDark": "base16-ocean.dark" },
    "mermaid": { "enabled": true, "backend": "client" } // "ssr" = tankan でビルド時 SVG
  },
  "search": {
    "enabled": true,
    // "dictionary": "models/custom.model.zst", // vaporetto モデルの差し替え
    "typoTolerance": { "enabled": true, "maxEdits": 1 },
    "shard": { "maxTermsPerShard": 16384 }
  },
  "llms": { "enabled": true, "full": true }, // llms.txt / llms-full.txt
  "build": { "baseUrl": "/docs/" }, // site.baseUrl より優先
  "dev": { "host": "127.0.0.1", "port": 5173, "liveReload": true, "open": false }
}
```

## ロードマップ

| Phase | 内容 | 状態 |
|---|---|---|
| **1 build（v0.1）** | build ＋ テーマ HTML ＋ ナビ/TOC/ダークモード ＋ Mermaid(client) ＋ `build --watch`/`preview` | ✅ |
| **2 dev** | axum ＋ WebSocket のフルライブリロード開発サーバ（`yuzu dev`） | ✅ |
| **3 検索** | 自前 BM25 転置インデックス ＋ 日本語分かち書き（vaporetto）＋ タイポトレランス ＋ Wasm クエリエンジン（Pagefind 型の静的配信） | ✅ |
| **4 llms.txt** | `llms.txt`（リンク索引）/ `llms-full.txt`（正規化 md 連結）の自動生成 | ✅ |
| **5 図表 SSR** | Mermaid 互換描画ライブラリ **tankan** を自作して SSR 化（`yuzu-*` 非依存の汎用設計、必要時に別リポ/crates.io へ分離）。sequence / flowchart / state / ER / gantt 対応、wasm32 ビルドは CI で担保 | ✅ |
| **6 fmt / lint / check** | `fmt`（AST → 決定的な正規化 md、frontmatter バイト温存）/ `lint`（文書規約）/ `check`（lint ＋ 内部リンク・アンカー検査 ＋ fmt 差分。sourcepos 付き診断） | ✅ |

## 凍結した設計判断

Web 調査込みで確定済み。差し替えないこと。

| 領域 | 採用 | 要点 |
|---|---|---|
| Markdown パース | **comrak** | GFM 完備・可変 AST・sourcepos（将来の Linter 用）・`format_commonmark`（将来の Formatter 用）。frontmatter は YAML（front matter extension）。パーサは `yuzu-core` 内部に隠蔽し、公開 API はパーサ非依存 |
| テンプレート | **minijinja** | ランタイム解釈 ＝ 将来 dev でテンプレのホットリロードが可能 |
| ハイライト | **syntect**（`fancy-regex`） | pure-Rust（onig 非依存）。`ClassedHTMLGenerator` で **CSS クラス出力**、ビルド時実行 |
| CLI | **clap（derive）** | `new` / `build` / `preview` / `dev` / `search` / `llms` / `fmt` / `lint` / `check`（終了コード: 0 = 成功 / 1 = 違反 / 2 = エラー） |
| 設定 | **serde ＋ JSONC** | `yuzu.jsonc` → 解決形 `.yuzu/settings.json`。上方向探索でルート確定 |
| テーマ同梱 | **rust-embed** | デフォルトテーマをバイナリ埋め込み、`theme/` でファイル単位の上書き |
| Mermaid | **mermaid.js クライアント描画**（v0.1） | 自作 SSR（tankan、`backend: "ssr"`）は Phase 5 で実装済み。既定は client のまま |
| dev サーバ | **axum ＋ notify（＋debouncer）＋ WebSocket** | `yuzu dev` は `/__livereload` への WS push でリロード。preview は純粋な静的配信 |

依存方向（凍結）:
`yuzu-cli → {server, render, index, core, config}` / `render, index → core` /
`search-wasm ↔ index-format`。逆方向依存は作らない。

## ワークスペース構成

```
crates/
├─ tankan             # Mermaid 互換描画（テキスト → SVG。yuzu 非依存の汎用ライブラリ）
├─ yuzu-core          # comrak パース → Document/サイトモデル（nav・TOC・slug・sourcepos）
├─ yuzu-render        # サイトモデル → HTML（minijinja・syntect・mermaid 変換・base path 解決）
├─ yuzu-config        # yuzu.jsonc（JSONC）の探索・スキーマ・解決
├─ yuzu-theme         # デフォルトテーマ（rust-embed: テンプレ + CSS + JS + mermaid.js）
├─ yuzu-cli           # CLI（bin: yuzu）
├─ yuzu-server        # preview/watch 用最小静的サーバ + notify 監視
├─ yuzu-index         # 検索インデクサ（BM25 転置・シャーディング・wasm 成果物の同梱）
├─ yuzu-index-format  # 索引フォーマット共有型（native/wasm でトークナイザを共有）
└─ yuzu-search-wasm   # クライアント検索クエリエンジン（cdylib）
```

## 開発

```bash
cargo build
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace          # insta スナップショットテストを含む
cargo fmt --all
```

- **MSRV**: 1.85（edition 2024）
- **スナップショット**: レンダリング結果は `insta` で回帰検証。syntect のバージョン更新で
  ハイライト HTML の差分が出た場合は `cargo insta review` で確認のうえ更新する
- **rust-embed の注意**: debug ビルドはテーマをファイルシステムから読む
  （テーマ編集が再コンパイルなしで反映）。debug バイナリ単体を別マシンへコピーすると
  アセットを見失う。リリースビルドは常に埋め込み
- **mermaid.min.js**（約 3.4MB）は `crates/yuzu-theme/assets/static/vendor/` に同梱
  （`scripts/vendor-mermaid.sh` で更新）。`backend: "ssr"` ならフォールバック発生
  ページ以外では配信されない（同梱自体はフォールバック用に継続）
- 既知の制限（v0.1）: ダークモード切替後の Mermaid 図はリロードで再描画
  （client 描画のみ。SSR の SVG は CSS 変数でそのまま追従する）

### 検索まわりの実装メモ

- **トークナイザ整合が最重要制約**: index 時（ネイティブ）と query 時（wasm）で
  同一コード（`yuzu-index-format`）＋同一モデルバイト（`dist/_search/model.zst`）を使う。
  `yuzu search` はブラウザと同じエンジンを通るので整合の検証にも使える
- **vaporetto モデル**（`bccwj-suw_c1.0`、圧縮 372KB、**MIT OR Apache-2.0**）は
  `crates/yuzu-index-format/assets/model/` に vendor（`scripts/vendor-vaporetto-model.sh` で更新）。
  ブラウザは初回検索時にモデルを遅延ダウンロードする
- **wasm 成果物**（453KB）は `crates/yuzu-index/assets/search/` に vendor
  （`scripts/build-search-wasm.sh` で更新。要 wasm32 target ＋ wasm-bindgen-cli ＋ binaryen。
  CLI は crate の `wasm-bindgen = "=x.y.z"` と完全同一バージョンにすること）
- 設計ノートの「rkyv で直列化」は**不採用**: postings は元々 delta+varint の自前設計で
  rkyv の出番がなく、fragment は JS が直接読むため JSON が自然（wasm サイズ・依存とも有利）
- 検索 UI の確認は `yuzu preview` / `yuzu dev` 経由で行う（`file://` では fetch が動かない）

### llms.txt まわりの実装メモ

- llms-full.txt の本文は原文ではなく **comrak `format_commonmark` による正規形**
  （見出し ATX 化・箇条書き `-` 統一・裸 URL の `<url>` 化等）。`yuzu fmt`（Phase 6）と同じ基盤
- llms.txt のリンクは `baseUrl` 由来。**公開サイトでは `build.baseUrl` にフル URL
  （`https://…/docs/`）を設定すると絶対 URL になる**（llms.txt の慣行に合う）
- `public/llms.txt` を手書きで置くと生成版を上書きできる（テーマ上書きと同じ思想）
- 将来候補（Phase 4.5）: ページ単位の正規化 md を `dist/<path>.md` に配信して
  llms.txt から `.md` リンクにする（vitepress/docusaurus プラグインで優勢の形式）

## ライセンス

MIT または Apache-2.0 のデュアルライセンス（お好きな方でどうぞ）。

- [LICENSE-MIT](LICENSE-MIT)
- [LICENSE-APACHE](LICENSE-APACHE)
