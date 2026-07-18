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
yuzu lint --fix     # 表記ゆれ（全角英数字・半角カナ・用語・長音符）を自動修正
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
  **`"..."` で囲むとフレーズ検索**: 引用部が連続で出現する文書だけにヒット
  （出現位置インデックスの隣接照合。引用部はタイポ・同義語展開なしの完全一致。
  抜粋はフレーズ全体を 1 まとまりでハイライトし、検索ボックスにも構文ヒントを表示）。
  引用符なしの複数語クエリには**近接ブースト**が働き、語がクエリ順に隣接して
  出現するページが上位に来る（ヒット集合は変えずスコアのみ）。
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
  **`yuzu lint --fix`** は表記ゆれ系の変換候補をソースへ自動適用する
  （fmt と同じく冪等・差分のないファイルには書き込まない。長音符ゆれは
  多数派へ統一し、同数タイは正解を決められないため報告のみ）。
  `yuzu check` はさらに**内部リンク・アンカー切れを行番号付きで報告**する
  統合 CI コマンド（終了コードは 0 = 違反なし / 1 = 違反あり / 2 = 実行エラー）

### 配信とカスタマイズ

- `public/` の静的物パススルー（画像等）
- **base path 対応**: `baseUrl: "/docs/"` でリンク・アセット参照をサブパスへ解決（社内リバプロ配下の配信を想定）。
  `yuzu build --base-url` で設定より優先して上書きできる（CI からの注入用）
- **GitHub Pages デプロイ雛形**: `yuzu new` が `.github/workflows/deploy.yml` を同梱。
  `configure-pages` の base_path を `--base-url` へ渡すため、project pages の
  サブパス（`/<リポジトリ名>/`）も設定なしで正しく配信される
- **404 ページ**: ビルド時に `404.html` を生成（テーマ統合・検索ボックスと
  サイドバー付き。GitHub Pages が自動で使う）。`public/404.html` を置けば
  そちらが優先される。`yuzu preview` / `dev` も存在しないパスへ同じ 404 を返す
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
v0.4（Phase 19〜23: 表記ゆれ組み込み lint / 検索の同義語・タイポ改善 / OpenAPI・JSON Schema SSR / flowchart スタイル構文。v0.4.1 で content 同伴アセットの自動コピーを追加）、
v0.5（Phase 24〜29: tankan スタイル構文の全図種展開 / 検索コードブロックの opt-in インデックス / OpenAPI Swagger 2.0・スキーマ一覧 / tankan mindmap・timeline / 形態素トークナイザ PoC は実測見送り / dogfooding 改善＝404 ページと lint --fix）は完了・リリース済み。

以下の Phase 30〜34 がすべて完了した時点で **v0.6** としてリリースする。  
実際の設計書運用（dogfooding）と並行して進め、Phase は価値と実装コスト・依存関係の順に並べている（着手時に個別に設計する）。

