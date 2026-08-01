# ロードマップ

yuzu の開発計画と、これまでのリリースの内訳。**このファイルが Phase 状態の正**
（README には現在の版と概要だけを置く）。

## 現在: v0.11（Phase 50〜53）

軸は「**執筆機能の拡充 第 3 弾**」。v0.10 で「黙って効かない・検証されない」穴を
塞いだので、その上に書ける表現と文言の再利用を積む。判断軸は従来どおり
「素の Markdown ビューアで壊れない」「クライアント JS ゼロを崩さない」。
Phase は価値と実装コスト・依存関係の順（着手時に個別に設計する）。

### 50 タブ / コードグループ ✅

言語別サンプル（Rust / TypeScript）や OS 別手順を**切り替えて**見せたいのに、
今は同じ内容を縦に並べるしかない。

**記法は (a) フェンスに `tab="Rust"` を採用する**（連続フェンスを 1 グループへ束ねる）。
comrak 0.53 で両案を実測したうえでの判断:

- **(a) の前提は実測どおり** — 情報文字列は `format_commonmark` を通しても
  バイト単位で一致し冪等。`yuzu fmt` の追加作業は本当にゼロ
- **(b) の欠点として書いていた「`format_commonmark` が先頭 `:` をエスケープする」は
  誤りだった** — エスケープされず、`block_directive` 有効時の fmt も冪等。
  ただし代わりに次が判明した:
  - **同じ長さのフェンスがネストできない**。`:::tabs` の中の `:::tab Rust` は
    誤パースして余分な `<div class="">` を吐く（外側を `::::` にする必要があり、
    lint で強制しないと黙って壊れる）
  - **info が丸ごと class に入る**（`:::tab Rust` → `class="tab Rust"`）。
    ラベルとして使うには結局 AST 介入が要り「自前パーサ不要」は半分だけ正しい
  - 素のビューアで `::::tabs` / `:::tab` が文字列として見える（判断軸に反する）
- **決め手**: この Phase の用途（言語別サンプル・OS 別手順）は**どちらもコードブロック**
  なので、(b) の唯一の優位点である「コード以外もタブにできる」が効かない。
  散文のタブが必要になったら `block_directive` を後から別記法として足せる（排他ではない）

- CSS は radio + flex `order` で**タブ枚数の上限なく JS ゼロ**（ラジオ名はページ内で一意に）
- 検索・llms.txt は**現状と変わらない**（タブの中身はコードブロックのままなので
  `search.indexCode` 既定 false のもとで従来どおり。llms は縦に並んだ原文）
- タブ見出しは既存の `code-block-meta` lint に相乗りする（新ルールを作らない）
- `CACHE_FORMAT_VERSION` の bump が要る

### 51 Markdown 片のインクルード ✅

