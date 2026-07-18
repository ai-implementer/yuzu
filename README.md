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
- **画像・添付ファイル**: `public/` 配置（`/images/...` のサイト絶対参照）に加え、
  **ページと同じディレクトリに置いて相対参照**もできる（`![図](diagram.png)`）。
  content 配下の `.md` 以外は dist へ自動コピーされ、相対参照は正しい URL に
  解決される（隠しファイルと `input.ignore` 一致は除外）。参照切れは `yuzu check` が検出する
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
  （sequence・flowchart・class・state・ER・gantt・pie・mindmap・timeline 対応。未対応図種は自動でクライアント描画にフォールバックし、
  フォールバックが発生したページだけ mermaid.js を読み込む。SSR の SVG は
  CSS 変数経由でダークモードに**再描画なしで追従**。flowchart / state / ER / class は
  `classDef`（`default` 含む）/ `:::` / `style` 文のスタイル指定も SSR
  （複数ノードへの一括適用は flowchart・state・ER が `class` 文、class 図は宣言と衝突しないよう `cssClass` 文）。
  ユーザ指定色は意図どおりの固定色で描き、色付きボックスの文字色は背景の明度から自動で読みやすい側を選ぶ。
  linkStyle / click はフォールバック）
- **API 仕様（OpenAPI / JSON Schema）**: ` ```openapi ` / ` ```jsonschema ` ブロックを
  **ビルド時に静的 HTML 化**（SSR 自前・クライアント JS ゼロ・テーマ/ダークモード統合）。
  YAML / JSON 両対応で、ブロック先頭 1 行を `file: specs/api.yaml`（プロジェクトルート相対）
  にするとファイル参照になる（参照ページはキャッシュ対象外 = 仕様ファイルの変更が次ビルドで必ず反映）。
  `$ref` は文書内（`#/...`）と**プロジェクト内の別ファイル**（`schemas/common.yaml#/...`。
  仕様ファイル内はファイル相対・インラインブロック内はルート相対・HTTP とルート外は拒否）を
  解決する（循環は参照名表示）。**Swagger 2.0 も描画対応**（`definitions` /
  `in: body` のリクエストボディ / `produces`・`consumes`）。文書末尾に
  **全スキーマの一覧**（`components/schemas` / `definitions`。操作から参照されない
  スキーマも読める）を閉じた折りたたみで出力。パース失敗は
  エラーボックス表示でビルドは継続する

### 検索と LLM 連携