| Phase | 内容 | 状態 |
|---|---|---|
| **30 検索インデックスの位置情報化（フォーマット v3）** | postings に term の出現位置（セクション内トークン位置の delta varint 列。tf は見出し重み付きで出現数と一致しないため件数 varint を明示）を追加し `FORMAT_VERSION` 2→3。フィールド間（タイトル/見出し/本文）に位置ギャップを挟んで偽隣接を防ぐ。エンジンは位置を読み飛ばすだけで挙動不変（BM25 据え置き）＝フレーズ照合の土台のみ。`CachedSection` の変更に伴い `CACHE_FORMAT_VERSION` も上げる。**サイズ実測ゲート**: `dist/_search` 合計（素/gzip）の現行比を計測し「静的ホスティングだけで動く」方針と照合 → **通過**（scaffold 2 ページ: 合計 gzip +0.3%・語彙が極端に密な合成 301 ページ: 合計 gzip +14.0%〔1.18MB→1.34MB。postings 小計は 7.6KB→173KB〕。Phase 28 で見送った 9〜35 倍とは桁違いに小さい）。v2/v3 で `yuzu search` の結果はスコアまで完全一致を確認。wasm 再 vendor 済み | ✅ |
| **31 フレーズ検索（クエリ照合＋UI）** | `"..."` 引用符でフレーズ指定（**引用符なしの既定挙動は不変**。全角・カーリー引用符も受理、閉じ忘れは末尾まで）。引用部はトークナイズ→位置の隣接照合で **filter**（含まない doc を除外。スコア加点は構成 term の BM25 が担う）。タイポ・同義語展開の対象外＝完全一致のみで、語彙に無いフレーズは 0 件。セクションまたぎ非対応。抜粋・ハイライトはフレーズ全体を 1 needle ＋隣接マージで 1 まとまりにマーク。実装は SearchEngine（yuzu-index-format）1 箇所で native/wasm 共有、CI e2e にフレーズ実ヒット・逆順 0 件の検証を追加、wasm 再 vendor 済み（481→492KB） | ✅ |
| **32 ビルドのページ並列化（render）** | `render_site` のページループ（本文 HTML 生成〜テンプレート〜書き出し）を rayon で並列化。前提リファクタとしてハイライタのページ内状態をページローカルな `PageCodeRenderer` へ分離（`Cell` の `!Sync` が誤共有をコンパイル時に防ぐ）。集約（nav / llms / 404 / アセット）は直列のまま＝層構造不変。**決定性ゲート通過**: スレッド数 1/N・並列化前バイナリとの `diff -r` バイト同一。実測（release・--force）: render 支配のコーパス（201 ページ・ハイライト 1,200 ブロック＋mermaid SSR 200 図）で **2.07s → 0.69s（3.0 倍）**、テキスト主体 301 ページは 0.53s → 0.48s（トークナイズ支配 = Phase 33 の領分）。rayon は「凍結した設計判断」表へ追記 | ✅ |
| **33 ビルドのページ並列化（index）＋実測** | 検索インデックスのページごとトークナイズ（compute_sections）を rayon 並列化。キャッシュ判定を先行パスに分け、miss があるときだけトークナイザを 1 回構築して `&Tokenizer` を共有（vaporetto Predictor は `Sync`＝コンパイルで確認）。集約（doc_id 採番・postings・fst）はページ順の直列のままで決定性維持（スレッド 1/N・改修前バイナリと `diff -r` バイト同一）。**実測（release・M 系 Mac）**: テキスト主体 301 ページのフル 0.54s→0.41s・1,001 ページのフル 1.6s→1.1s（1 スレッド比。無変更 0.33s・1 ページ編集 0.39s）。render 支配なら Phase 32 の 3.0 倍が効く。残る直列部はメタ抽出（comrak）・モデル展開・fst/書き出し | ✅ |
| **34 dogfooding 改善** | 恒例のバッファ枠: **近接ブースト**（引用符なしの複数語クエリで、クエリ順に隣接出現するページを ×1.2/ペア のスコアで上位へ。フレーズ照合と同じ位置ロジックの soft 版で、ヒット集合は不変・タイポ/同義語展開語は対象外）・**フレーズ検索の発見性**（検索ドロップダウン末尾に `"..."` 構文のヒントを常時表示。引用符使用時は消える）・**ビルド時間の表示**（`build`/`dev` の完了ログに elapsed を追加。並列化の効果が見える）。OG メタ・favicon は今回も見送り | ✅ |
| **35 検索スタックのライブラリ化＋OPFS キャッシュ** | 外部記事（DuckDB-Wasm/Lindera-Wasm/OPFS 構成のオフライン検索）を受けて調査した結果、トークナイザ差し替えは Phase 28 の却下理由（転送量 9〜35 倍）がそのまま当てはまるため**見送り、vaporetto＋自作 BM25 エンジンは維持**。代わりに (1) 集約ロジック（doc_id 採番・postings・fst・シャード分割・manifest 構築）を `yuzu-index`（yuzu-core 依存）から `yuzu-index-format::build`（yuzu-* 非依存）へ移設し、tankan と同水準の「分離可能な設計」を検索スタックにも適用、(2) `Manifest` に `contentHash`（terms.fst＋全シャード＋モデルバイトの sha256、`#[serde(default)]` で後方互換）を追加し、ブラウザ側 OPFS（Origin Private File System）キャッシュの版管理に使用。フェッチ・OPFS・wasm 起動のオーケストレーションは `crates/yuzu-search-wasm/js/search-client.js`＋汎用ブロブキャッシュ `opfs-cache.js`（新規、DOM 非依存）に切り出し、テーマの `search-ui.js` は DOM/UX 層に純化。OPFS は contentHash 不一致 or 非対応環境で即座にフェッチのみ経路へフォールバック（`yuzu search` ネイティブ CLI は無関係・無改修）。**サイズ実測ゲート**: scaffold 2 ページで `dist/_search` 合計が raw 922,722→931,133B（+0.91%）・gzip 626,774→630,538B（+0.60%）。新規 JS は語彙量に依存しない固定コスト（`search-client.js` 4.9KB＋`opfs-cache.js` 2.7KB）で、`search_bg.wasm` は 494KB のまま実質不変（Cargo 依存・エクスポート API を変えていないため）。決定性テスト（`content_hash` は同一入力で同一値・内容変更で別値）を追加 | ✅ |