`file=` は**コードブロック専用**で、共通の注意書き・用語定義・免責文を複数ページで
再利用できない（設計書運用では「同じ文言が 5 ページに散り、片方だけ古い」が起きる）。
` ```include file="snippets/note.md" ` が Markdown 断片を本文の AST へ展開する。

論点 7 つへの回答（着手時の設計判断）:

- **記法**: フェンス（言語トークン `include` ＋ `file=` / `lines=` 再利用）。
  素のビューアでは空のコードブロックに見えるだけで壊れず、`yuzu fmt` は情報
  文字列を逐語温存する契約（Phase 39）なので追加作業ゼロ
- **断片は散文専用**（見出し・図表キャプション行・脚注・frontmatter を check が
  `include-error` でエラーに）。これにより extract_meta は無展開のままでよく、
  **アンカー採番の 3 経路同期・meta キャッシュ無効化の問題がそもそも発生しない**。
  展開が要るのは本文 HTML と検索の 2 経路だけ
- **入れ子は禁止**（断片内の `file=` 付きフェンス全般を禁止。検索の deps ハッシュが
  入れ子の参照先を追えず、Phase 48 修正前の「参照先を編集しても検索が古い」を
  再導入するため）。循環検出・深さ上限は不要になった
- **キャッシュ**: 本文 HTML は `RenderedBody.used_fragment` で非対象化
  （core 展開は renderer の external_deps を通らないため、戻り値で報告する。
  v15 事故の再演防止）。検索 tf は `searchDepsSha256` を per-spec ゲート化して流用
  （断片は indexCode と無関係に常に索引）
- **fmt は断片自体を対象にしない**（断片は content 外の `snippets/` に置く慣例。
  content 内に置くとページになるので避けるか input.ignore で除外）
- **llms.txt は原文のまま**（Phase 42 と同じ非対称。fmt 正規形との一致を保つ）
- **断片内の見出しレベル飛び lint は「見出し自体を禁止」で解消**

制限（docs に明記済み）: 断片内リンクは linkcheck 対象外・相対リンクは
取り込み先ページ基準で解決・断片内の tab= は無効。

### 52 用語集・略語 ✅

設計書は略語が多いのに、初出の説明を毎ページ書くか読み手の記憶に頼るしかなかった。
`lint.terms`（表記ゆれ辞書）と同じ「**設定に辞書を置く**」形（`markdown.glossary.terms`）に
したので、本文の Markdown は 1 バイトも汚れない（Markdown Extra の `*[API]: …` 記法は
素のビューアで定義行が見えてしまい、comrak に該当拡張も無い）。

論点 5 つへの回答（着手時の設計判断）:

- **除外**: 見出し・リンク文字列・画像 `alt`・図表キャプション・コード・数式。
  **画像 alt は整合性上の必須条件** — comrak は alt を「生 HTML を通せない文脈」で
  描くため、`HtmlInline` を入れると `alt="&lt;abbr …"` とエスケープされて壊れる。
  一方コード・数式・インラインコードは**そもそも `Text` ノードにならない**ので無料で外れる
- **初出だけ**（ページ内）。「初出で説明、あとは素のテキスト」という設計書の慣例に合わせた。
  見出しとリンクを除外しているので、見出しで初出を使い切って本文の説明が消えることはない。
  ページ単位で閉じる＝クロスページ依存が無いので **routesKey への追加は不要**
- **AST の適用順序**: 走査で集めて後段で適用（Phase 43・44 の罠）に加えて、
  **既存の適用 A〜D がすべて終わった後**（`format_html` の直前）に単独で回す。
  この順序ならキャプション段落とコードブロックは既に `HtmlBlock` へ差し替わっており、
  `parse_caption` を再実装せずに除外が成立する。逆に前段で集めると、後で子を `detach`
  される段落のノードへ `insert_before` することになり**置換が静かに消える**
- **検索**: 用語集ページが通常ページとして索引されるので「説明文で検索して用語集へ」が
  成立する（`/glossary/#ssr` へディープリンクする）。本文の `title` 属性は索引しない。
  `lint.terms` → `search.synonyms` の合成経路には手を出していない（説明文は複数語の
  フレーズで、単語単位の同義語グループとは形が合わない）
- **用語集ページは合成 `Page` を `pages` へ混ぜる**（404.html のような単発描画にしない）。
  nav・パンくず・pager・sitemap・llms.txt・検索・route 衝突検査・routesKey・
  出力マニフェスト（＝設定を消すと孤児掃除される）がすべて既存経路のまま効く。
  `Page.generated` を足して fmt / lint / `edit_url` から外し、**リンク検査では
  リンク先としてだけ**有効にする（`[用語集](../glossary.md#ssr)` が解決できる）。
  新しいルール ID は作らず、`route-conflict` / `unsafe-page-path` / `alias-conflict` の
  メッセージを分岐させて「直す場所は `markdown.glossary.page`」と案内する。
  `CACHE_FORMAT_VERSION` 17 → 18

単語境界は**用語の端が ASCII 英数字のときだけ**要求する（`API` は `RAPID` / `APIs` に
一致せず、日本語の用語は文中のどこでも一致する）。重なりは最長一致優先。

### 53 dogfooding 改善 ⬜

恒例のバッファ枠（着手時にユーザが 3 点選ぶ）。候補:

- **`cjk_friendly_emphasis` の有効化** — comrak 0.53 のフラグ 1 つ。現状
  `**「…」**が出ます` のように日本語の括弧・句読点に隣接する強調が効かない
  （v0.10 の docs 執筆中に実際に踏んだ）。既存の本文 HTML が変わるのでスナップショット
  確認と `CACHE_FORMAT_VERSION` bump が要る
