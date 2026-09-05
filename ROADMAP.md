# ロードマップ

yuzu の開発計画と、これまでのリリースの内訳。**このファイルが Phase 状態の正**
（README には現在の版と概要だけを置く）。

## 現在: v0.15（Phase 64〜67）

**v0.14 まで公開済み**（[kabosu 0.1.0 も crates.io で公開済み](https://crates.io/crates/kabosu)。
yuzu のリリースとは非同期。publish 前に fuzz を回す規律は CLAUDE.md にある）。

v0.15 の軸は「**正しさ・堅牢性**」。v0.10.1 の外部コードレビューで「今回は入れない」と
判断した持ち越し 3 件（URL のパーセントエンコード全面対応 / `ServeDir` がシンボリック
リンクを辿る / `syntect.css` の無条件出力）は、どれも**正しさの穴を規約や空ファイルで
塞いでいる**状態のまま 5 版を越えた。v0.14 で設定の正しさ（位置付き診断・未知キーの拒否）
を整えたので、次は生成 URL・配信・検査の正しさへ寄せる。候補に残していた
「外部リンク切れ検査」も、凍結方針（決定的・オフライン）と衝突しない opt-in の形を
決めてここで入れる。Phase は価値と実装コスト・依存関係の順（着手時に個別に設計する）。

### 64 URL のパーセントエンコード ✅

従来は `#` `?` `%` `"` `'` `<` `>` `` ` `` `\` を含むファイル名を `unsafe-page-path`
（error・build 中断）で**拒否**して整合を取り、空白と非 ASCII は ServeDir とブラウザが
黙って補正するから動いていた。route → URL の変換点を 1 つに決めてパスセグメントを
エンコードし、「たまたま動く」を「仕様として動く」に格上げした。

- **変換点は (A) `yuzu-core/src/urlpath.rs`** の `encode_path` / `percent_decode` の対
  （`linkcheck::percent_decode` をここへ移設）。呼ぶのは `UrlResolver::page_url` /
  `md_url` / `rewrite`（本文・ナビ・pager・パンくず・リダイレクト・llms・sitemap が
  全部ここを通る）、検索索引の `url`（`yuzu-index/builder.rs`）、`git.edit_url` の
  `{path}` の 4 箇所。(C) の `Page` に URL 表現を持たせる案は CachedMeta / routesKey の
  再定義を伴うので採らなかった。**`| url` フィルタに足す案は不可**のまま
  （`page.edit_url` の `://` とクエリを壊す）
- **非 ASCII もエンコードする**（英数字と `-._~!$*+,;=:@` 以外を UTF-8 バイト単位で
  `%XX`）。本文リンクは comrak の `escape_href` が既に非 ASCII を `%XX` にしていて、
  ナビ・llms・sitemap・索引だけ生の `/設計/` だった食い違いが解消した。comrak は
  `%XX` を素通しするので、先にエンコードした値を本文に埋めても二重にならない。
  `'` `(` `)` `&` は RFC 上は許されるが属性・CommonMark のリンク先・実体参照を
  壊すのでエンコード側（llms.txt の `[title](url)` が `)` で壊れる潜在バグの修正）
- **「ディスクは生・URL はエンコード」**: `Page.route` と route をキーにした HashMap
  （nav / pager / breadcrumbs / llms / linkcheck / 索引のグループ）は生のまま、
  書き出し・表示の直前だけエンコードする。`?` / `#` 以降の suffix はエンコードしない
  （フラグメントは従来どおり comrak が処理）。`/foo` 始まりの著者 URL も再エンコード
  しない（`%E8` が `%25E8` に化けるため）
- **著者がエンコード済みで書いた参照はデコードして照合する**（render の `.md` 解決・
  同伴アセット解決、linkcheck の 3 分類、aliases の正規化）。CommonMark で空白入りの
  ファイル名を書く素の形 `[x](my%20page.md)` が従来は route 解決に失敗して警告＋機械変換、
  alias `old%20name/` は `old%20name/` ディレクトリに書かれて 404 だった、の 2 件を
  修正した形。**`yuzu fmt`（format_commonmark）は `<my page.md>` を `my%20page.md` へ
  正規化する**ので、デコードしないと fmt の正規形でリンクが切れる = この判断は必須だった。
  aliases の生の `#` `?` は引き続き拒否（URL として書く場所なので）。ファイル名の
  `#` `?` へのリンクは `%23` `%3F` と書く以外にない（`<a#b.md>` でも `#` 以降は
  フラグメント。URL 構文上の制約で docs に明記）
- `unsafe-page-path` は実ファイル名では `\` と制御文字だけに縮小（メッセージも
  「出力パスとして使えない」へ）。**設定・frontmatter 由来の route（合成ページ・
  aliases）は Windows の予約文字 `< > : " | ? *` も全 OS で拒否する**（レビュー指摘。
  Linux で通った `search.page = "a?b"` や alias `a%3Fb` が Windows の書き出し途中で
  I/O エラーになる形を事前診断へ。実ファイル名は執筆者の OS が作れた時点で正当）。
  linkcheck の絶対・相対の分類は render と同じくデコード前の文字列で行う
  （`%2Flogo.png` は相対。同じくレビュー指摘）。`CACHE_FORMAT_VERSION` 21 → 22。
  docs `reference/rules.md` / `guide/writing.md`（リンクの書き方と aliases のデコード）
  と ci.yml のゲート（原稿の語とエンコード例）を追随

### 65 配信とテーマ契約の堅牢化 ✅

小さい 2 件。どちらも「書き込み側は塞いだが読み側・テンプレート側に同じ規律が
無い」型だった。

- **`preview` / `dev` がシンボリックリンクを辿らない**: tower-http 0.7 の `ServeDir` には
  リンク追従を止めるオプションが無い（パス検証は字句のみ）ので、公開されている
  `Backend` トレイトと `ServeDir::with_backend` で `TokioBackend` を包む `GuardedBackend` を
  yuzu-server に置き、`open` / `metadata` と 404 フォールバックの読み込みの前に述語を呼ぶ。
  述語は `ServeOptions.path_guard`（`PathGuard = Arc<dyn Fn(&Path) -> Result<(), String>>`）
  で cli が渡す（`WatchIgnore` と同型 = `server → core` の辺を作らない。中身は
  `yuzu_core::output::ensure_symlink_free` = 書き側 `ensure_no_symlink_under` と同じ検査で
  `target == root` だけ許す読み側版。**起点はプロジェクトルート**で書き側と同じ =
  `output.dir = "alias/site"` の `alias` のような出力先までの中間ディレクトリのリンクも
  拒否する。レビュー指摘: 当初は出力ディレクトリ起点で、build が拒否する構成を
  preview だけが配信していた）。**判断: 遮断は 404 ＋ warn ログ**（build が書かない
  ものは「無い」扱い。GitHub Pages 等の本番と見え方が一致し 404.html に乗る）/
  **既定 ON・設定キー無し**（配信で辿る正当な用途が無く、opt-out を作ると
  `pin_restart_only` の対象が増えるだけ）/ **同期 lstat をそのまま呼ぶ**（ローカル dev で
  深さ分の µs。core の 1 実装を共有できる）
- **`syntect.css` は有効時だけ**: `RenderShared.syntect_css` を `Option` にし、
  `pipeline.rs` の `highlight_enabled => cfg.markdown.highlight.enabled` で `base.jinja` の
  `<link>` と書き出しを同じ条件にした（`dark_enabled` と同型。無効化したら孤児掃除が消す）。
  `theme/templates/base.jinja` を上書きしている利用者は追随が要るので、**「テーマ上書きは
  デフォルトテーマ側の変更へ追随する責任が利用者側にある」を `guide/deploy.md` に
  明文化**し、破壊的変更を許す契約に格上げした（i18n 候補の `theme.strings` も同じ契約に
  乗せる前提）

### 66 外部リンク切れ検査（opt-in） ✅

`broken-link` は「外部 URL は検査しない」が契約（決定的・オフライン）。これを既定経路に
入れずに `yuzu check --external-links` の opt-in で足した。

- **土台**: 外部 URL 判定を `urlpath::is_external_url` / `is_http_url` の 1 実装へ寄せた
  （`linkcheck.rs` / `urls.rs` の文字列リテラルを置換）。`check_links` は
  `LinkReport { diags, external }` を返し、http / https の出現箇所（`ExternalLink` =
  rel / span / url / is_image）を捨てない。HTTP を行うのは cli の `commands/extlink.rs`
  だけ。インクルード断片内のリンクは従来どおり対象外
- **判断: HTTP は curl へ委譲**（(b)）。workspace に HTTP クライアントも TLS も無く、
  `ureq` + `rustls` は ring の C/asm ビルドと 20 超の crate を配布バイナリと 4 プラット
  フォームのリリースビルドへ持ち込む（comrak / syntect で onig を避けたのと同じ規律で
  却下）。opt-in の検査だけが外部ツールに依存し、curl が無ければ実行エラー（exit 2）。
  `curl -sS -o /dev/null -L --max-redirs 10 --connect-timeout 10 --max-time 20 -w %{http_code}`
  を 8 並列（`std::thread::scope`）・同一 URL は 1 回。テストは依存を増やさない手書きの
  ローカル HTTP サーバに curl を当てる。**docs 自身への dogfooding で判明**: crates.io は
  ブラウザ相当の `Accept: text/html` が無いと 404 を返すので、curl に Accept ヘッダを
  付ける（付けないと `crates.io/crates/tankan` 等が誤報になる）
- **判断: 入口はフラグだけ**（`[check]` セクションは作らない。必要になったら後から）。
  新ルール `external-link-broken` は warning・suppressible（`lintDisable` / 行コメント /
  `lint.rules`）。`DISABLEABLE_RULES` と docs `rules.md` に追記
- **判断: 4xx（429 を除く）だけ診断、それ以外は skipped**。DNS 失敗・タイムアウト・
  TLS エラー・5xx・429・curl の失敗は診断に載せず `summary.skipped`（URL 単位・キー追加の
  加算的変更・常に出す）と集計行「スキップ N 件」に計上し、理由は warn ログへ = 環境依存の
  失敗で CI を赤にしない。「凍結した設計判断」表（docs `development/index.md` と
  CLAUDE.md）に「ネットワーク I/O は既定経路に入れない」を明文化
- CI の e2e はネットワークへ出ず、自分の `yuzu preview` を相手に「4xx → warning・
  到達不能 → skipped・既定の check は触れない」を検証する。docs.yml で実運用するかは
  Phase 67 で決める
- **レビュー指摘 2 件**: (1) `--external-links` なしの `check` / `lint` では
  `external-link-broken` の抑制が `unused-lint-suppression` になり、例外指定が既定の
  オフライン CI を落としていた → `LintOptions.unevaluated_rules`（この実行で評価しなかった
  ルール）を足し、`apply_suppressions` の unused 免除を「全体無効化中 or 未評価」にした。
  (2) curl のグロブ展開で `?q=[1-2]` が 2 回取得（`404404`）・`?filter[name]=x` が
  構文エラーになり、壊れたリンクが skipped に化けていた → `--globoff` を付け、
  角括弧・波括弧 URL の回帰テストを追加。(3) 続報: `--external-links` ありでも
  到達性を判定できずスキップした URL への抑制が unused になり、環境要因で exit 1 に
  なっていた → `LintOptions.unevaluated_occurrences`（rel / 行 / ルール）で
  スキップした出現箇所を抑制処理へ渡し、その行の行コメント・そのページの
  `lintDisable` を「効いた」扱いにして unused 判定を保留（suppressed には数えない）

### 67 dogfooding 改善 ⬜

前 3 Phase を自分のサイトで使い、漏れを拾う。候補: 日本語・空白入りファイル名のページを
docs（または scaffold）に置いて検索・llms・sitemap まで通す / docs.yml に外部リンク検査を
乗せる（週次 or 手動）/ `preview` のリンク遮断の e2e / 持ち越し候補（OS ダーク追従・
パーマリンクの到達性・head メタ・`--root`）から 1 つ。着手時に選定する。

## v0.10.1 レビューの持ち越し

v0.10.1（外部コードレビュー対応）で「今回は入れない」と判断したもの。
**判断の根拠ごと残す**（同じ検討を繰り返さないため）。

- **URL のパーセントエンコード全面対応** — v0.15 の Phase 64 へ（上記）
- **キャッシュ保存の原子性** — Phase 53（v0.11）で実装済み（`write_atomic_under`
  が global.json のみ tmp → rename = 当時の見積もりどおり安価な側だけ。詳細は
  v0.11 の内訳を参照）
- **`syntect.css` の無条件出力** / **`ServeDir` が dist 内のリンクを辿る** — v0.15 の
  Phase 65 へ（上記）
- **`.devcontainer/post-create.sh` の Claude Code インストーラ** — 取得した
  `install.sh` を検証せず bash へ渡している。ただし**インストーラ自身が
  ダウンロードしたバイナリを SHA-256 検証している**（バージョンごとの
  `manifest.json` の値と照合し、不一致なら削除して終了）ので、残るギャップは
  スクリプトの TOFU のみ。`install.sh` に公開チェックサムが無く、ベンダ更新のたびに
  devcontainer のビルドが壊れるため固定は見送った。バージョン指定
  （`bash -s -- <version>`）は可能なので、必要になったらそこから

## v0.16 以降の候補

- **dogfooding 候補（v0.13 Phase 61 からの持ち越し。判断根拠ごと残す）**:
  - `theme.dark: false`・JS 無効時の OS ダーク追従 — base.jinja が
    `data-theme="light"` を無条件ハードコードしており CSS フォールバックの前提から
    崩す必要がある。ダーク定義が 3 箇所（theme.css / syntect 生成 / css_vars_dark
    生成）に散りフォールバック追加で全部 2 系統化（21K の syntect.css が倍増）、
    さらに「dark: false でもダークになる」= 設定キーの意味の再定義（3 値化等）を
    伴う。候補中最重量で単独 Phase 相当
  - 見出しパーマリンクのキーボード到達性 — `<a aria-hidden class="anchor">` は
    comrak のハードコード出力で、完全対応（aria-hidden 除去＋ラベル付与）は
    yuzu-core の後処理 = 本文 HTML 変更で CACHE bump ＋全スナップショット更新。
    CSS だけの部分対応は「aria-hidden 内のフォーカス可能要素」という別の違反を生む
  - ページメタの拡充（読了時間・文字数）/ `<head>` メタ — canonical / og:url は
    sitemap と同じ「baseUrl がフル URL のときだけ」ゲート（pipeline.rs）に乗せれば
    新キーゼロで実装可能と調査済み（og:image だけ素材不足）。読了時間・文字数は
    extract_meta で数えて CachedMeta へ載せる = CACHE bump を伴う
  - `--root` グローバルオプションと shell 補完 — 探索・読み込みは Phase 63 で
    `commands::load_project` に一本化済み（`--root` はここ 1 箇所に足せば全コマンドに
    効く）。残るのは clap_complete の新規依存＋ 8 つの run() シグネチャ変更と、
    build / dev だけが load_config（上書き適用・`.yuzu` のリンク検査）を通る
    非対称を揃えるかの設計判断。着手時は MarkdownOptions 構築の 8 箇所コピーの
    解消と抱き合わせると割が良い
- **i18n** — テーマ UI 文字列の多言語化。実測で jinja 18 ＋ テーマ JS 19 ＋
  apispec 35 ＋ crossref 3 文字列。`site.lang` は `<html lang>` の 2 箇所でしか
  使われていない。判断材料:
  - **検索（vaporetto の分かち書き）と `lint.rules`（全角英数・半角カナ・長音符）は
    日本語固有**なので、UI だけ多言語化しても半端になる
  - テーマ上書き（`theme/templates/`）で文言は今でも変えられるが、粒度がファイル単位で
    アップストリームから fork するため代替にならない（`search-ui.js` は 470 行）
  - 最小案は `theme.strings` の部分上書き辞書（`theme.css_vars` / `glossary.terms` と同型）。
    既定を日本語のまま据え置けばスナップショットは動かない
  - apispec の文言は**描画のエラーボックスと `yuzu check` の診断で共有**しているので、
    翻訳すると `--format json` の出力も言語で変わる（CLI を含めるかの線引きが要る）
- **ドキュメントバージョニング** — 要否含め保留中
- **VS Code 拡張** — wasm プレビュー。`yuzu-core` / `yuzu-render` が 9 ファイルで
  `std::fs` に依存しており I/O 抽象化が前提
- **yuzu 本体の crates.io 公開** — 汎用ライブラリ層は tankan・mikan まで公開済み。
  名前 `yuzu`・`yuzu-core` が別プロジェクトに取得済みのため、単一パッケージ化するか
  名称を再検討する必要がある（Phase 37 の決定事項）

## これまでのリリース

- **v0.1**（Phase 1〜6）build / dev サーバ / 日本語検索 / llms.txt / tankan SSR / fmt・lint・check
- **v0.2**（Phase 7〜12）執筆表現 / 数式 / ページナビ / 検索のセクション単位化 /
  デプロイ雛形 / インクリメンタルビルド
- **v0.3**（Phase 13〜18）執筆の即効改善 / ページ Markdown 配信とコピー / 用語統一 lint /
  tankan class・pie / git 連携メタ / dogfooding 改善
- **v0.4**（Phase 19〜23）表記ゆれの組み込み lint / 検索の同義語・タイポ改善 /
  OpenAPI・JSON Schema SSR / flowchart スタイル構文
  （v0.4.1 で content 同伴アセットの自動コピーを追加）
- **v0.5**（Phase 24〜29）tankan スタイル構文の全図種展開 / コードブロックの opt-in 索引 /
  OpenAPI Swagger 2.0・スキーマ一覧 / tankan mindmap・timeline /
  形態素トークナイザ PoC は実測見送り / dogfooding 改善＝404 ページと `lint --fix`
- **v0.6**（Phase 30〜35）検索インデックスの位置情報化（フォーマット v3） / フレーズ検索 /
  ビルドのページ並列化（render・index） / dogfooding 改善＝近接ブースト・フレーズヒント・
  ビルド時間表示 / 検索スタックのライブラリ化と OPFS キャッシュ
- **v0.7**（Phase 36〜38）公開・配布の整備 —
  [ドキュメントサイト](https://ai.implementer.net/yuzu/)を GitHub Pages へ公開 /
  tag push で 4 プラットフォームのバイナリを配布する release.yml /
  [tankan の crates.io 単独公開](https://crates.io/crates/tankan)。
  名前 `yuzu`・`yuzu-core` の取得済み判明により本体の crates.io 公開は将来構想へ再定義
- **v0.8**（Phase 39〜41）執筆機能の拡充 — コードブロックの表示メタ（title / 行ハイライト /
  行番号。JS ゼロ維持） / リダイレクト・エイリアス / dogfooding 改善＝エイリアス診断の行番号・
  コードメタ lint・sitemap.xml・`git.lastUpdated` のサブディレクトリ運用バグ修正
- **v0.9**（Phase 42〜45）執筆機能の拡充 第 2 弾 — コンテンツインクルード（`file=`） /
  図表番号と相互参照 / 折りたたみ（`> [!NOTE]-`） / dogfooding 改善＝折りたたみの自動展開・
  fmt の独自記法温存・図表番号のサイト全体通し番号
  （v0.9.1 でサイドバーのスクロール位置維持を追加）
- **v0.10**（Phase 46〜49）実運用の質を上げる — 診断の機械可読出力
  （`--format {human,json,github}`） / 検証の網羅性（API 仕様の `file:` 参照・
  `yuzu.jsonc` のキー診断） / watch・キャッシュの正しさ / dogfooding 改善＝検索の
  追加読み込み・`yuzu fmt --diff`・scaffold 刷新・SIGPIPE 対応
  （v0.10.1 でコードレビュー指摘の修正を追加＝出力先の境界検証・ページ URL の検証・
  エイリアス `.` の拒否・ハイライト無効時のインクルード欠落修正・走査エラーの伝播・
  URL エスケープ・vendor 取得のバージョンとアーカイブのチェックサム固定。**非互換**:
  `output.dir` がルート外・ルート自身・`input.dir` / `public/` / `theme/` / `.yuzu` と
  重なる場合はエラー、ルートから出力先（と `.yuzu`）までの経路に
  シンボリックリンクがあればエラー、`x.md` と `x/index.md` の共存・
  エイリアス `"."`・ファイル名の URL 危険文字（`#` `?` `%` `"` 等）もエラーになる）
- **v0.11**（Phase 50〜53）執筆機能の拡充 第 3 弾 — タブ / コードグループ（JS ゼロ）/
  Markdown 断片のインクルード（` ```include `）/ 用語集・略語（設定の辞書から
  `<abbr>` 化とページ自動生成）/ dogfooding 改善＝約物に隣接した強調・定義リスト・
  検索結果のセクション絞り込み（エンジン側）・ポート衝突の案内と
  `build --watch` のポート指定・キャッシュ保存の原子化
- **v0.12**（Phase 54〜57）読む体験の完成 — 全文検索の結果専用ページ
  （`?q=` / `?section=` を URL で共有。ドロップダウンはサジェストへ格下げ）/
  印刷・PDF 対応（画面 UI 非表示・常にライト配色・折りたたみとタブの全展開・
  thead 再掲）/ ナビと目次の規模対応（サイドバー折りたたみ・入れ子 TOC・
  `theme.toc.levels`・scrollspy の基準線修正）/ dogfooding 改善＝サイト URL 更新
- **v0.13**（Phase 58〜61）lint の制御性 — ページ単位の抑制（frontmatter
  `lintDisable`）/ 行単位の抑制（`<!-- yuzu-lint-disable-next-line -->` コメント）/
  `lint.rules` の「ルール ID → bool」化による全ルールの enable/disable /
  dogfooding 改善＝抑制記法を docs・scaffold で実運用・SSR 図のモバイル対応。
  Phase 外でビルド進捗ログ（処理中ページ・watch の変更ファイル表示）と
  comrak 整形パニックの防御（該当ページを原文へ縮退）も追加
- **v0.14**（Phase 62〜63）設定基盤の刷新 = TOML 化 — 依存ゼロ・`no_std + alloc` の
  TOML ライブラリ **kabosu** を新設（設計は
  [docs/content/development/kabosu.md](docs/content/development/kabosu.md)。
  [crates.io で単独公開](https://crates.io/crates/kabosu)）/ 設定を `yuzu.jsonc`（JSONC）から `yuzu.toml`（snake_case
  キー）へ全面移行。**非互換**: JSONC の互換読み込み・変換コマンドは無し・
  未知キー / 型違い / 重複キーは設定エラー（exit 2）で停止・`config-unknown-key` /
  `config-duplicate-key` ルールは廃止・`.yuzu/settings.json` は廃止・envKey が
  変わるため移行後の初回ビルドはフルビルド

検索エンジン本体 **mikan**（旧 yuzu-index-format）と wasm ラッパ **mikan-wasm**
（旧 yuzu-search-wasm）は v0.7 リリース後に yuzu- プレフィックスを外して改名し、
mikan は crates.io で単独公開している（tankan と同じく独立バージョン）。

各版の Phase 内訳:

<details>
<summary>完了済み: v0.14（Phase 62〜63）の内訳</summary>

軸は「**設定基盤の刷新 = TOML 化**」。設定は serde ＋ JSONC で読んでいたが、
serde の derive では位置付きの診断が組めず、未知キーの検出は
「`Config::default()` を JSON 化した既知キー木を別経路で走査する」二重実装
（Phase 47 / 60）で補っていた。列位置は取れず、重複キーは後勝ちで黙って上書きされ、
`lint.rules` のタイポ検出も既知キー木の非空 Default という間接的な仕掛けに
頼っていた。設定ファイルを TOML にし、パーサを依存ゼロの自作ライブラリ kabosu
として切り出すことで、span 付き診断・未知キーの方針（Warn / Deny / Ignore）・
正規化出力（envKey 用）を 1 実装で持つ。Phase 62 / 63 は設計書
（2026-08-16 確定）の「v0.1 の対応範囲」と「yuzu への統合」をそのまま切ったもの。

- **62 kabosu v0.1** — 依存ゼロ・純 Rust・`no_std + alloc`・Sans I/O・
  `#![forbid(unsafe_code)]` の TOML ライブラリを `crates/kabosu` に新設（yuzu 非依存）。
  対応範囲は TOML 1.0 のサブセット（bare / quoted / dotted key・標準テーブル・
  単行 basic / literal string・10 進整数・boolean・ネスト配列・コメント）で、
  float / date-time / 進数整数 / 複数行文字列 / inline table / array of tables は
  一般構文エラーにせず**位置付きの `Unsupported`** として返す（設定ファイル用途では
  「書き換え先を案内できる」ことが要点。ただし `Unsupported` は参照実装 = `toml`
  crate が受理する妥当なリテラルに限り、`1e` / `0xGG` / `1979-02-29` のような
  不正リテラルは `InvalidLiteral`）。キー・値・コメントすべてがバイト範囲の span を
  持ち、`KeyPath` は文字列へ平坦化しない。**手書き decode / encode**（derive なし）で、
  `TableDecoder` が必須 / 任意 / 既定値 / ネスト / 未知キー 3 方針を担い、型変換の
  診断は全件蓄積（エラーが 1 件でもあれば値を返さず、上限で省略された分も
  `has_errors` に数える）。正規化出力は同じ値から常に同じバイト列。検証ゲートは
  単体・corpus（valid / invalid / unsupported）・round-trip・正規化 snapshot・
  `toml` crate との差分テスト・fuzz 3 ターゲット（手動 workflow `fuzz.yml`）・
  CI の `msrv` ジョブ（Rust 1.85）・`thumbv7em-none-eabi` の no_std check・
  `cargo package` 後の依存ゼロ検査
- **63 yuzu-config の統合** — 設定を `yuzu.jsonc` から `yuzu.toml`（snake_case
  キー）へ全面移行し、yuzu-config の通常依存を kabosu だけにした（jsonc-parser は
  workspace からも消え、serde / serde_json / thiserror / tracing も yuzu-config から
  外れた）。**非互換を意図的に取る**: JSONC の互換読み込み・フォールバック・変換
  コマンドは作らない（2 形式の解釈を並走させない）。未知キーは Deny = 位置と
  「その階層の対応キー一覧」付きの設定エラー（exit 2）で、型不一致・選択肢外の値・
  `lint.rules` の未知 ID と一緒に全件蓄積して 1 回で出す。重複キーは TOML の構文
  エラー（先の定義の位置付き）。これで `config-unknown-key` / `config-duplicate-key`
  ルールは役目を終えて廃止（`config-path-outside-root` だけ残る）。`codec.rs` の
  `table_codec!` で「キー名 => フィールド」を 1 行ずつ定義し Decode / Encode を同時に
  生成する（集合がズレない。キーを足すときはここにも足す）。`.yuzu/settings.json` は
  代替なしで廃止し、envKey は `Config::to_toml()`（正規化出力）に替えた = 移行後の
  初回ビルドは全ページ再計算。yuzu-config はログを出さず、探索・読み込み・警告表示は
  yuzu-cli の `commands::load_project` に一本化。追随: scaffold の注釈付き
  `yuzu.toml`・docs 13 ページ（`reference/config.md` は TOML で全面書き換え）・
  `docs/yuzu.toml`（インクルード引用は 25〜45 行目）・ci.yml の e2e

</details>

<details>
<summary>完了済み: v0.13（Phase 58〜61）の内訳</summary>

- **58 ページ単位の抑制** — frontmatter `lintDisable` でそのページに限り warning
  ルールを抑制する（HTML コメント案は不採用 = fmt がバイト温存する構造化キーを
  選択。`CACHE_FORMAT_VERSION` 19 → 20）。土台として**ルール ID レジストリ**
  （`yuzu-core/src/rules.rs`。全ルールの ID・深刻度・抑制可否の唯一の定義。docs の
  ルール表との一致をテストで縛る = `SPEC_LANGS` と同型）と適用層（`suppress.rs`。
  check / lint / lint --fix が報告直前に通る単一の漏斗）を新設。抑制できるのは
  warning のみ（error は壊れた出力を防ぐ正・`config-*` はページ外・抑制機構自身の
  2 ルールも不可）。未知・抑制不可の名前は `invalid-lint-suppression`、発火しなかった
  抑制は `unused-lint-suppression` の warning（「黙って効かない」を
  `config-unknown-key` と同じ事故クラスとして扱う）。`--fix` は抑制箇所を書き換えず、
  集計行と `--format json` に抑制件数を追加
- **59 行単位の抑制** — `<!-- yuzu-lint-disable-next-line <rule…> -->` が「空行を
  飛ばした次の内容行」に限り抑制する（disable-line は語彙予約のみ）。収集は行走査
  ではなく comrak AST（HtmlBlock / HtmlInline）なので、コードブロック内の記法例は
  構造的に誤認しない（docs が実例をフェンスで安全に書ける根拠）。文字列解釈は
  `markdown/suppress_comment.rs` に一元化。fmt は対象行との密着形へ正規化
  （restore_yuzu_syntax 拡張 = comrak が HtmlBlock 後に挿入する空行を落とす）。
  閉じ忘れ・裸コメント・行途中・未知ディレクティブは invalid 警告で防御。
  出力 HTML・配信 .md・llms への素通しは仕様と割り切り（検索索引には入れない）。
  CACHE bump 不要
- **60 全ルールの enable/disable** — `lint.rules` を「ルール ID（kebab-case）→
  bool」のマップへ一般化し、`false` でプロジェクト全体無効化。「マップ化すると
  `config-unknown-key` の既知キー木でタイポ検出不能」という策定時の前提は
  **Default を「全 disableable ID → true」の非空マップにする**ことで覆した
  （タイポ・旧 camelCase キー・error 系 ID は行番号付き warning のまま）。
  無効化できる集合はレジストリの suppressible と同一（`DISABLEABLE_RULES` を
  yuzu-config に持ち双方向テストで縛る）。適用は `apply_suppressions` の漏斗一本
  （spec-warning にも同経路で効く。無効化中もルールの計算は走らせ漏斗で落とす =
  「無効化 N 件」の正確な集計と引き換えの意図的トレード）。旧 camelCase 3 キーは
  エイリアスなしで廃止（安全側）。severity 上書きは終了コード規約 0 / 1 / 2 と
  噛み合わないため off のみ
- **61 dogfooding 改善** — 選定 3 点。(1) **抑制記法の実運用**: GFM の表セルは
  行コメントで抑制できない（表の前に置いても対象はヘッダ行のみ。テストで仕様化）
  ため、docs の表 5 箇所は `lintDisable`（Phase 58）・scaffold の箇条書き 3 行は
  行コメント（Phase 59）と使い分けて両記法を dogfood。リスト項目は「1 行目
  ラベル文・2 行目コメント・3 行目例」の形が fmt 正規形（`- <!-- … -->` 同一行形は
  fmt が「`- `（末尾スペース）＋字下げ」へ書き換えるので不採用）。
  (2) **SSR 図のモバイル対応**: `max-width: 100%` の縮小（1571px 幅の図が 375px
  端末で文字 3px）をやめ、pre / table と同じ「等倍＋ figure 内横スクロール」へ
  （svg は block ＋ margin-inline: auto = inline のままだと中央寄せの左端が
  スクロール範囲外へ切れる。印刷は紙幅へ縮小のまま）。(3) **ROADMAP 整理**:
  キャッシュ保存の原子性が Phase 53 実装済みと持ち越し欄に二重記載だったのを解消
- **Phase 外** — ビルド進捗ログ（レンダ / 索引の 1 ページ 1 行・キャッシュヒット
  印付き・watch の「変更を検知」へ変更ファイル表示。`RUST_LOG` を初文書化）/
  comrak `format_commonmark` の既知バグ（引用等の入れ子内の順序付きリストが
  9 → 10 項目で桁が増えると prefix 計算がずれてパニック。0.54 でも未修正）を
  catch_unwind で防御し、該当ページは警告付きで原文へ縮退（fmt は整形スキップ /
  llms-full は原文の本文）

</details>

<details>
<summary>完了済み: v0.12（Phase 54〜57）の内訳</summary>

- **54 全文検索の結果専用ページ** — `?q=` / `?section=` を**状態の唯一の持ち主**とする
  結果ページを合成 `Page`（`GeneratedKind::Search`）として追加し、検索結果を URL で
  共有できるようにした。ドロップダウンは**無条件でサジェスト**（上位 5 件＋
  「すべての結果を見る」）へ格下げし、Phase 53 の遷移後復元（RESTORE_KEY）と
  絞り込みの sessionStorage 保持は削除（URL と状態の持ち主が二重になる事故の芽を摘む）。
  `Page.generated` は bool → `Option<GeneratedKind>` へ昇格（該当は実測 **18 箇所**。
  診断文面の設定キー名は `config_key()` が唯一の定義）し、集約の載せる / 載せないは
  `Page::in_nav / in_search_index / in_sitemap / emits_page_md` に集約（検索ページは
  nav・検索索引・llms・sitemap・ページ単位 .md すべてから除外。llms だけは合成時に
  `frontmatter.llms = false` を立てて既存フィルタに乗せる）。**既定は無効**
  （`search.page` 空）— `content/search.md` を持つ既存プロジェクトを route-conflict で
  壊さないため（scaffold と docs は有効にして配布）。wasm・mikan・インデックス
  フォーマットは不変で、表示件数は `search.pageSize` で決着（Phase 49 の保留分）
- **55 印刷 / PDF 対応** — ダーク定義（theme.css のダークブロック・syntect.css の
  ダークスコープ・cssVarsDark 注入）を **`@media screen` で画面専用化**し、印刷は
  常にライト（syntect のハイライトはリテラル色かつ theme.css より後に読まれるため、
  print 側からの上書きでは詳細度戦争になる。「theme.css だけで完結」の想定は
  yuzu-render css.rs に及んだ）。閉じた details は beforeprint 全開 / afterprint 復元
  （CSS では開けない。開いた分だけ記録して戻す）。タブは CSS のみで全パネル縦展開
  （ラベルは小見出し化）。表は display:table へ戻して thead のページ再掲と行単位の
  改ページ制御を回復。外部リンクのみ URL 併記。mermaid クライアント描画の印刷
  ライト化は**見送り**（beforeprint と非同期 mermaid.run() の競合で未描画の生ソースが
  紙に載る改悪リスク。既定の SSR は SVG 内 <style> の var() 参照で自動追従済み）
- **56 ナビと目次の規模対応** — サイドバーは **`<details>` 折りたたみのみ**
  （`nav.collapse` 既定 on。現在ページの祖先チェーンだけ open・JS ゼロ・summary 内
  リンクでテキスト = 遷移 / マーカー = 開閉）。プルーニングは不採用 — details は
  バイトを減らさないが、削減側は他セクションの子へのワンクリック到達を壊す。
  規模問題の本体は「現在地の迷子」で折りたたみが解決する。`NavCtx` は `open` を追加し
  **active の意味（完全一致）は不変**、毎ページの全ツリー DFS は `NavTrails`
  （ループ外 1 回の route → 祖先チェーン前計算）へ集約。TOC は入れ子化＋
  `theme.toc.levels`（既定 "2-3"）・`<nav>` 化・空 TOC は `:has()` でトラックごと
  畳む。**scrollspy の基準線が `scroll-padding-top` 参照のまま死んでいた既存バグ**
  （実測 66px ずれ）をアンカーの `scroll-margin-top` 実測へ修正。狭幅は Esc /
  ナビリンク / 外側クリックで閉じる最小改修。`nav.auto` は配線も削除もせず
  「予約・効果なし」へ文言を正直化（削除は既存プロジェクトへ未知キー警告が出る）
- **57 dogfooding 改善** — 選定は**サイト URL の更新のみ**（README 6・ROADMAP 2 に
  加え、調査で scaffold の getting-started.md にも 1 箇所発見 = `yuzu new` した全
  プロジェクトに旧 URL が配られていた）。残候補は v0.13 の Phase 61 へ持ち越し

> 策定時のメモ: `prefers-reduced-motion` は現状不要（theme.css に transition /
> animation が 0 件で空振りする。動きを足すときに同時に必要になる）。
> 「クライアント JS ゼロ」はサイト全体の凍結方針ではなく、実効的な規律は
> add-theme-asset スキルの「本文の描画は JS に依存させない＋ UI 補助は縮退可能な
> 外部 JS でよい」（v0.12 で外部 JS は 11 → 13 本になった）。

</details>

<details>
<summary>完了済み: v0.11（Phase 50〜53）の内訳</summary>

- **50 タブ / コードグループ** — 連続するフェンスの `tab="Rust"` を 1 グループへ束ね、
  radio + label + `order` の CSS で**タブ枚数の上限なくクライアント JS ゼロ**で切り替える。
  記法は 2 案を comrak 0.53 で実測したうえで**フェンス情報文字列**を採った。
  `block_directive`（`:::tabs`）案は「同じ長さのフェンスがネストできない」
  「info が丸ごと class に入るのでラベルには AST 介入が要る」「素のビューアで
  `:::` が文字列として見える」の 3 点で落ちた。**決め手はこの Phase の用途
  （言語別サンプル・OS 別手順）がどちらもコードブロックで、`block_directive` の
  唯一の優位点「コード以外もタブにできる」が効かないこと**（散文のタブが要るなら
  後から別記法として足せる。排他ではない）。フェンス案は `yuzu fmt` が情報文字列を
  逐語温存する契約（Phase 39）に乗るので fmt 側の追加作業がゼロ
- **51 Markdown 断片のインクルード** — ` ```include file="snippets/note.md" ` が
  断片を本文の AST へ展開する。共通の注意書き・免責文が複数ページに散って
  片方だけ古くなる問題への対処。**断片は散文専用**（見出し・図表キャプション・脚注・
  frontmatter を `include-error` で弾く）にしたのが設計の要で、これにより
  `extract_meta` は無展開のままでよく、**アンカー採番の 3 経路同期と meta キャッシュの
  無効化がそもそも発生しない**。展開が要るのは本文 HTML と検索の 2 経路だけ。
  入れ子は禁止（検索の deps ハッシュが入れ子の参照先を追えず、Phase 48 で直した
  「参照先を編集しても検索が古い」を再導入するため）。展開は従来の AST 走査より前
  （パス0）に置き、断片ノードがパス1 を通ることで URL 書き換え・ハイライト・
  折りたたみ・数式検出が追加コードなしで効く
- **52 用語集・略語** — `markdown.glossary.terms` に辞書を置くと、本文の Markdown を
  1 バイトも変えずに**ページ内の初出だけ**が `<abbr title>` になり、**用語集ページが
  自動生成**される。Markdown Extra の `*[API]: …` は素のビューアで定義行が見え、
  comrak に該当拡張も無い。**画像 alt の除外は整合性上の必須条件**（comrak は alt を
  生 HTML 不可の文脈で描くため `alt="&lt;abbr …"` に化ける）。見出しとリンクの除外は
  方針（初出を散文で消費させる）。適用は既存の AST 変換がすべて終わった後に回すことで、
  キャプション段落とコードブロックは `HtmlBlock` 化済み ＝ 除外が無料で成立する
  （前段で集めると、後で子を detach される段落へ `insert_before` して**置換が静かに消える**）。
  用語集ページは**合成 `Page` を `pages` へ混ぜる**方式で、nav・パンくず・sitemap・
  検索・route 衝突検査・孤児掃除が既存経路のまま効く。`Page.generated` を足して
  fmt / lint / `edit_url` から外し、**リンク検査ではリンク先としてだけ**有効にする
  （ガードが無いと `yuzu fmt` が実在しない `content/glossary.md` を作ってしまう）
- **53 dogfooding 改善** — 候補から 5 点を選定。**`cjk_friendly_emphasis`**
  （`**「重要」**です` の強調が効く。既存生成物への差分は実測ゼロ）/ **定義リスト**
  （`<dt>` は id を持たないので用語集ページの生成形は据え置き）/
  **検索結果のセクション絞り込み**（下記）/ **ポート衝突の案内と
  `build --watch` の `--port` / `--host`** / **キャッシュ保存の原子化**（`global.json` のみ）。
  絞り込みは区分を**ナビ第 1 階層**にして表示名と並びをサイドバーへ揃え、
  フィルタは BM25 スコアリングの**後**に適用する（前段で落とすと idf がグループ内 df に
  なって絞り込みの有無で順位が入れ替わり、ファセット件数も取れない）。
  **`FORMAT_VERSION` は据え置き** — `manifest.json` は毎回フェッチされるのに
  `search_bg.wasm` は固定 URL で HTTP キャッシュに残るため、上げると再デプロイ直後の
  再訪問者が「新 manifest ＋ 旧 wasm」で検索全停止になる。同じ理由で wasm は既存
  `search()` を変えず `searchIn()` を新設した。副産物として、外側クリックの判定が
  `ev.target.closest()` だったため**再描画で押した要素が DOM から外れると検索が閉じる**
  既存の不具合（「さらに N 件を表示」も踏んでいた）を `composedPath` で直した

</details>

<details>
<summary>完了済み: v0.10（Phase 46〜49）の内訳</summary>

- **46 診断の機械可読出力** — `yuzu check` / `lint` に `--format {human,json,github}` を追加（既定 human で従来の出力は不変）。
  **github 形式は GitHub Actions の注釈として PR の diff 行へ直接出す**。
  パスは `GITHUB_WORKSPACE` からの相対へ自動で付け替えるので、
  ワークフローが `cd docs` してから実行しても正しいファイルに紐づく（**これが無いと注釈が PR に出ない**のが実装上の要）。
  json は単一オブジェクト（`diagnostics` ＋ `summary`）で、
  **内部の `Diagnostic` に derive せず CLI 側に DTO を置いた**（`rel` の基点不明・非 UTF-8 での失敗・`fix` の置換文字列漏れ・`span` のネストを公開契約から切り離すため。
  yuzu-core は無改修）。
  注釈メッセージのエスケープは必須（`broken-link` が URL を生で埋め込むため `%` が実際に出る）。
  副次的に **`check` と `lint` の共通末尾を `diag::report` へ集約**し、`lint` のソート漏れも解消。
  **yuzu-cli 初のユニットテスト 13 本**を追加。あわせて全ルールのリファレンス（`reference/rules.md`。
  当時 16 ルール、現在 20）を新設し、ci.yml の docs 検証を `--format github` へ差し替えた
- **47 検証の網羅性** — `openapi` / `jsonschema` の `file:` 参照を `yuzu check` が検証するようにした（`spec-error` / `spec-warning`）。
  描画は「Err を返さない」方針でエラーボックスにして継続するため、
  **仕様ファイルを消す・壊しても終了コードは 0 のまま**だった。
  とくに `$ref` 先の失敗はエラーボックスにすらならず小さな注記へ縮退するので見落としやすい。
  検証は apispec パーサのある yuzu-render に置き（core に別実装を作ると解釈がズレる）、
  `file:` の解釈とファイル読みだけ core へ移して 1 実装を共有。
  **`yuzu.jsonc` のキーのタイポと重複も診断化**（`config-unknown-key` / `config-duplicate-key`）
  — 既知キー木は `Config::default()` の JSON 化で実行時に得るので手書き定数とのズレが起きず、
  `deny_unknown_fields` は使わない（古いバイナリが新しい設定で落ちる＝前方互換を壊すため）。
  設定ファイルは content の外にあるので `Diagnostic` に基点（`DiagBase`）
  を追加した（`rel` に `..` を入れると JSON の path 契約が壊れ GitHub 注釈も紐づかない）。
  あわせて Phase 46 の契約穴（tracing の既定 writer が stdout で `--format json` を汚す）を修正
- **48 watch・キャッシュの正しさ** — Phase 42（コンテンツインクルード）
  がプロジェクトルート監視へ広げた副作用 3 件を解消。
  ①**検索インデックスのキャッシュがページ source ハッシュだけで判定していた**ため、
  インクルード参照先だけを編集すると本文は更新されるのに**検索結果が古いまま**だった（本文 HTML は external_deps で非対象化済みなのに検索 tf は対象外）。
  参照先の内容ハッシュを**別フィールド**（`searchDepsSha256`）
  で持つ — `sourceHash` へ畳み込むとエントリが丸ごと作り直され、
  メタ・本文・llms まで巻き添えで毎ビルド全ミスになる。
  索引されない引用（`search.indexCode` 無効・特別レンダリング言語）はハッシュ対象から外し、
  フェンス情報文字列の解釈は core の `collect_include_specs` に寄せて check と 1 実装を共有 ②監視除外を **`build.watchIgnore`** で設定可能にした（既定 `**/target` / `**/node_modules`。
  従来は出力ディレクトリと隠しディレクトリだけで `target/` を丸ごと再帰監視していた）。
  glob の解釈は `input.ignore` と同じ core の `IgnoreMatcher` を通し、
  yuzu-server は yuzu-core を知らないまま**述語で受け取る**（凍結した依存グラフを守る）。
  判定は**パス自身＋祖先ディレクトリ**に対して行う — 実機確認で「`**/target/**` は `target/` の**作成イベント自体**に当たらず 1 回だけ再ビルドが走る」
  ことが判明したため。
  **除外はイベントのフィルタで監視登録自体は減らない**（notify にパス単位の除外が無い）
  ③**`yuzu dev` / `build --watch` が `yuzu.jsonc` の変更を取り込む**ようにした（従来は再ビルドもライブリロードも走るのに設定だけ効かず「設定ミスを疑う」
  で時間を溶かした）。`WatchBuild` が設定の持ち主になり、envKey が変わるのでセッションごと作り直す。
  壊れた JSONC では前回の設定で続行してプロセスを落とさない。
  ただし**監視・配信の前提に焼き付いた設定は起動時の値へ固定して警告する**（`output.dir` を差し替えると新しい出力先が監視除外から外れて無限ループになる。
  `baseUrl` / `dev.host` / `dev.port` / `dev.liveReload` / `build.watchIgnore` も同様）
- **49 dogfooding 改善** — 恒例のバッファ枠（ユーザ選定の 3 点＋小粒 1 件）
  : **①検索ドロップダウンの 10 件打ち止めを解消** — 末尾に「さらに N 件を表示（残り M 件）」
  行を置き、
  limit を増やして再クエリして**増えた分だけ追記描画**する（DOM を消さないのでスクロール位置と選択が保たれ、
  fragment fetch もクライアント側のメモ化で増分だけになる）。
  追記が正しいのは**エンジンの並びが (スコア降順, doc_id 昇順) の全順序で、
  limit を増やした結果が前回の厳密な接頭辞になる**ため。
  more 行は `<button>` ではなく **`role="option"`**（listbox に interactive を入れず Tab フォーカスを input に留める）
  で矢印キーの循環に含め、**キーボードだけで「続きがある」ことに気づける**。
  Enter は **IME ガード → more 行 → 遷移**の順（逆にすると Safari の確定 Enter で誤爆、
  分岐漏れは `href` が undefined で `/undefined` へ飛ぶ）。`search-ui.js` + `theme.css` だけで完結し、
  テンプレート無改修＝スナップショット・JS 無効時の挙動は不変。
  設定キーは足さない（v0.11 候補の検索結果ページと意味が衝突するため。
  件数を変えたい人はテーマ上書きで）
   **②`yuzu fmt --diff`** — unified diff を標準出力へ（`--check` を含意して書き換えない）。
  ヘッダはルート相対・`/` 区切り・タイムスタンプ無し・色無しで、
  **`> x.patch` → `patch -p1` がそのまま通る**ことを CI で縛る。
  集計行は stderr（`--format json` と同じ「stdout は契約物だけ」の規律）。
  diff 生成は `similar`（insta 経由で Cargo.lock に既存＝ロック不変・純 Rust）。
  `check` の `fmt` 診断には差分を載せず（github 形式は改行を `%0A` にするため巨大な 1 行注釈になる）
  メッセージから `--diff` へ誘導する **③scaffold の陳腐化を解消** — `index.md` の「5 図種」
  → 9（リポジトリ唯一の食い違いで、次ページには 9 と書いてあった）、機能表を現行へ、
  `snippets/greet.rs` を同梱して**インクルードの動く実例**（`lines=` は使わない = 行範囲の結合を雛形に持ち込まない）、
  `aliases` の実例でリダイレクト HTML が出る状態に、state 図を足して 9 図種そろえ、
  lint 節に規約系ルールとルール一覧への導線、
  `build.watchIgnore` のコメント例 **④SIGPIPE で panic しない** — `yuzu search … \| head` が `failed printing to stdout: Broken pipe`（終了コード 101）
  で落ちていた。
  `libc` で `SIG_DFL` に戻す案は**終了コード 141 が漏れて 0/1/2 規約が壊れる**ので採らず、
  標準出力を `out.rs` へ集約して **BrokenPipe は「以降の出力を捨てる合図」
  **として扱う（本来の 0/1/2 を保つ）。再発防止に `#![deny(clippy::print_stdout)]`。
  **回帰ゲートは 64KB 超の出力＋`head -c 1`＋`PIPESTATUS`**（パイプバッファに収まると EPIPE 自体が起きず空振りする）

</details>

<details>
<summary>完了済み: v0.9（Phase 42〜45）の内訳</summary>

- **42 コンテンツインクルード** — 実ソースファイルの一部をコードブロックへ埋め込む ` ```rust file="src/api.rs" lines=10-25 `（設計書とコードの乖離を防ぐ）。
  fence 情報文字列に `file=` / `lines=` を追加し（Phase 39 の `parse_fence_info` 基盤）、
  読み込みと行切り出しは `yuzu-core::include`（canonicalize でルート配下強制。
  描画・検索・check の 3 経路で共有）。`title` 省略時は `パス:行範囲` を自動キャプション、
  言語省略時は拡張子で syntect 構文を推定、行ハイライトは切り出し後の相対行。
  参照ページは既存 external_deps でキャッシュ非対象（参照先の変更が次ビルドで必ず反映）。
  不在・ルート外・行範囲外はエラーボックスでビルド継続＋`yuzu check` が `include-error` で報告、
  `lines=` 単独等の書き間違いは Phase 41 の `code-block-meta` lint が警告。
  **着手時の 3 判断**: 範囲指定は行番号のみ（region マーカーは不採用）/ 検索（`indexCode`）
  は展開・llms.txt は原文のまま（fmt 正規形との一致を保つ）
  / `yuzu dev` はプロジェクトルート監視へ変更（**出力ディレクトリと隠しディレクトリを除外する仕組みを watch に新設** = 除外なしでは再ビルドの無限ループになる）。
  `CACHE_FORMAT_VERSION` 10→11
- **43 図表番号と相互参照** — 図・表・コードの前後に置く**キャプション行**（`Figure: 説明 {#fig:label}`。
  日本語の `図:` / `表:` / `リスト:` も受理）でページ内自動採番し、本文から参照できるようにした。
  着手時判断: 記法はキャプション行方式（**素の Markdown ビューアでも壊れないただの段落とリンク**）
  / 採番はページ内連番（種別ごとに独立カウンタ）/ 対象は図・表・コードの 3 種。
  参照は空テキストリンク `[](#fig:label)` を「図 1」
  へ自動補完（テキスト付き `[この図](#fig:label)` は著者指定を尊重）。
  実装は `yuzu-core::markdown::crossref`（解釈・採番・HTML 化の単一実装）で、
  採番はメタ抽出（`Page.labels`）と本文 HTML 化の両方を同じ規則・文書順で回して一致させる。
  ラベルは linkcheck の有効アンカーに追加（切れは `broken-anchor`）、
  重複は lint の `duplicate-label` が警告。`CACHE_FORMAT_VERSION` 11→12。
  **AST 操作の注意**: comrak は `descendants()` のイテレート中に木構造を変えるとパニックするため、
  走査では置換対象を集めるだけにして適用は後段で行う（段落 → HtmlBlock 化では子ノードの切り離しも必要）
- **44 折りたたみ** — Admonition の種別直後に `-`（閉じた状態）/ `+`（開いた状態）
  を付けると `<details>` / `<details open>` で描画する（Obsidian callouts 互換。
  ネイティブ要素なのでクライアント JS 不要）。
  comrak は `[!NOTE]-` の `-` をタイトルの一部として渡してくるため、
  `yuzu-core::markdown::collapse` がマーカーを剥がして判定し、
  `<details>` 開始タグ + 中身 + 終了タグへ **AST 上で組み替える**（comrak に details 出力が無いため。
  Alert ノードの子を外へ移して自身は detach）。
  タイトル省略時は comrak と同じ既定ラベル（Note / Tip …）。
  テーマ CSS は既存の `.markdown-alert` 共通ルールがそのまま効くので summary のクリック領域だけ追加。
  折りたたみの中身は閉じていても HTML に含まれるため検索・llms.txt にそのまま収録される。
  `yuzu fmt` は `> [!NOTE] - タイトル` の形へ正規化するが解釈は不変・冪等（docs に注記）
- **45 dogfooding 改善** — 恒例のバッファ枠（ユーザ選定の 3 点）
  : **①折りたたみの自動展開** — 検索結果・目次・図表参照から `<details>` の中へアンカーで飛んだとき祖先を開いて該当箇所を見せる（`details-target.js`。
  プログレッシブエンハンスメントで JS 無効でも中身は HTML にある。
  ページ内検索の自動展開はブラウザ側の対応に委ねる = 閉じた details の中身は details 自身が隠すため `hidden=until-found` は効かない）。
  **②fmt 正規化の見た目改善** — `format_commonmark` は `#` を無条件にエスケープし（`{#fig:x}` → `{\#fig:x}`）
  Admonition のタイトル前に空白を入れる（`[!NOTE]-` → `[!NOTE] -`）。
  どちらも comrak 側にオプションが無いため、
  **fmt 出力に対象を絞った復元処理**（行末ラベルと Admonition マーカーだけ）
  を入れて書いた形を保つようにした（通常の `#` エスケープは従来どおり）。
  **③図表番号のサイト全体通し番号** — `markdown.crossref.numbering: "site"`（既定 `"page"`）
  でサイドバー表示順の通し番号にする。
  オフセット割り当ては nav とラベルの両方を持つ `build_site_model` で行い、
  先行ページの図表増減が後続ページの番号を変えるため routesKey にラベル個数を含めて本文キャッシュを無効化する。
  OG メタ・favicon は方針どおり対象外

</details>

<details>
<summary>完了済み: v0.8（Phase 39〜41）の内訳</summary>

- **39 コードブロックの拡充** — フェンス情報文字列を拡張し ` ```rust title="src/main.rs" {2,4-6} showLineNumbers ` の形で**ファイル名キャプション・行ハイライト・行番号**に対応。
  行番号のサイト既定は `markdown.highlight.lineNumbers`（既定 false）で、
  ブロック単位の `showLineNumbers` / `noLineNumbers` が優先。
  パースは `yuzu-core`（`markdown/fence.rs` の `parse_fence_info` = HTML 化と検索抽出の単一実装。
  `CodeBlockRenderer` trait に `CodeBlockMeta` を追加）、描画は `yuzu-render`（`highlight.rs`）。
  **全コードブロックを行 span 化**（syntect の一括 HTML を `split_lines_balanced` で行ごとに自己完結化 = 行またぎ scope は行末で閉じ次行頭で開き直す）
  し、
  キャプション = `<figcaption>`・行ハイライト = `hl` クラス・行番号 = CSS カウンタで**クライアント JS ゼロ維持**。
  改行は span 内に残すためコピーボタン（`code.textContent`）は改行を保ち、
  行番号・キャプションは混入しない。
  未知言語・言語なしでもメタか行番号指定があればエスケープ済みプレーン本文を同構造で描画（指定なしは従来どおりパーサ既定）。
  特別レンダリング言語（mermaid / openapi / jsonschema / math）はメタを無視。
  検索はコード本文だけを索引（メタ非混入）・`yuzu fmt` は情報文字列を逐語温存（冪等をテストで担保）
  ・`CACHE_FORMAT_VERSION` 8→9。scaffold と docs サイトに実例を追加
- **40 リダイレクト / エイリアス** — frontmatter `aliases`（旧 URL の配列。
  先頭 `/`・末尾スラッシュ省略は正規化で吸収）から、
  旧パスへのリダイレクト HTML（`redirect.jinja` = meta refresh + canonical + `noindex` + JS フォールバック。
  テーマ上書き可）をビルド時に生成（静的ホスティングにサーバリダイレクトが無いための定石）。
  リダイレクト先は `UrlResolver` 経由で baseUrl に追随。出力はマニフェストに記録され、
  エイリアス削除時は孤児掃除で自動的に消える。
  検証は `yuzu-core::validate_aliases` に集約（形式不正 `alias-invalid`・実ページ route / 他エイリアスとの衝突 `alias-conflict`）
  : `yuzu check` は draft 込みの全ソースでエラー報告（exit 1）、
  render_site は書き出し前に検証して中断（exit 2。実ページの上書き事故をレンダラ自身でも防ぐ）。
  エイリアスは検索・llms.txt の対象外で、
  linkcheck の有効ターゲットにも含めない（本文からエイリアス URL へのリンクは check が指摘 = 内部リンクは常に正 URL へ）。
  `KNOWN_KEYS` へ追加・`CACHE_FORMAT_VERSION` 9→10（CachedMeta の Frontmatter 変更）。
  docs サイトで実運用（`guide/lint/` → 品質チェックページ。ci.yml にゲート）
- **41 dogfooding 改善** — 恒例のバッファ枠（ユーザ選定の 3 点）
  : **①エイリアス診断の行番号** — `alias-invalid` / `alias-conflict` に frontmatter の該当行 span を付与（値の行 → `aliases:` キー行 → frontmatter 全体のフォールバック。
  `validate_aliases` が `MarkdownOptions` を受ける形に）。
  **②フェンス情報文字列のタイポ検出** — lint 新ルール `code-block-meta`（Warning・常時有効）
  : 未知トークン（`showLineNumber` 等のタイポ）
  ・`{2,x}` の解釈不能要素・コード行数を超える行ハイライト・特別レンダリング言語への表示メタ指定（無視される旨）
  を行番号付きで警告。描画は従来どおり寛容（挙動を変えるのは lint だけ。
  `parse_fence_info_detailed` で「何を無視したか」を返す）。
  **③sitemap.xml の自動生成** — baseUrl がフル URL のときだけ全ページを `<loc>` 絶対 URL で列挙（`git.lastUpdated` 有効なら `<lastmod>` 付き）。
  エイリアス・404 は載せず、`public/sitemap.xml` で上書き可・孤児掃除対象。
  **④（検証中に発見したバグ修正）
  `git.lastUpdated` がサブディレクトリ運用で全滅していた問題** — `git log --name-only` のパスはリポジトリルート相対のため、
  yuzu プロジェクトが git リポジトリのサブディレクトリにある場合（monorepo 内の docs/ 等）
  に content プレフィクスの除去が全ファイルで失敗し、日付が静かに空になっていた。
  `--relative` を追加してプロジェクトルート相対に揃えて修正（= 自ドキュメントサイトのフッター最終更新日と sitemap の `<lastmod>` はこの修正で初めて機能）。
  OG メタ・favicon は方針どおり対象外

</details>

<details>
<summary>完了済み: v0.7（Phase 36〜38）の内訳</summary>

- **36 yuzu 自身のドキュメントサイト公開** — dogfooding の総仕上げとして、
  yuzu 自身のドキュメントを yuzu で書いて GitHub Pages に公開（https://ai.implementer.net/yuzu/ ）。
  `docs/` をこのリポジトリ自身の yuzu プロジェクトにし（`docs/yuzu.jsonc` ＋ content 16 ページ: トップ＋ガイド 9・リファレンス 3・開発 3。
  現在は 17 ページ）、README をページ階層へ再構成。
  主要機能を実運用で使用: tankan SSR（9 図種ギャラリー＋ワークスペース依存図。
  フォールバック 0 を CI でゲート）・OpenAPI（インライン＋ `file: specs/sample-api.yaml` 参照）
  ・数式・検索（`indexCode` / `synonyms` / `lint.terms` クエリ拡張を有効化）
  ・`lint.maxDirectoryDepth`・git 連携メタ（`fetch-depth: 0` で lastUpdated）。
  デプロイは `.github/workflows/docs.yml`（main push で自前 release バイナリをビルド → `yuzu check` 品質ゲート → `--base-url` に Pages のフル URL を注入 = llms.txt が絶対 URL → Pages へ配置）。
  ci.yml にも docs の check・build・SSR フォールバック検出を追加し、壊れた原稿はマージ前に検出する
- **37 配布整備（バイナリ配布）** — 当初案は全クレートの依存順 crates.io 公開だったが、
  着手時調査で `yuzu`・`yuzu-core` の名前が crates.io 上の別プロジェクトに取得済みと判明（crates.io に namespace はなく、
  公開には依存クロージャ全公開が必須 = 部分公開は不可）。
  公開単位の検討の結果 **crates.io は今回使わない**と決定し、
  将来 tankan・検索スタック（yuzu-index-format）を切り離し公開した後に、
  それらへ依存する形で yuzu 本体を公開する構想へ再定義（v0.8 以降候補へ）。
  誤公開防止に全 10 crate へ `publish = false` を明示。
  バイナリ配布は tag push トリガーの `.github/workflows/release.yml`（手書き matrix + gh CLI・サードパーティアクション不使用）
  : タグ整合ガード（`v` + workspace バージョン一致・main 包含 = CI 済み担保）
  → macOS arm64/x64（arm64 runner からクロス）・Linux x64（ubuntu-22.04 = glibc 2.35 基準）
  ・Windows x64 を `--release --locked` ビルド → `--version` smoke → draft Release へ集約 → SHA256SUMS 添付 → 公開。
  部分失敗は Re-run failed jobs だけで復旧（--clobber + draft 非公開）。
  workflow_dispatch でタグなし検証可。
  README / docs のインストール手順をバイナリ + `cargo install --git` へ更新
- **38 tankan の分離公開** — Mermaid 互換 SSR だけを求める非 yuzu ユーザーへの訴求が目的。
  着手時判断で **monorepo のまま crates.io 単独公開**に決定（リポジトリ分離は開発の往復コストが恒常的に増えるため、
  需要を見て後日判断。後からの分離はいつでも可能）。
  tankan を **workspace と独立のバージョン 0.1.0** へ切り替え（yuzu のリリースと非同期に tankan の変更時だけ版を上げる）、
  `publish = false` を除去し crates.io メタデータを整備（description は英語化・keywords / categories・`readme`）。
  README の対応状況表を現行化（mindmap・timeline 追加、state / ER / class のスタイル構文対応を反映、
  `cargo add tankan` 導線）。ci.yml に `cargo package --locked -p tankan` ゲートを追加し、
  CLAUDE.md に tankan の公開手順を記録。実公開は `cargo publish -p tankan`（ユーザ実行）

</details>

<details>
<summary>完了済み: v0.6（Phase 30〜35）の内訳</summary>

- **30 検索インデックスの位置情報化（フォーマット v3）
  ** — postings に term の出現位置（セクション内トークン位置の delta varint 列。
  tf は見出し重み付きで出現数と一致しないため件数 varint を明示）を追加し `FORMAT_VERSION` 2→3。
  フィールド間（タイトル/見出し/本文）に位置ギャップを挟んで偽隣接を防ぐ。
  エンジンは位置を読み飛ばすだけで挙動不変（BM25 据え置き）＝フレーズ照合の土台のみ。
  `CachedSection` の変更に伴い `CACHE_FORMAT_VERSION` も上げる。
  **サイズ実測ゲート**: `dist/_search` 合計（素/gzip）の現行比を計測し「静的ホスティングだけで動く」
  方針と照合 → **通過**（scaffold 2 ページ: 合計 gzip +0.3%・語彙が極端に密な合成 301 ページ: 合計 gzip +14.0%〔1.18MB→1.34MB。
  postings 小計は 7.6KB→173KB〕。Phase 28 で見送った 9〜35 倍とは桁違いに小さい）。
  v2/v3 で `yuzu search` の結果はスコアまで完全一致を確認。wasm 再 vendor 済み
- **31 フレーズ検索（クエリ照合＋UI）
  ** — `"..."` 引用符でフレーズ指定（**引用符なしの既定挙動は不変**。全角・カーリー引用符も受理、
  閉じ忘れは末尾まで）。引用部はトークナイズ→位置の隣接照合で **filter**（含まない doc を除外。
  スコア加点は構成 term の BM25 が担う）。タイポ・同義語展開の対象外＝完全一致のみで、
  語彙に無いフレーズは 0 件。セクションまたぎ非対応。
  抜粋・ハイライトはフレーズ全体を 1 needle ＋隣接マージで 1 まとまりにマーク。
  実装は SearchEngine（yuzu-index-format）1 箇所で native/wasm 共有、
  CI e2e にフレーズ実ヒット・逆順 0 件の検証を追加、wasm 再 vendor 済み（481→492KB）
- **32 ビルドのページ並列化（render）
  ** — `render_site` のページループ（本文 HTML 生成〜テンプレート〜書き出し）を rayon で並列化。
  前提リファクタとしてハイライタのページ内状態をページローカルな `PageCodeRenderer` へ分離（`Cell` の `!Sync` が誤共有をコンパイル時に防ぐ）。
  集約（nav / llms / 404 / アセット）は直列のまま＝層構造不変。
  **決定性ゲート通過**: スレッド数 1/N・並列化前バイナリとの `diff -r` バイト同一。
  実測（release・--force）
  : render 支配のコーパス（201 ページ・ハイライト 1,200 ブロック＋mermaid SSR 200 図）
  で **2.07s → 0.69s（3.0 倍）**、
  テキスト主体 301 ページは 0.53s → 0.48s（トークナイズ支配 = Phase 33 の領分）。
  rayon は「凍結した設計判断」表へ追記
- **33 ビルドのページ並列化（index）
  ＋実測** — 検索インデックスのページごとトークナイズ（compute_sections）を rayon 並列化。
  キャッシュ判定を先行パスに分け、
  miss があるときだけトークナイザを 1 回構築して `&Tokenizer` を共有（vaporetto Predictor は `Sync`＝コンパイルで確認）。
  集約（doc_id 採番・postings・fst）
  はページ順の直列のままで決定性維持（スレッド 1/N・改修前バイナリと `diff -r` バイト同一）。
  **実測（release・M 系 Mac）
  **: テキスト主体 301 ページのフル 0.54s→0.41s・1,001 ページのフル 1.6s→1.1s（1 スレッド比。
  無変更 0.33s・1 ページ編集 0.39s）。render 支配なら Phase 32 の 3.0 倍が効く。
  残る直列部はメタ抽出（comrak）・モデル展開・fst/書き出し
- **34 dogfooding 改善** — 恒例のバッファ枠: **近接ブースト**（引用符なしの複数語クエリで、
  クエリ順に隣接出現するページを ×1.2/ペア のスコアで上位へ。
  フレーズ照合と同じ位置ロジックの soft 版で、ヒット集合は不変・タイポ/同義語展開語は対象外）
  ・**フレーズ検索の発見性**（検索ドロップダウン末尾に `"..."` 構文のヒントを常時表示。
  引用符使用時は消える）・**ビルド時間の表示**（`build`/`dev` の完了ログに elapsed を追加。
  並列化の効果が見える）。OG メタ・favicon は今回も見送り
- **35 検索スタックのライブラリ化＋OPFS キャッシュ** — 外部記事（DuckDB-Wasm/Lindera-Wasm/OPFS 構成のオフライン検索）
  を受けて調査した結果、トークナイザ差し替えは Phase 28 の却下理由（転送量 9〜35 倍）
  がそのまま当てはまるため**見送り、vaporetto＋自作 BM25 エンジンは維持**。
  代わりに (1) 集約ロジック（doc_id 採番・postings・fst・シャード分割・manifest 構築）
  を `yuzu-index`（yuzu-core 依存）から `yuzu-index-format::build`（yuzu-* 非依存）へ移設し、
  tankan と同水準の「分離可能な設計」を検索スタックにも適用、
  (2) `Manifest` に `contentHash`（terms.fst＋全シャード＋モデルバイトの sha256、
  `#[serde(default)]` で後方互換）を追加し、ブラウザ側 OPFS（Origin Private File System）
  キャッシュの版管理に使用。
  フェッチ・OPFS・wasm 起動のオーケストレーションは `crates/yuzu-search-wasm/js/search-client.js`＋汎用ブロブキャッシュ `opfs-cache.js`（新規、
  DOM 非依存）に切り出し、テーマの `search-ui.js` は DOM/UX 層に純化。
  OPFS は contentHash 不一致 or 非対応環境で即座にフェッチのみ経路へフォールバック（`yuzu search` ネイティブ CLI は無関係・無改修）。
  **サイズ実測ゲート**: scaffold 2 ページで `dist/_search` 合計が raw 922,722→931,133B（+0.91%）
  ・gzip 626,774→630,538B（+0.60%）。
  新規 JS は語彙量に依存しない固定コスト（`search-client.js` 4.9KB＋`opfs-cache.js` 2.7KB）で、
  `search_bg.wasm` は 494KB のまま実質不変（Cargo 依存・エクスポート API を変えていないため）。
  決定性テスト（`content_hash` は同一入力で同一値・内容変更で別値）を追加

</details>

<details>
<summary>完了済み: v0.5（Phase 24〜29）の内訳</summary>

- **24 tankan スタイル構文の全図種展開** — flowchart で対応した `classDef` / `class` / `:::` / `style`（＋fill 明度からのラベル色自動選択）
  を **state / ER / class 図**へ展開。適用先は状態ボックス・エンティティ・クラスボックスで、
  色付きボックスはタイトル帯含め全体を塗り全テキストを自動読みやすい色に。
  共通ロジックは `tankan::common::style` に集約。
  class 図は宣言の `class` と衝突しないよう一括適用を `cssClass` に
- **25 検索: コードブロックの opt-in インデックス** — `search.indexCode`（既定 off）
  でフェンスコードブロック本文を検索対象に追加。関数名・設定キーで設計書を引ける。
  tf 重みは本文と同じ 1・コードは抜粋にも出す（merge）
  ・特別レンダリングされる言語（mermaid / openapi / jsonschema / math。
  無効化してプレーン表示なら索引対象）は除外・インデントコードは対象外・llms.txt には非混入。
  envKey が on/off を拾いキャッシュ自動無効化
- **26 OpenAPI レンダリングの拡充** — Swagger 2.0 対応（`definitions` の `$ref` は既存機構で解決・`in: body` はリクエストボディ表示・responses 直下の `schema`・`produces`/`consumes` のメディアタイプ表示は operation が top-level を上書き。
  host/basePath 等は非表示）と、
  **全スキーマ一覧の描画**（`components/schemas` / `definitions` を文書末尾に閉じた details で。
  操作から参照されないスキーマも読める）。2.0 分岐は `SpecVersion::V2` に隔離し 3.x パスは挙動不変
- **27 tankan 新図種** — **mindmap と timeline** を SSR 追加。
  mindmap は中央ルート左右振り分けの tidy tree（インデント階層パース・7 形状・ブランチごとのパレット色）、
  timeline は等間隔カラム＋セクション帯＋イベント縦積み。
  幅ベースの自動折返し `wrap_text` を common に新設（日本語は文字単位・ASCII は単語境界）。
  I/O なし・時刻非依存の設計原則は維持、corpus 11 本＋スナップショット 6 枚
- **28 形態素トークナイザ PoC** — vibrato / lindera への差し替えを実測し（wasm サイズ・精度・速度・辞書配布）、
  **見送り = 現行 vaporetto + SUW 継続を決定**。
  根拠: 差し替えは合計転送量が現行 ≈450KB の 9〜35 倍（vibrato+ipadic ≈7.8MB / lindera embed-ipadic は wasm 58MB・gzip 15.8MB）
  で「静的ホスティングだけで動く」方針と衝突。精度改善は辞書語の 1 語化に限られ、
  ipadic の誤分割（ワークス/ペース）やカタカナ連結による部分語 recall 低下も確認。
  SUW 細分割の弱点は同義語・タイポ機構（Phase 20/21）で緩和済み。
  v0.6 のフレーズ検索はトークナイザ据え置きで位置情報インデックスのみで実現する
- **29 dogfooding 改善** — 実運用の不満の一括解消（バッファ枠）
  : **404 ページの生成**（テーマ統合・検索ボックス付き `404.html`。
  Pages デプロイ雛形同梱なのに直リンク切れが素の 404 だった穴。
  `public/404.html` で上書き可・`preview`/`dev` も 404 ステータスで配信）
  と **`yuzu lint --fix`**（表記ゆれ lint は変換候補まで出すのに適用が手作業だった穴。
  全角英数字・半角カナ・`lint.terms`・長音符ゆれ多数派を自動適用。
  冪等・mtime 温存・同数タイは報告のみ）

</details>

<details>
<summary>完了済み: v0.4（Phase 19〜23）の内訳</summary>

- **19 表記ゆれ lint の組み込みルール** — `fullwidth-alphanumeric`（全角英数字。
  半角の変換候補付き）・`halfwidth-kana`（半角カナ。濁点合成込みの変換候補付き）
  ・`katakana-choon`（長音符ゆれの混在をプロジェクト横断の多数決で検出。少数派の出現箇所に警告）。
  既定有効・`lint.rules` でルール単位の無効化可
- **20 検索の用語ゆれ・同義語対応** — `lint.terms` ＋ `search.synonyms` を manifest 経由でクエリ拡張に使用（同義語 = weight 1.0、
  変形上限 8）。ハイライトも同義語側に対応。実装は SearchEngine（yuzu-index-format）
  1 箇所で native/wasm 共有、wasm 再 vendor 済み
- **21 検索 UX の磨き込み** — **日本語タイポトレランスの修正**（levenshtein_automata の文字単位 DFA へ置換。
  CI e2e も実ヒットを検証するよう強化）＋検索 UI の改善: 結果件数表示（`search_with_total`）
  ・IME 変換中の検索抑制とキー競合回避・ローディング表示・未選択 Enter で先頭ヒットへ・aria-selected / aria-activedescendant の同期
- **22 OpenAPI / JSON Schema レンダリング** — ` ```openapi ` / ` ```jsonschema ` ブロックのビルド時 SSR（自前実装・JS ゼロ・テーマ統合）。
  インラインと `file:` 参照（ルート相対・ルート外拒否）の両対応、`$ref` ローカル解決＋循環ガード、
  参照ページはキャッシュ非対象で仕様変更が即反映。失敗はエラーボックスでビルド継続
- **23 dogfooding 改善** — 積み残しの一括解消（バッファ枠）
  : tankan flowchart のスタイル構文 SSR（`classDef` / `class` / `:::` / `style`）
  ・OpenAPI のプロジェクト内ファイル間 `$ref` 解決（参照元ファイル相対・ルート外拒否・参照ページはキャッシュ非対象）
  ・小粒の磨き込み（trace メソッド・description 二重表示修正・ドキュメント陳腐化）。
  リリース後の v0.4.1 で content 同伴アセット（ページ横の画像）の自動コピーと相対参照の URL 解決を追加

</details>

<details>
<summary>完了済み: v0.3（Phase 13〜18）の内訳</summary>

- **13 執筆の即効改善** — draft プレビュー（`dev --drafts` / `build --drafts` で下書きをバナー付き表示、
  通常ビルドに戻すと出力は自動掃除）
  ・Mermaid client 描画のダークモード切替時の再描画（既知の制限の解消）
  ・テーマ CSS 変数の設定化（`theme.cssVars` / `cssVarsDark`。値の検証込み）
- **14 ページ単位 .md 配信とページコピー** — 各ページの原文 Markdown を `dist/<route>.md` に配信して llms.txt を `.md` リンク化（vitepress / docusaurus プラグインで優勢の形式）
  ＋各ページに「Markdown をコピー」ボタンと `.md` リンク（fetch → クリップボード。
  コードコピーと同じプログレッシブエンハンスメント）
- **15 日本語 lint: 用語統一** — `lint.terms` のプロジェクト用語辞書による用語統一チェック（`term-variant`）
  を `yuzu lint` / `check` に統合。本文・見出し・リンクラベルを行番号・列番号付きで報告し、
  コード・URL・正表記の部分一致は対象外。組み込みルール（全角/半角等）は実運用の需要を見て拡張
- **16 tankan 図種追加** — 設計書頻出の **class 図**（3 区画ボックス・関係 8 種・多重度・ジェネリクス・アノテーション）
  と **pie**（showData・凡例・CSS 変数パレット）を SSR 対応。
  corpus 13 本＋スナップショット＋wasm32 担保
- **17 git 連携メタ** — `git.lastUpdated`（1 回の git log で全ページの最終コミット日を収集しフッター表示）
  ・`git.editUrl`（`{path}` 置換の編集リンク）。git 実行は cli 層のみ（render はデータ注入）で、
  git 不在・未コミットは表示なしに縮退
- **18 dogfooding 改善** — 実運用で踏んだ不満の解消: **JSONC 重複キーの警告**（後勝ちで設定が黙って無視される事故の検出。
  `site.title` 形式のパス付き）
  と **`yuzu dev --host` / `preview --host`**（コンテナ内から 0.0.0.0 で配信する用途。設定より優先）

</details>

<details>
<summary>完了済み: v0.2（Phase 7〜12）の内訳</summary>

- **7 執筆表現** — Admonition（`> [!NOTE]`、comrak alerts 拡張＋テーマ CSS）
  ・脚注（footnotes 拡張）・コードブロックのコピーボタン（プログレッシブエンハンスメント JS）。
  fmt / llms.txt との整合（format_commonmark の出力確認）込み
- **8 数式** — comrak math（`$...$` / `$$...$$`）→ KaTeX 描画。
  クライアント描画か SSR かの設計判断・vendor 資産の同梱方針を含む
- **9 ページナビ** — 前/次ページリンク（nav 順から導出）＋階層パンくず。
  テンプレート＋nav モデルの拡張
- **10 検索セクション単位化** — fragment を見出し単位に分割して `#アンカー` へ直接ジャンプ＋クエリ一致箇所周辺の動的抜粋。
  index フォーマット変更のため wasm/native トークナイザ整合制約に注意
- **11 デプロイ雛形** — GitHub Pages デプロイ用 Actions ワークフローを `yuzu new` の scaffold に同梱（baseUrl 設定の導線込み）
- **12 インクリメンタルビルド** — `.yuzu/cache/` のページ単位キャッシュで build / dev の再ビルドを短縮（常時有効・`--force` で全再計算）。
  未変更出力は書き込みスキップ（mtime 温存）＋削除ページの孤児出力をマニフェスト差分で掃除

</details>
