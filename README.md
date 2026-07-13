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
# GitHub に push すると Pages へ自動デプロイ（.github/workflows/deploy.yml 同梱。
# リポジトリの Settings > Pages > Source を「GitHub Actions」にするだけ）
```

## できること

### サイト生成

- `content/**/*.md`（GFM: 表・打ち消し線・autolink・タスクリスト）→ テーマ HTML
- **左サイドバーナビ**（ディレクトリ階層 ＝ ナビ階層。frontmatter `title` / `order` で制御）
- **ページ内 TOC**（h2/h3、アンカーは本文見出しと同期）
- **前/次ページリンク＋階層パンくず**（サイドバー表示順で全ページを連結。
  トップページではパンくず非表示。消したい場合はテーマ上書きで partial を空にする）
- **ダークモード切替**（localStorage 保存、OS 設定に追従、FOUC なし）
- **Admonition / 脚注**: GitHub 互換の `> [!NOTE]`〜`> [!CAUTION]`（5 種・`> [!NOTE] タイトル` で上書き可）と
  `[^1]` 脚注。`yuzu fmt` / llms-full.txt は脚注定義の位置・未参照定義を温存する
- frontmatter（YAML）: `title` / `order` / `draft` / `description` / `llms`

### コードと図

- **シンタックスハイライト**: syntect をビルド時に実行し **CSS クラス出力**
  （クライアント JS ゼロ、ライト/ダーク両対応）
- **コピーボタン**: コードブロック右上からワンクリックコピー
  （Clipboard API のプログレッシブエンハンスメント。JS 無効・非 https では現れない）
- **数式**: GitHub 互換の `$...$` / `$$...$$` / `` $`...`$ `` / ` ```math `（`$100`
  のような通貨表記は数式にならない）。同梱 KaTeX でクライアント描画し、
  **数式のあるページだけ** CSS/JS（約 600KB）を読み込む。`markdown.math.enabled: false` で無効化
- **Mermaid**: ` ```mermaid ` ブロック → 既定は同梱 mermaid.js でクライアント描画。
  **`backend: "ssr"` にすると自作の [tankan](crates/tankan/) がビルド時に SVG 化**
  （sequence・flowchart・class・state・ER・gantt・pie 対応。未対応図種は自動でクライアント描画にフォールバックし、
  フォールバックが発生したページだけ mermaid.js を読み込む。SSR の SVG は
  CSS 変数経由でダークモードに**再描画なしで追従**）

### 検索と LLM 連携

- **日本語全文検索**（自前 BM25）: vaporetto の分かち書き＋fst の編集距離 1 の
  タイポトレランス。インデックスは静的ファイル（`dist/_search/`）で、ブラウザは
  wasm ＋ 2 段フェッチ（Pagefind 型）— サーバ不要、CDN/静的ホストだけで動く。
  **検索はセクション（h2/h3）単位**で「ページ › 見出し」の結果から `#アンカー` へ
  直接ジャンプ。抜粋はクエリ一致箇所周辺を動的生成し、分かち書き単位でハイライト。
  `yuzu search <クエリ>` で同じエンジンをターミナルからも使える