- **印刷用 CSS** — `@media print` が 0 件で、PDF 保存するとサイドバー・TOC・
  検索ボックス・コピーボタンが全部紙に載る
- **定義リスト** — comrak `description_lists` が未有効化。用語集と相性が良い
- `--root` グローバルオプション / shell 補完（clap_complete）/
  ポート衝突時のエラーにポート番号を出す / 検索結果の絞り込み
- 下の「v0.10.1 レビューの持ち越し」の小さいもの（キャッシュ原子性・syntect.css）

### v0.10.1 レビューの持ち越し

v0.10.1（外部コードレビュー対応）で「今回は入れない」と判断したもの。
**判断の根拠ごと残す**（同じ検討を繰り返さないため）。

- **URL のパーセントエンコード全面対応** — 現状は `#` `?` `%` 等を含むファイル名を
  `unsafe-page-path` で**拒否**して整合を取っている。本来は route → URL の変換で
  パスセグメントをエンコードするのが筋だが、`urls.rs` の `page_url` / `md_url` /
  `asset_url` / `public_url` と `UrlRewriter::rewrite` の全経路に加えて、検索
  インデックス・llms.txt・sitemap・`linkcheck` の整合まで取り直しになるので
  **Phase 相当の規模**。テンプレート段階では解決できない（パスの一部の `#` と
  URL 構文の `#` を区別できず、`| url` フィルタに足すと `page.edit_url` の
  クエリを壊す）
- **キャッシュ保存の原子性** — `cache.rs` の save は「ページ → global.json」の順で
  書くため global.json が事実上のコミットレコードになり、危険な向き（新メタデータ
  ＋旧ページキャッシュ）は**構造上発生しない**。中断時は envKey 不一致で全捨て＝
  フルビルドへ縮退するので、実害は計算のやり直しだけ。直すなら global.json だけ
  tmp ＋ rename が安価
- **`syntect.css` の無条件出力** — `markdown.highlight.enabled: false` でも
  `base.jinja` が無条件に `<link>` するため、空の CSS を書き出して 404 を避けている。
  テンプレート側を条件分岐にするとテーマを上書きしている利用者と非互換になるので、
  やるならテーマ契約の変更として設計する
- **`.devcontainer/post-create.sh` の Claude Code インストーラ** — 取得した
  `install.sh` を検証せず bash へ渡している。ただし**インストーラ自身が
  ダウンロードしたバイナリを SHA-256 検証している**（バージョンごとの
  `manifest.json` の値と照合し、不一致なら削除して終了）ので、残るギャップは
  スクリプトの TOFU のみ。`install.sh` に公開チェックサムが無く、ベンダ更新のたびに
  devcontainer のビルドが壊れるため固定は見送った。バージョン指定
  （`bash -s -- <version>`）は可能なので、必要になったらそこから
- **`ServeDir` が dist 内のリンクを辿る** — `preview` / `dev` の配信は既定で
  シンボリックリンクを追う。書き込み側（`output::write_under`）を塞いだので
  混入経路は無いはずだが、手で置かれた場合は配信される

## v0.12 以降の候補

- **i18n** — テーマ UI 文字列の多言語化。`site.lang` は現在 `<html lang>` にしか
  効いていない（コア UI で 36 文字列・apispec を含めると +32）
- **全文検索の結果専用ページ** — URL で共有できる検索結果
  （Phase 49 でドロップダウンの追加読み込みは解決済み）
- **lint の制御性** — inline 抑制と全ルールの enable/disable
- **外部リンク切れ検査** — 「決定的・オフライン」の凍結方針と衝突するため opt-in 設計が要る
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
  [ドキュメントサイト](https://ai-implementer.github.io/yuzu/)を GitHub Pages へ公開 /
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

検索エンジン本体 **mikan**（旧 yuzu-index-format）と wasm ラッパ **mikan-wasm**
（旧 yuzu-search-wasm）は v0.7 リリース後に yuzu- プレフィックスを外して改名し、
mikan は crates.io で単独公開している（tankan と同じく独立バージョン）。

各版の Phase 内訳:

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
  yuzu 自身のドキュメントを yuzu で書いて GitHub Pages に公開（https://ai-implementer.github.io/yuzu/ ）。
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