v0.7 以降の候補（このリリースではやらない）: ドキュメントバージョニング（要否含め保留中）・tantivy バックエンド（静的ホスティング方針と要調整）・i18n・VS Code 拡張（wasm プレビュー）・crates.io 公開とバイナリ配布・tankan の分離公開・yuzu 自身のドキュメントサイト公開・引用符なしクエリへの近接ブースト（フレーズ検索の発展形。Phase 34 の実運用結果で判断）。

<details>
<summary>完了済み: v0.5（Phase 24〜29）の内訳</summary>

| Phase | 内容 | 状態 |
|---|---|---|
| **24 tankan スタイル構文の全図種展開** | flowchart で対応した `classDef` / `class` / `:::` / `style`（＋fill 明度からのラベル色自動選択）を **state / ER / class 図**へ展開。適用先は状態ボックス・エンティティ・クラスボックスで、色付きボックスはタイトル帯含め全体を塗り全テキストを自動読みやすい色に。共通ロジックは `tankan::common::style` に集約。class 図は宣言の `class` と衝突しないよう一括適用を `cssClass` に | ✅ |
| **25 検索: コードブロックの opt-in インデックス** | `search.indexCode`（既定 off）でフェンスコードブロック本文を検索対象に追加。関数名・設定キーで設計書を引ける。tf 重みは本文と同じ 1・コードは抜粋にも出す（merge）・特別レンダリングされる言語（mermaid / openapi / jsonschema / math。無効化してプレーン表示なら索引対象）は除外・インデントコードは対象外・llms.txt には非混入。envKey が on/off を拾いキャッシュ自動無効化 | ✅ |
| **26 OpenAPI レンダリングの拡充** | Swagger 2.0 対応（`definitions` の `$ref` は既存機構で解決・`in: body` はリクエストボディ表示・responses 直下の `schema`・`produces`/`consumes` のメディアタイプ表示は operation が top-level を上書き。host/basePath 等は非表示）と、**全スキーマ一覧の描画**（`components/schemas` / `definitions` を文書末尾に閉じた details で。操作から参照されないスキーマも読める）。2.0 分岐は `SpecVersion::V2` に隔離し 3.x パスは挙動不変 | ✅ |
| **27 tankan 新図種** | **mindmap と timeline** を SSR 追加。mindmap は中央ルート左右振り分けの tidy tree（インデント階層パース・7 形状・ブランチごとのパレット色）、timeline は等間隔カラム＋セクション帯＋イベント縦積み。幅ベースの自動折返し `wrap_text` を common に新設（日本語は文字単位・ASCII は単語境界）。I/O なし・時刻非依存の設計原則は維持、corpus 11 本＋スナップショット 6 枚 | ✅ |
| **28 形態素トークナイザ PoC** | vibrato / lindera への差し替えを実測し（wasm サイズ・精度・速度・辞書配布）、**見送り = 現行 vaporetto + SUW 継続を決定**。根拠: 差し替えは合計転送量が現行 ≈450KB の 9〜35 倍（vibrato+ipadic ≈7.8MB / lindera embed-ipadic は wasm 58MB・gzip 15.8MB）で「静的ホスティングだけで動く」方針と衝突。精度改善は辞書語の 1 語化に限られ、ipadic の誤分割（ワークス/ペース）やカタカナ連結による部分語 recall 低下も確認。SUW 細分割の弱点は同義語・タイポ機構（Phase 20/21）で緩和済み。v0.6 のフレーズ検索はトークナイザ据え置きで位置情報インデックスのみで実現する | ✅ |
| **29 dogfooding 改善** | 実運用の不満の一括解消（バッファ枠）: **404 ページの生成**（テーマ統合・検索ボックス付き `404.html`。Pages デプロイ雛形同梱なのに直リンク切れが素の 404 だった穴。`public/404.html` で上書き可・`preview`/`dev` も 404 ステータスで配信）と **`yuzu lint --fix`**（表記ゆれ lint は変換候補まで出すのに適用が手作業だった穴。全角英数字・半角カナ・`lint.terms`・長音符ゆれ多数派を自動適用。冪等・mtime 温存・同数タイは報告のみ） | ✅ |

