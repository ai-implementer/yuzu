# ロードマップ

yuzu の開発計画と、これまでのリリースの内訳。**このファイルが Phase 状態の正**
（README には現在の版と概要だけを置く）。

## 現在

**v0.16 まで公開済み**。次の版（v0.17）は未策定で、候補は下の
「[v0.17 以降の候補](#v017-以降の候補)」にある。着手時に軸を 1 つ選んで Phase を切る。
kabosu 0.2.0 / tankan 0.2.0 / mikan 0.2.0 は crates.io で公開済み（yuzu のリリースとは
非同期。kabosu の publish 前に fuzz を回す規律は CLAUDE.md にある）。

## v0.10.1 レビューの持ち越し

v0.10.1（外部コードレビュー対応）で「今回は入れない」と判断したもの。
**判断の根拠ごと残す**（同じ検討を繰り返さないため）。

- **URL のパーセントエンコード全面対応** — v0.15 の Phase 64 で実装済み（内訳は下の「完了済み: v0.15」）
- **キャッシュ保存の原子性** — Phase 53（v0.11）で実装済み（`write_atomic_under`
  が global.json のみ tmp → rename = 当時の見積もりどおり安価な側だけ。詳細は
  v0.11 の内訳を参照）
- **`syntect.css` の無条件出力** / **`ServeDir` が dist 内のリンクを辿る** — v0.15 の
  Phase 65 で実装済み
- **`.devcontainer/post-create.sh` の Claude Code インストーラ** — 取得した
  `install.sh` を検証せず bash へ渡している。ただし**インストーラ自身が
  ダウンロードしたバイナリを SHA-256 検証している**（バージョンごとの
  `manifest.json` の値と照合し、不一致なら削除して終了）ので、残るギャップは
  スクリプトの TOFU のみ。`install.sh` に公開チェックサムが無く、ベンダ更新のたびに
  devcontainer のビルドが壊れるため固定は見送った。バージョン指定
  （`bash -s -- <version>`）は可能なので、必要になったらそこから

## v0.17 以降の候補

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
- **v0.15**（Phase 64〜67）正しさ・堅牢性 — URL のパーセントエンコード
  （route → URL の変換点を 1 つに決め、非 ASCII も含めて本文・ナビ・llms・sitemap・
  検索索引で同じ表記。著者のエンコード済み参照と aliases はデコードして照合）/
  配信のシンボリックリンク遮断と `syntect.css` の条件出力（テーマ上書きは
  デフォルトテーマの変更へ追随する契約を明文化）/ 外部リンク切れ検査の opt-in
  （`yuzu check --external-links`。HTTP は curl へ委譲し、4xx だけ warning・環境要因は
  `summary.skipped` へ）/ dogfooding 改善＝docs の外部リンク検査を週次実行・
  preview のリンク遮断 e2e。**非互換**: `unsafe-page-path` はファイル名では
  `\` と制御文字だけに縮小（`#` `?` `%` 等を含むファイル名が受理される）一方、
  `markdown.glossary.page` / `search.page` / `aliases` は Windows 予約文字を全 OS で
  拒否・非 ASCII を含む URL がパーセントエンコード形になる（本文リンクは従来どおり）・
  `highlight.enabled = false` で `syntect.css` を出力しない（`base.jinja` を上書きしている
  利用者は追随が要る）・preview / dev がシンボリックリンクを辿らない
- **v0.16**（Phase 68〜71）kabosu の TOML 1.0 完全対応 — 未対応だった 6 構文
  （float / date-time / 16,8,2 進整数 / 複数行文字列 / インラインテーブル /
  テーブルの配列）を実装し、公式 [toml-test](https://github.com/toml-lang/toml-test) の
  TOML 1.0.0 対象ケース（valid 205 / invalid 474）を全通過して
  [kabosu 0.2.0](https://crates.io/crates/kabosu) を公開。TOML 1.1 でだけ妥当な記法
  （`\e` / `\xHH`・インラインテーブルの改行と末尾カンマ・秒を省略した時刻）は
  `Unsupported(TomlV11)` として「1.0 には無い記法」と案内する。**yuzu 本体の機能追加は
  無し**で、`yuzu.toml` に書ける構文が増えるだけ（インラインテーブル・テーブルの配列・
  日時・小数・複数行文字列が「未対応の構文」エラーにならなくなった）

検索エンジン本体 **mikan**（旧 yuzu-index-format）と wasm ラッパ **mikan-wasm**
（旧 yuzu-search-wasm）は v0.7 リリース後に yuzu- プレフィックスを外して改名し、
mikan は crates.io で単独公開している（tankan と同じく独立バージョン）。

各版の Phase 内訳:

<details>
<summary>完了済み: v0.16（Phase 68〜71）の内訳</summary>

軸は「**kabosu の TOML 1.0 完全対応**」。v0.14 で「設定ファイル用途のサブセット」として
切り出したまま crates.io へ公開していたが、TOML 1.0 全体を受理できないパーサは利用側で
「書ける TOML が分からない」摩擦になる。未対応 6 構文を実装し、公式 toml-test を通して
kabosu 0.2.0 を公開した。yuzu 本体の機能追加はしていない。Phase は lexer → 値型 →
構造 → 検証・公開の依存順で、各 Phase は着手時に判断点を決めてから実装し、
レビュー指摘（計 4 件）と fuzz の検出（1 件）を同じ PR で取り込んだ。

- **68 数値と文字列** — lexer が持っていた「TOML として妥当なリテラルか」の判定
  （`is_valid_float` 等）を値の構築へ昇格させ、float / 16,8,2 進整数 / 複数行文字列を
  受理する。判断は 2 点とも推奨案: `Decode for f64` は整数リテラルを受けない
  （型厳格。`String` / `bool` / `i64` と同じ規律）/ 正規化は `{:?}` の最短表現に `.` も
  `e` も無ければ `.0` を補い、`inf` / `-inf` / `nan`（符号は落とす）・`-0.0` は保持。
  進数整数は `Value::Integer` へ畳んで表記を保持しない。複数行文字列は
  `read_string_value` が単行 / 複数行 × basic / literal を振り分け、キー位置の `"""` は
  `MultilineStringAsKey`、閉じ直前の引用符 3 個以上は `TooManyQuotes`
- **69 日時** — 依存ゼロを保つため独自型（`Datetime` / `Date` / `Time` / `Offset`）を持ち、
  時刻演算・タイムゾーン変換・他の日時 crate への変換は持たない。判断は 3 点とも推奨案:
  参照実装と同型の 1 型で 4 種を表す（区別は `Datetime::kind`）/ 小数秒は 9 桁まで保持し
  10 桁目以降は切り捨て / オフセットは分単位の数値だけを持ち 0 は `Z` へ正規化。
  **フィールドは非公開でコンストラクタが範囲を検証する**ため、暦として存在しない日付を
  組み立てて不正な TOML を出力できない。妥当性の判定と値の構築は `parse_datetime_str` の
  1 実装で兼ねる（文法を 2 箇所に書くとズレる）。空白区切り（`1979-05-27 07:32:00`）は
  `read_scalar_blob` が「前が妥当な日付で後ろが `HH:`」のときだけ空白 1 個をまたぐ
- **70 インラインテーブルとテーブルの配列** — これで TOML 1.0 の全構文が揃った。
  判断は 2 点とも推奨案: 要素が全部テーブルの配列は `[[a]]` へ展開 / 到達不能になる
  `EncodeErrorKind::TableInArray` は削除。`TableOrigin` に `Inline`（閉じている）と
  `ArrayHeader` を追加し、**配列が `[[...]]` 由来かは「最後の要素が `ArrayHeader` 起源の
  テーブルか」で判定**する（`Value::Array` に印を足さずに済み、`a = [{...}]` は静的配列の
  まま）。ヘッダー経路は `walk_intermediates` / `can_descend` / `descend_mut` に分け、
  配列なら最後の要素へ降りるので `[a.b]` も `[[a.b]]` も直前の要素の中に入る。正規化は
  「自身の値 → ヘッダー形式」の 2 パスで、インラインテーブルはヘッダー形式で書けない位置
  （スカラーと混在した配列・配列の中の配列）だけで使う（ここを error にすると
  `a = [1, { b = 2 }]` が再エンコードできず fuzz の roundtrip が落ちる）。
  `UnsupportedFeature` と `ParseErrorKind::Unsupported` は中身が空になるので削除。
  レビュー指摘 3 件: dotted key で作ったテーブルは中間経路としては通れる
  （TOML 1.0 の `[fruit.apple.texture]` の例。v0.14 からの回帰で終端も中間も一律拒否して
  いた）/ インラインテーブルの深度はキー 1 段につき 1 / 空の `{}` `[]` で深度を消費しない
- **71 toml-test による検証と kabosu 0.2.0 公開** — 公式 toml-test の
  **valid 205 / invalid 474 を全通過**した。判断は 3 点とも推奨案:
  `scripts/vendor-toml-test.sh` で vendor（タグ＋アーカイブ sha256 固定。**タグはテスト
  スイートの版であって仕様の版ではない**ので、1.0 の選別は上流の `files-toml-1.0.0`）/
  期待値の tagged JSON は dev 依存の `serde_json` で読む / `Unsupported` は TOML 1.1 の
  案内へ転用。期待値との比較は**文字列ではなく値**で行う（float は `5e+22` と `5e22`、
  date-time は区切りやオフセットの表記が揺れる）。**toml-test が見つけたバグは 1 件** =
  コメントの中のタブ以外の制御文字を受理していた（`ControlCharInComment` を追加。
  単独の CR も不可で、CRLF の CR だけコメントの終わりとして扱う）。
  `Unsupported(TomlV11)` の対象は `\e` / `\xHH`・インラインテーブルの改行とコメントと
  末尾カンマ・秒を省略した時刻の 3 つで、**toml-test の「1.0 では invalid・1.1 では valid」
  で裏付けたものだけ**を入れる（引用符なしの非 ASCII キーは 1.1 でも許されないため、
  レビュー指摘で外した）。**fuzz が実バグを 1 件検出** = `[[a]]` はエンコーダ側で
  「配列＋要素テーブル」の 2 段なのに、パーサが現在セクションの深さを経路のセグメント数
  （1 段）で代用していて「パースできたのにエンコードできない」入力が作れた
  （`section_depth` で解決）

**教訓**: Phase 70〜71 で「パーサとエンコーダで深さの数え方がズレる」バグが 3 件出た
（インラインテーブル / 空のコンテナ / 配列ヘッダー）。うち 2 件はレビュー、1 件は fuzz が
見つけた。読み側と書き側で同じ量を別に数えている箇所は、必ず「出力できたのに読めない」
入力が作れる。

**非互換**は kabosu の API だけ（`UnsupportedFeature` と `EncodeErrorKind::TableInArray` の
削除、`Value` / `ValueKind` への variant 追加、`Datetime` 型の追加）。yuzu の利用者には
`yuzu.toml` に書ける構文が増えるだけで、既存の設定はそのまま読める。

</details>

<details>
<summary>完了済み: v0.15（Phase 64〜67）の内訳</summary>

軸は「**正しさ・堅牢性**」。v0.10.1 の外部コードレビューで「今回は入れない」と判断した
持ち越し 3 件（URL のパーセントエンコード全面対応 / `ServeDir` がシンボリックリンクを
辿る / `syntect.css` の無条件出力）は、どれも正しさの穴を規約や空ファイルで塞いでいる
状態のまま 5 版を越えていた。v0.14 で設定の正しさを整えたので、生成 URL・配信・検査の
正しさへ寄せ、候補に残していた外部リンク切れ検査も凍結方針（決定的・オフライン）と
衝突しない opt-in の形で入れた。各 Phase は着手時に判断点を決めてから実装し、
PR ごとの外部コードレビュー指摘（計 7 件）を同じ PR で取り込んだ。

- **64 URL のパーセントエンコード** — route → URL の変換点を `yuzu-core/src/urlpath.rs`
  の `encode_path` / `percent_decode` の対に 1 つ決め（呼ぶのは `UrlResolver::page_url` /
  `md_url` / `rewrite`・検索索引の `url`・`git.edit_url` の `{path}` の 4 箇所）、
  「ディスクは生・URL はエンコード」の境界で揃えた。非 ASCII もエンコードする（本文
  リンクは comrak の `escape_href` が既に `%XX` にしていて、ナビ・llms・sitemap・索引だけ
  生だった食い違いを解消。comrak は `%XX` を素通しするので二重にならない）。
  `'` `(` `)` `&` も属性・CommonMark のリンク先・実体参照を壊すのでエンコード側
  （llms.txt が `)` で壊れる潜在バグの修正）。著者がエンコード済みで書いた参照
  （`my%20page.md` / `/%E8%A8%AD…/` / aliases の `old%20name/`）はデコードして照合する =
  `yuzu fmt` が `<my page.md>` を `my%20page.md` へ正規化するため必須の判断で、alias
  `old%20name/` が 404 になる不具合も直った。`unsafe-page-path` は実ファイル名では `\` と
  制御文字だけに縮小し、設定・frontmatter 由来の route（合成ページ・aliases）は Windows の
  予約文字 `< > : " | ? *` を全 OS で拒否（レビュー指摘）。linkcheck の絶対・相対の分類は
  render と同じくデコード前の文字列で行う（レビュー指摘）。`CACHE_FORMAT_VERSION`
  21 → 22。ファイル名の `#` `?` へのリンクは `%23` `%3F` と書く以外にない（URL 構文上の
  制約）ことを docs に明記
- **65 配信とテーマ契約の堅牢化** — `preview` / `dev` / `build --watch` がシンボリック
  リンクを辿らない。tower-http 0.7 の `ServeDir` にはリンク追従を止めるオプションが無いので、
  公開されている `Backend` トレイトと `ServeDir::with_backend` で `TokioBackend` を包む
  `GuardedBackend` を yuzu-server に置き、`open` / `metadata` と 404 フォールバックの前に
  述語を通す。述語は `ServeOptions.path_guard` で cli が渡す（`WatchIgnore` と同型 =
  server → core の辺を作らない。中身は `yuzu_core::output::ensure_symlink_free` = 書き側と
  同じ検査で基点自身だけ許す読み側版。起点はプロジェクトルートで書き側と同じ =
  `output.dir = "alias/site"` の中間リンクも拒否。レビュー指摘）。判断: 遮断は 404 ＋
  warn ログ・既定 ON で設定キー無し・同期 lstat。`syntect.css` は
  `markdown.highlight.enabled` が有効なときだけ書き出し、`highlight_enabled` で
  `base.jinja` の `<link>` と同条件にした。テーマ上書きはデフォルトテーマ側の変更へ
  利用者が追随する契約を `guide/deploy.md` に明文化
- **66 外部リンク切れ検査（opt-in）** — `yuzu check --external-links` を付けたときだけ
  http / https の到達性を検査する。外部 URL 判定を `urlpath::is_external_url` /
  `is_http_url` の 1 実装へ寄せ、`check_links` は `LinkReport { diags, external }` で
  出現箇所を返す（core はネットワークに触れない）。判断: HTTP は **curl へ委譲**（cli の
  `commands/extlink.rs`。ureq + rustls は ring の C/asm と 20 超の crate を配布バイナリへ
  持ち込むので却下。8 並列・同一 URL は 1 回・curl が無ければ exit 2）/ 入口はフラグだけ /
  4xx（429 除く）だけ `external-link-broken`（warning・抑制可）で、DNS 失敗・タイムアウト・
  5xx・429 は診断にせず `summary.skipped`（加算キー）と集計行「スキップ N 件」へ =
  環境依存の失敗で CI を赤にしない。「ネットワーク I/O は既定経路に入れない」を凍結した
  設計判断に明文化。dogfooding で crates.io が `Accept: text/html` 無しでは 404 を返すと
  分かり Accept を付与。レビュー指摘 3 件: 未評価ルールの抑制を unused にしない
  （`LintOptions.unevaluated_rules`）/ curl のグロブ展開を `--globoff` で止める /
  判定できずスキップした URL への抑制も unused 判定を保留
  （`LintOptions.unevaluated_occurrences`）
- **67 dogfooding 改善** — docs の外部リンク検査を週次実行する `docs-links.yml` を新設
  （月曜 09:00 JST ＋ 手動。デプロイのゲートには入れない。初回手動実行は問題なし）/
  `preview` のリンク遮断を実バイナリ経由の e2e で固定。日本語ファイル名のページを docs に
  置く案は見送り（公開サイトに演出用ページを増やさない）、持ち越し候補は v0.16 以降へ。
  レビュー指摘: ci.yml の否定ゲート `! cmd` は `bash -e` でも止まらない（既存含む
  7 箇所を `if … exit 1` へ。CLAUDE.md の罠に追記）

</details>

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