- **llms.txt / llms-full.txt の自動生成**（[llms.txt 仕様](https://llmstxt.org/)準拠）:
  `dist/` 直下にリンク索引と全ページの正規化 Markdown 連結を出力。
  frontmatter `llms: false` で個別除外、`yuzu llms [--full]` で dist なしでも標準出力へ
- **ページ単位 Markdown の配信とコピー**: 各ページの原文 Markdown を
  `dist/<route>.md` に配信（llms.txt のリンク先も `.md`）。ページ右上の
  「**Markdown をコピー**」ボタンでそのまま LLM に貼れる（`.md` を開くリンク付き。
  プログレッシブエンハンスメント — JS 無効時は非表示）

### 執筆ワークフロー

- **`yuzu dev`**: notify で `content/`・`theme/` を監視 → 自動再ビルド →
  **WebSocket ライブリロード**（`/__livereload`。md 編集から約 1 秒で自動更新、
  サーバ再起動後はブラウザが自動再接続してリロード）。`dev.open: true` で起動時にブラウザを開く
- `yuzu build --watch`: 同じ監視ビルドを簡易オートリフレッシュ（build_id ポーリング）で。
  WS が通らない環境向けの退避先
- **draft プレビュー**: frontmatter `draft: true` のページは通常ビルドから除外されるが、
  `yuzu dev --drafts` / `yuzu build --drafts` で**バナー付きで**確認できる
  （通常ビルドに戻すと draft の出力は自動掃除される）
- **インクリメンタルビルド**: `yuzu build` / `dev` は常時インクリメンタル
  （`.yuzu/cache/` にページ単位キャッシュ）。未変更ページはパース・ハイライト・
  トークナイズをスキップし、出力は内容一致なら書き込まない（mtime 温存）。
  削除ページの古い出力はマニフェスト差分で自動掃除。設定変更・yuzu 更新時は
  自動で全再計算に縮退する。`--force` でキャッシュを破棄してフルビルド
  （`.yuzu/cache/` はいつ消しても安全）
- **fmt / lint / check**: `yuzu fmt` は AST 経由の決定的整形（frontmatter は
  バイト温存・冪等）。`yuzu lint` は文書規約（h1 重複・見出しレベル飛び・
  frontmatter 未知キー）に加え、**用語統一チェック**（`lint.terms` の辞書で
  「サーバ/サーバー」のような表記ゆれを行番号付きで検出。コード・URL は対象外）。
  `yuzu check` はさらに**内部リンク・アンカー切れを行番号付きで報告**する
  統合 CI コマンド（終了コードは 0 = 違反なし / 1 = 違反あり / 2 = 実行エラー）

### 配信とカスタマイズ

- `public/` の静的物パススルー（画像等）
- **base path 対応**: `baseUrl: "/docs/"` でリンク・アセット参照をサブパスへ解決（社内リバプロ配下の配信を想定）。
  `yuzu build --base-url` で設定より優先して上書きできる（CI からの注入用）
- **GitHub Pages デプロイ雛形**: `yuzu new` が `.github/workflows/deploy.yml` を同梱。
  `configure-pages` の base_path を `--base-url` へ渡すため、project pages の
  サブパス（`/<リポジトリ名>/`）も設定なしで正しく配信される
- テーマ上書き: プロジェクトの `theme/` に同じ相対パスのファイルを置くだけ

## 設定（`yuzu.jsonc`）

JSONC（コメント可）。解決済み設定は `.yuzu/settings.json` に書き出されます。
`yuzu.jsonc` のあるディレクトリがプロジェクトルートです（cwd から上方向に探索）。

```jsonc
{
  "site": { "title": "My Docs", "description": "...", "lang": "ja", "baseUrl": "/docs/", "logo": "/images/logo.svg" }, // logo はヘッダーのロゴ画像（未指定なら 🍊）
  "input": { "dir": "content", "ignore": ["**/_drafts/**"] },
  "output": { "dir": "dist", "clean": true },
  "theme": {
    "name": "default",
    "dark": true,
    // テーマ CSS 変数の上書き（キーは -- 省略可。変数名は theme.css の :root を参照）
    "cssVars": { "accent": "#0a6cff" },
    "cssVarsDark": { "accent": "#7fb2ff" } // ダークモード時のみの上書き
  },
  "nav": { "auto": true },
  "markdown": {
    "gfm": true,
    "highlight": { "enabled": true, "themeLight": "InspiredGitHub", "themeDark": "base16-ocean.dark" },
    "mermaid": { "enabled": true, "backend": "client" } // "ssr" = tankan でビルド時 SVG
  },
  "lint": {
    "maxDirectoryDepth": 1, // content 配下のディレクトリ階層を制限（直下 = 0。未設定なら無制限）
    // 用語統一の辞書（正しい表記 → ゆれ表記）。本文・見出しのゆれを行番号付きで検出
    "terms": { "サーバー": ["サーバ"], "ユーザー": ["ユーザ"] }
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

v0.1（Phase 1〜6: build / dev サーバ / 日本語検索 / llms.txt / tankan SSR / fmt・lint・check）、
v0.2（Phase 7〜12: 執筆表現 / 数式 / ページナビ / 検索セクション単位化 / デプロイ雛形 / インクリメンタルビルド）は完了・リリース済み。

以下の Phase 13〜18 がすべて完了した時点で **v0.3** としてリリースする。  
実際の設計書運用（dogfooding）と並行して進め、Phase は価値と実装コスト・依存関係の順に並べている（着手時に個別に設計する）。

| Phase | 内容 | 状態 |
|---|---|---|
| **13 執筆の即効改善** | draft プレビュー（`dev --drafts` / `build --drafts` で下書きをバナー付き表示、通常ビルドに戻すと出力は自動掃除）・Mermaid client 描画のダークモード切替時の再描画（既知の制限の解消）・テーマ CSS 変数の設定化（`theme.cssVars` / `cssVarsDark`。値の検証込み） | ✅ |
| **14 ページ単位 .md 配信とページコピー** | 各ページの原文 Markdown を `dist/<route>.md` に配信して llms.txt を `.md` リンク化（vitepress / docusaurus プラグインで優勢の形式）＋各ページに「Markdown をコピー」ボタンと `.md` リンク（fetch → クリップボード。コードコピーと同じプログレッシブエンハンスメント） | ✅ |
| **15 日本語 lint: 用語統一** | `lint.terms` のプロジェクト用語辞書による用語統一チェック（`term-variant`）を `yuzu lint` / `check` に統合。本文・見出し・リンクラベルを行番号・列番号付きで報告し、コード・URL・正表記の部分一致は対象外。組み込みルール（全角/半角等）は実運用の需要を見て拡張 | ✅ |
| **16 tankan 図種追加** | 設計書頻出の **class 図**（3 区画ボックス・関係 8 種・多重度・ジェネリクス・アノテーション）と **pie**（showData・凡例・CSS 変数パレット）を SSR 対応。corpus 13 本＋スナップショット＋wasm32 担保 | ✅ |
| **17 git 連携メタ** | 最終更新日（git log 由来）・「このページを編集」リンク。git が無い環境では出さない縮退込み | ⬜ |
| **18 dogfooding 改善** | Phase 13〜17 と並行した実運用で溜まった不満の一括解消（バッファ枠）。v0.3 の締めとして実施 | ⬜ |

v0.4 以降の候補（このリリースではやらない）: VS Code 拡張（wasm プレビュー）・OpenAPI / JSON Schema レンダリング・ドキュメントバージョニング / i18n・検索辞書の高精度化（vibrato / lindera）・crates.io 公開とバイナリ配布・tankan の分離公開。

<details>
<summary>完了済み: v0.2（Phase 7〜12）の内訳</summary>

| Phase | 内容 | 状態 |
|---|---|---|
| **7 執筆表現** | Admonition（`> [!NOTE]`、comrak alerts 拡張＋テーマ CSS）・脚注（footnotes 拡張）・コードブロックのコピーボタン（プログレッシブエンハンスメント JS）。fmt / llms.txt との整合（format_commonmark の出力確認）込み | ✅ |
| **8 数式** | comrak math（`$...$` / `$$...$$`）→ KaTeX 描画。クライアント描画か SSR かの設計判断・vendor 資産の同梱方針を含む | ✅ |
| **9 ページナビ** | 前/次ページリンク（nav 順から導出）＋階層パンくず。テンプレート＋nav モデルの拡張 | ✅ |
| **10 検索セクション単位化** | fragment を見出し単位に分割して `#アンカー` へ直接ジャンプ＋クエリ一致箇所周辺の動的抜粋（現在はページ先頭・固定冒頭 160 字）。index フォーマット変更のため wasm/native トークナイザ整合制約に注意 | ✅ |
| **11 デプロイ雛形** | GitHub Pages デプロイ用 Actions ワークフローを `yuzu new` の scaffold に同梱（baseUrl 設定の導線込み） | ✅ |
| **12 インクリメンタルビルド** | `.yuzu/cache/` のページ単位キャッシュで build / dev の再ビルドを短縮（常時有効・`--force` で全再計算）。未変更出力は書き込みスキップ（mtime 温存）＋削除ページの孤児出力をマニフェスト差分で掃除 | ✅ |

</details>

## 凍結した設計判断

Web 調査込みで確定済み。差し替えないこと。

| 領域 | 採用 | 要点 |
|---|---|---|
| Markdown パース | **comrak** | GFM 完備・可変 AST・sourcepos（将来の Linter 用）・`format_commonmark`（将来の Formatter 用）。frontmatter は YAML（front matter extension）。パーサは `yuzu-core` 内部に隠蔽し、公開 API はパーサ非依存 |
| テンプレート | **minijinja** | ランタイム解釈 ＝ 将来 dev でテンプレのホットリロードが可能 |
| ハイライト | **syntect**（`fancy-regex`）＋ **two-face** 拡張構文セット | pure-Rust（onig 非依存）。`ClassedHTMLGenerator` で **CSS クラス出力**、ビルド時実行。two-face（bat のアセット由来）で TypeScript/TSX/TOML/Dockerfile 等を補完 |
| CLI | **clap（derive）** | `new` / `build` / `preview` / `dev` / `search` / `llms` / `fmt` / `lint` / `check`（終了コード: 0 = 成功 / 1 = 違反 / 2 = エラー） |
| 設定 | **serde ＋ JSONC** | `yuzu.jsonc` → 解決形 `.yuzu/settings.json`。上方向探索でルート確定 |
| テーマ同梱 | **rust-embed** | デフォルトテーマをバイナリ埋め込み、`theme/` でファイル単位の上書き |
| Mermaid | **mermaid.js クライアント描画**（v0.1） | 自作 SSR（tankan、`backend: "ssr"`）は Phase 5 で実装済み。既定は client のまま |
| dev サーバ | **axum ＋ notify（＋debouncer）＋ WebSocket** | `yuzu dev` は `/__livereload` への WS push でリロード。preview は純粋な静的配信 |

依存方向（凍結。逆方向依存は作らない）:

```
yuzu-cli → {yuzu-server, yuzu-render, yuzu-index, yuzu-core, yuzu-config}
yuzu-render → yuzu-core, tankan     yuzu-index → yuzu-core
yuzu-search-wasm ↔ yuzu-index-format（native/wasm でトークナイザ共有）
tankan は yuzu-* 非依存の汎用ライブラリ（将来 crates.io へ分離可能な設計を維持）
```

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
- **コンテナ開発環境**: CI 相当の Linux 環境（stable + wasm32 + cargo-insta）を
  ホストを汚さず使える。mac（apple container）は `scripts/dev-container.sh build` →
  `up` → `shell`、Docker / VS Code は `.devcontainer/`（Reopen in Container）。
  移行手順・落とし穴は [.devcontainer/README.md](.devcontainer/README.md)
- **スナップショット**: レンダリング結果は `insta` で回帰検証。syntect のバージョン更新で
  ハイライト HTML の差分が出た場合は `cargo insta review` で確認のうえ更新する
- **rust-embed の注意**: debug ビルドはテーマをファイルシステムから読む
  （テーマ編集が再コンパイルなしで反映）。debug バイナリ単体を別マシンへコピーすると
  アセットを見失う。リリースビルドは常に埋め込み
- **mermaid.min.js**（約 3.4MB）は `crates/yuzu-theme/assets/static/vendor/` に同梱
  （`scripts/vendor-mermaid.sh` で更新）。`backend: "ssr"` ならフォールバック発生
  ページ以外では配信されない（同梱自体はフォールバック用に継続）
- Mermaid のダークモード追従: SSR の SVG は CSS 変数で再描画なしに追従、
  client 描画は `data-theme` の変化を監視して同じ図を再描画する（Phase 13 で対応）

### 検索まわりの実装メモ

- **トークナイザ整合が最重要制約**: index 時（ネイティブ）と query 時（wasm）で
  同一コード（`yuzu-index-format`）＋同一モデルバイト（`dist/_search/model.zst`）を使う。
  `yuzu search` はブラウザと同じエンジンを通るので整合の検証にも使える
- **vaporetto モデル**（`bccwj-suw_c1.0`、圧縮 372KB、**MIT OR Apache-2.0**）は
  `crates/yuzu-index-format/assets/model/` に vendor（`scripts/vendor-vaporetto-model.sh` で更新）。
  ブラウザは初回検索時にモデルを遅延ダウンロードする
- **wasm 成果物**（467KB）は `crates/yuzu-index/assets/search/` に vendor
  （`scripts/build-search-wasm.sh` で更新。要 wasm32 target ＋ wasm-bindgen-cli ＋ binaryen。
  CLI は crate の `wasm-bindgen = "=x.y.z"` と完全同一バージョンにすること）
- **doc = セクション（h2/h3 境界）**: fragment v2 は `{ title, heading, url, anchor, text }`。
  `text` はセクション全文で、抜粋はクエリ時に `yuzu-index-format::make_excerpt`
  （native / wasm の 1 実装共有）で一致箇所周辺を動的生成する。ページタイトルの重みは
  リード doc（アンカーなし）だけに載せ、タイトル検索の重複ヒットを防ぐ
- manifest の `docLens` 直置きは維持: doc 数はセクション数に増えるが 1 doc 数バイトで、
  初回ロードが manifest 1 fetch で済むメリットが勝つ（数千 doc で数十 KB 程度）
- 設計ノートの「rkyv で直列化」は**不採用**: postings は元々 delta+varint の自前設計で
  rkyv の出番がなく、fragment は JS が直接読むため JSON が自然（wasm サイズ・依存とも有利）
- 検索 UI の確認は `yuzu preview` / `yuzu dev` 経由で行う（`file://` では fetch が動かない）

### llms.txt まわりの実装メモ

- llms-full.txt の本文は原文ではなく **comrak `format_commonmark` による正規形**
  （見出し ATX 化・箇条書き `-` 統一・裸 URL の `<url>` 化等）。`yuzu fmt`（Phase 6）と同じ基盤
- llms.txt のリンクは `baseUrl` 由来。**公開サイトでは `build.baseUrl` にフル URL
  （`https://…/docs/`）を設定すると絶対 URL になる**（llms.txt の慣行に合う）
- `public/llms.txt` を手書きで置くと生成版を上書きできる（テーマ上書きと同じ思想）
- ページ単位の md は**原文バイトそのまま**（frontmatter 込み）を `dist/<route>.md` に
  配信する（`yuzu fmt` 運用なら正規形と一致）。llms.txt のリンク先はこの `.md`。
  llms-full.txt は従来どおり正規化 Markdown の連結

### インクリメンタルビルドの実装メモ

- **キャッシュ対象は高価なページ派生物だけ**（メタ・本文 HTML・検索 tf・llms 正規化 md）。
  テンプレート合成や nav / fst / llms 連結などの集約は毎回全実行する。
  クロスページ依存はテンプレート段に閉じているため、この分離で依存解析なしに正しさを保てる
- 無効化キーは 3 層: **envKey**（設定・yuzu バージョン・トークナイザモデルの
  内容ハッシュ。不一致で全破棄）→ **routesKey**（rel→route 集合。`.md` リンク
  解決の入力なので、ページ増減・改名時は本文 HTML だけ全破棄）→
  **sourceHash**（ページ単体。変更されたページだけ再計算）
- 出力は **compare-before-write**（内容一致なら書き込まず mtime 温存。`yuzu fmt` と
  同じ思想）。書き出した dist 相対パスを `output-manifest.json` に記録し、
  前回との差分で削除ページの孤児出力を掃除する
- 不整合・破損・`.yuzu/cache/` 削除は常に「キャッシュなし = フルビルド」へ縮退。
  インクリメンタル結果はフルビルドと**バイト同一**（`__yuzu/build_id` を除く）

## ライセンス

MIT または Apache-2.0 のデュアルライセンス（お好きな方でどうぞ）。

- [LICENSE-MIT](LICENSE-MIT)
- [LICENSE-APACHE](LICENSE-APACHE)