- **日本語全文検索**（自前 BM25）: vaporetto の分かち書き＋**文字単位**の編集距離 1 の
  タイポトレランス（「ダーくモード」でもダークモードがヒット）。インデックスは静的ファイル（`dist/_search/`）で、ブラウザは
  wasm ＋ 2 段フェッチ（Pagefind 型）— サーバ不要、CDN/静的ホストだけで動く。
  **検索はセクション（h2/h3）単位**で「ページ › 見出し」の結果から `#アンカー` へ
  直接ジャンプ。抜粋はクエリ一致箇所周辺を動的生成し、分かち書き単位でハイライト。
  **`lint.terms` の用語辞書と `search.synonyms` がクエリ拡張に使われ、
  ゆれ表記（「サーバ」）で検索しても正表記（「サーバー」）の文書がヒット**
  （ハイライトも正表記側に乗る）。`search.indexCode` を有効にすると
  フェンスコードブロックも検索対象になり、**関数名・設定キーで設計書を引ける**
  （既定 off。特別レンダリングされる mermaid / openapi / jsonschema / math のソースは除外。
  mermaid / math を設定で無効化しプレーンコード表示にしている場合は見えるまま索引される）。
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
  「サーバ/サーバー」のような表記ゆれを行番号付きで検出）と**組み込みの表記ゆれ
  ルール**（全角英数字・半角カナ・長音符ゆれの混在をプロジェクト横断で検出。
  既定有効・`lint.rules` でルール単位の無効化可。コード・URL は対象外）。
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
- **git 連携メタ**: `git.lastUpdated` でページフッターに最終コミット日、
  `git.editUrl` で「このページを編集」リンク（`{path}` が content 相対パスに置換）。
  git が無い環境・未コミットのページでは日付を出さずに縮退する

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
    "terms": { "サーバー": ["サーバ"], "ユーザー": ["ユーザ"] },
    // 組み込みの表記ゆれルール（既定はすべて有効。false で個別無効化）
    "rules": { "fullwidthAlphanumeric": true, "halfwidthKana": true, "katakanaChoon": true }
  },
  "search": {
    "enabled": true,
    // "dictionary": "models/custom.model.zst", // vaporetto モデルの差し替え
    "typoTolerance": { "enabled": true, "maxEdits": 1 },
    "shard": { "maxTermsPerShard": 16384 },
    // 同義語グループ（lint.terms と合成してクエリ拡張に使う）
    "synonyms": [["ログイン", "サインイン"]]
  },
  "llms": { "enabled": true, "full": true }, // llms.txt / llms-full.txt
  "build": { "baseUrl": "/docs/" }, // site.baseUrl より優先
  "dev": { "host": "127.0.0.1", "port": 5173, "liveReload": true, "open": false },
  "git": {
    "lastUpdated": true, // ページフッターに最終コミット日（git 不在時は自動で非表示）
    "editUrl": "https://github.com/me/docs/edit/main/content/{path}" // 「このページを編集」
  }
}
```

## ロードマップ

v0.1（Phase 1〜6: build / dev サーバ / 日本語検索 / llms.txt / tankan SSR / fmt・lint・check）、
v0.2（Phase 7〜12: 執筆表現 / 数式 / ページナビ / 検索セクション単位化 / デプロイ雛形 / インクリメンタルビルド）、
v0.3（Phase 13〜18: 執筆の即効改善 / ページ Markdown 配信とコピー / 用語統一 lint / tankan class・pie / git 連携メタ / dogfooding 改善）、
v0.4（Phase 19〜23: 表記ゆれ組み込み lint / 検索の同義語・タイポ改善 / OpenAPI・JSON Schema SSR / flowchart スタイル構文。v0.4.1 で content 同伴アセットの自動コピーを追加）は完了・リリース済み。

以下の Phase 24〜29 がすべて完了した時点で **v0.5** としてリリースする。  
実際の設計書運用（dogfooding）と並行して進め、Phase は価値と実装コスト・依存関係の順に並べている（着手時に個別に設計する）。

| Phase | 内容 | 状態 |
|---|---|---|
| **24 tankan スタイル構文の全図種展開** | flowchart で対応した `classDef` / `class` / `:::` / `style`（＋fill 明度からのラベル色自動選択）を **state / ER / class 図**へ展開。適用先は状態ボックス・エンティティ・クラスボックスで、色付きボックスはタイトル帯含め全体を塗り全テキストを自動読みやすい色に。共通ロジックは `tankan::common::style` に集約。class 図は宣言の `class` と衝突しないよう一括適用を `cssClass` に | ✅ |
| **25 検索: コードブロックの opt-in インデックス** | `search.indexCode`（既定 off）でフェンスコードブロック本文を検索対象に追加。関数名・設定キーで設計書を引ける。tf 重みは本文と同じ 1・コードは抜粋にも出す（merge）・特別レンダリングされる言語（mermaid / openapi / jsonschema / math。無効化してプレーン表示なら索引対象）は除外・インデントコードは対象外・llms.txt には非混入。envKey が on/off を拾いキャッシュ自動無効化 | ✅ |
| **26 OpenAPI レンダリングの拡充** | Swagger 2.0 対応（`definitions` の `$ref` は既存機構で解決・`in: body` はリクエストボディ表示・responses 直下の `schema`・`produces`/`consumes` のメディアタイプ表示は operation が top-level を上書き。host/basePath 等は非表示）と、**全スキーマ一覧の描画**（`components/schemas` / `definitions` を文書末尾に閉じた details で。操作から参照されないスキーマも読める）。2.0 分岐は `SpecVersion::V2` に隔離し 3.x パスは挙動不変 | ✅ |
| **27 tankan 新図種** | **mindmap と timeline** を SSR 追加。mindmap は中央ルート左右振り分けの tidy tree（インデント階層パース・7 形状・ブランチごとのパレット色）、timeline は等間隔カラム＋セクション帯＋イベント縦積み。幅ベースの自動折返し `wrap_text` を common に新設（日本語は文字単位・ASCII は単語境界）。I/O なし・時刻非依存の設計原則は維持、corpus 11 本＋スナップショット 6 枚 | ✅ |
| **28 形態素トークナイザ PoC** | vibrato / lindera への差し替えを **PoC として実測**（wasm サイズ・精度・速度・辞書の配布形態）。トークナイザ整合制約で index/query 両側同時差し替えが必要なため、採用判断のみ行い、本実装＋フレーズ検索（位置情報インデックス）は結論次第で v0.6 のフォーマット改版 1 回にまとめる | ⬜ |
| **29 dogfooding 改善** | 実運用で溜まった不満の一括解消（バッファ枠）。v0.5 の締めとして実施 | ⬜ |

v0.6 以降の候補（このリリースではやらない）: ドキュメントバージョニング（要否含め保留中）・フレーズ検索（Phase 28 の結論と合わせてフォーマット改版）・tantivy バックエンド（静的ホスティング方針と要調整）・i18n・VS Code 拡張（wasm プレビュー）・crates.io 公開とバイナリ配布・tankan の分離公開・yuzu 自身のドキュメントサイト公開・ビルドのページ並列化。

<details>
<summary>完了済み: v0.4（Phase 19〜23）の内訳</summary>

| Phase | 内容 | 状態 |
|---|---|---|
| **19 表記ゆれ lint の組み込みルール** | `fullwidth-alphanumeric`（全角英数字。半角の変換候補付き）・`halfwidth-kana`（半角カナ。濁点合成込みの変換候補付き）・`katakana-choon`（長音符ゆれの混在をプロジェクト横断の多数決で検出。少数派の出現箇所に警告）。既定有効・`lint.rules` でルール単位の無効化可 | ✅ |
| **20 検索の用語ゆれ・同義語対応** | `lint.terms` ＋ `search.synonyms` を manifest 経由でクエリ拡張に使用（同義語 = weight 1.0、変形上限 8）。ハイライトも同義語側に対応。実装は SearchEngine（yuzu-index-format）1 箇所で native/wasm 共有、wasm 再 vendor 済み | ✅ |
| **21 検索 UX の磨き込み** | **日本語タイポトレランスの修正**（levenshtein_automata の文字単位 DFA へ置換。CI e2e も実ヒットを検証するよう強化）＋検索 UI の改善: 結果件数表示（`search_with_total`）・IME 変換中の検索抑制とキー競合回避・ローディング表示・未選択 Enter で先頭ヒットへ・aria-selected / aria-activedescendant の同期 | ✅ |
| **22 OpenAPI / JSON Schema レンダリング** | ` ```openapi ` / ` ```jsonschema ` ブロックのビルド時 SSR（自前実装・JS ゼロ・テーマ統合）。インラインと `file:` 参照（ルート相対・ルート外拒否）の両対応、`$ref` ローカル解決＋循環ガード、参照ページはキャッシュ非対象で仕様変更が即反映。失敗はエラーボックスでビルド継続 | ✅ |
| **23 dogfooding 改善** | 積み残しの一括解消（バッファ枠）: tankan flowchart のスタイル構文 SSR（`classDef` / `class` / `:::` / `style`）・OpenAPI のプロジェクト内ファイル間 `$ref` 解決（参照元ファイル相対・ルート外拒否・参照ページはキャッシュ非対象）・小粒の磨き込み（trace メソッド・description 二重表示修正・ドキュメント陳腐化）。リリース後の v0.4.1 で content 同伴アセット（ページ横の画像）の自動コピーと相対参照の URL 解決を追加 | ✅ |

</details>

<details>
<summary>完了済み: v0.3（Phase 13〜18）の内訳</summary>

| Phase | 内容 | 状態 |
|---|---|---|
| **13 執筆の即効改善** | draft プレビュー（`dev --drafts` / `build --drafts` で下書きをバナー付き表示、通常ビルドに戻すと出力は自動掃除）・Mermaid client 描画のダークモード切替時の再描画（既知の制限の解消）・テーマ CSS 変数の設定化（`theme.cssVars` / `cssVarsDark`。値の検証込み） | ✅ |
| **14 ページ単位 .md 配信とページコピー** | 各ページの原文 Markdown を `dist/<route>.md` に配信して llms.txt を `.md` リンク化（vitepress / docusaurus プラグインで優勢の形式）＋各ページに「Markdown をコピー」ボタンと `.md` リンク（fetch → クリップボード。コードコピーと同じプログレッシブエンハンスメント） | ✅ |
| **15 日本語 lint: 用語統一** | `lint.terms` のプロジェクト用語辞書による用語統一チェック（`term-variant`）を `yuzu lint` / `check` に統合。本文・見出し・リンクラベルを行番号・列番号付きで報告し、コード・URL・正表記の部分一致は対象外。組み込みルール（全角/半角等）は実運用の需要を見て拡張 | ✅ |
| **16 tankan 図種追加** | 設計書頻出の **class 図**（3 区画ボックス・関係 8 種・多重度・ジェネリクス・アノテーション）と **pie**（showData・凡例・CSS 変数パレット）を SSR 対応。corpus 13 本＋スナップショット＋wasm32 担保 | ✅ |
| **17 git 連携メタ** | `git.lastUpdated`（1 回の git log で全ページの最終コミット日を収集しフッター表示）・`git.editUrl`（`{path}` 置換の編集リンク）。git 実行は cli 層のみ（render はデータ注入）で、git 不在・未コミットは表示なしに縮退 | ✅ |
| **18 dogfooding 改善** | 実運用で踏んだ不満の解消: **JSONC 重複キーの警告**（後勝ちで設定が黙って無視される事故の検出。`site.title` 形式のパス付き）と **`yuzu dev --host` / `preview --host`**（コンテナ内から 0.0.0.0 で配信する用途。設定より優先） | ✅ |

</details>

<details>
<summary>完了済み: v0.2（Phase 7〜12）の内訳</summary>

| Phase | 内容 | 状態 |
|---|---|---|
| **7 執筆表現** | Admonition（`> [!NOTE]`、comrak alerts 拡張＋テーマ CSS）・脚注（footnotes 拡張）・コードブロックのコピーボタン（プログレッシブエンハンスメント JS）。fmt / llms.txt との整合（format_commonmark の出力確認）込み | ✅ |
| **8 数式** | comrak math（`$...$` / `$$...$$`）→ KaTeX 描画。クライアント描画か SSR かの設計判断・vendor 資産の同梱方針を含む | ✅ |
| **9 ページナビ** | 前/次ページリンク（nav 順から導出）＋階層パンくず。テンプレート＋nav モデルの拡張 | ✅ |
| **10 検索セクション単位化** | fragment を見出し単位に分割して `#アンカー` へ直接ジャンプ＋クエリ一致箇所周辺の動的抜粋。index フォーマット変更のため wasm/native トークナイザ整合制約に注意 | ✅ |
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