</details>

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
| ページ並列化 | **rayon** | `render_site` のページループをデータ並列化（Phase 32）。ページ内状態は `PageCodeRenderer` がページローカルに持ち、`Cell` の `!Sync` が誤共有をコンパイル時に防ぐ。出力はスレッド数に依らずバイト同一（決定性ゲートで検証） |

依存方向（凍結。逆方向依存は作らない）:

```
yuzu-cli → {yuzu-server, yuzu-render, yuzu-index, yuzu-core, yuzu-config}
yuzu-render → yuzu-core, tankan     yuzu-index → yuzu-core, yuzu-index-format
yuzu-search-wasm ↔ yuzu-index-format（native/wasm でトークナイザ・フォーマット共有）
tankan・yuzu-index-format・yuzu-search-wasm は yuzu-render/yuzu-theme/yuzu-core/
yuzu-config 非依存の汎用ライブラリ（将来 crates.io/npm へ分離可能な設計を維持。
検索スタックの書き側集約ロジックは yuzu-index-format::build に、読み側クエリエンジンは
SearchEngine にあり、yuzu-index はページ抽出とファイル I/O だけを担う薄い呼び出し側）
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
- 検索 UI の確認は `yuzu preview` / `yuzu dev` 経由で行う（`file://` では fetch が動かない。
  OPFS も同様にセキュアコンテキスト＝https か localhost が必要）
- **OPFS によるブラウザ側キャッシュ**（Phase 35）: `manifest.json` は毎回ネットワークから
  取得し、`contentHash`（terms.fst ＋ 全シャード ＋ モデルバイトの sha256）が OPFS 保存済みの
  前回 manifest と一致すれば `terms.fst`/`model.zst`/シャードは OPFS から読み、再訪問時の
  再フェッチを省略する。不一致・OPFS 非対応・非セキュアコンテキストでは既存のフェッチのみ
  経路へ自然にフォールバックする（`crates/yuzu-search-wasm/js/search-client.js` /
  `opfs-cache.js`）。DuckDB-Wasm・Lindera-Wasm への置き換えは**行っていない**
  （Phase 28 の却下理由がそのまま当てはまるため。既存の vaporetto＋自作 BM25 エンジンを維持）

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
