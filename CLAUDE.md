# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## プロジェクト概要

yuzu は Markdown の設計書を静的 HTML ドキュメントサイトに変換する Rust 製ツール（Cargo workspace、MSRV 1.85 / edition 2024）。対話・コメント・ドキュメント・テスト名はすべて日本語で書く。コミットはユーザの指示があるまで行わない（push もユーザが行う運用）。

プロジェクトスキル（`.claude/skills/`）: 検証一式は `verify`、実機確認は `run`、リリースは `release`、汎用ライブラリの crates.io 公開は `publish-crate`、Markdown 記法・本文レンダリング機能の追加は `add-markdown-feature`、テーマ JS / アセットの追加は `add-theme-asset`、tankan の図種追加は `tankan-add-diagram`、vendor 資産更新は `vendor-update`、開発コンテナ操作は `dev-container` を使う（apple container CLI 自体の汎用リファレンスはユーザスキル `apple-container`）。

## コマンド

```bash
cargo build --workspace
cargo test --workspace                        # insta スナップショットテストを含む
cargo test -p yuzu-core 正規化                # 単一 crate・テスト名でフィルタ（テスト名は日本語）
cargo test -p yuzu-core --test normalize_test # ファイル単位で絞るならこちら
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
```

CI 相当の検証一式（ci.yml と同じ順序・罠込み）は `verify` スキル。cargo test に含まれない追加の確認:

```bash
rustup target add wasm32-unknown-unknown
cargo check -p mikan-wasm --target wasm32-unknown-unknown
cargo check -p tankan --target wasm32-unknown-unknown

# e2e（CLI 実機確認）— cargo test は target/debug/yuzu を更新しないので必ず先にビルドする
cargo build -p yuzu-cli
./target/debug/yuzu new /tmp/e2e-docs && cd /tmp/e2e-docs
"$OLDPWD/target/debug/yuzu" build && "$OLDPWD/target/debug/yuzu" check
```

- **insta スナップショット**: 差分が出たら内容を必ず目視してから更新する。`cargo insta review` は cargo-insta が要る（**ホストに入っていないことがある**。開発コンテナには同梱）ので、無ければ `INSTA_UPDATE=always cargo test -p <crate>` で直接更新して `git diff` で確認する。CI は `INSTA_UPDATE=no` で未承認を失敗にする
- **vendor 更新スクリプト**: `scripts/build-search-wasm.sh`（wasm-bindgen-cli は workspace の `wasm-bindgen = "=x.y.z"` と完全同一バージョン必須）/ `scripts/vendor-mermaid.sh` / `scripts/vendor-katex.sh` / `scripts/vendor-vaporetto-model.sh`
- CLI の終了コード規約: 0 = 成功 / 1 = 違反あり（lint・check・fmt --check）/ 2 = 実行エラー

## アーキテクチャ

ワークスペース構成と依存方向は**凍結**（逆方向依存を作らない）:

```
yuzu-cli → {yuzu-server, yuzu-render, yuzu-index, yuzu-core, yuzu-config}
yuzu-render → yuzu-core, yuzu-config, yuzu-theme, tankan
yuzu-index → yuzu-core, mikan       mikan-wasm → mikan
yuzu-config → kabosu（通常依存はこれだけ）
（mikan は native/wasm 共通の本体。mikan-wasm はその wasm ラッパで、
トークナイザ・フォーマット・抜粋生成を 1 実装で共有する）
tankan・mikan・mikan-wasm・kabosu は他の yuzu crate 非依存の汎用ライブラリ
（tankan・mikan・kabosu は crates.io で公開済み。検索スタックの書き側集約は
mikan::build、読み側クエリエンジンは SearchEngine にあり、
yuzu-index はページ抽出とファイル I/O だけの薄い呼び出し側）
mikan = 旧 yuzu-index-format・mikan-wasm = 旧 yuzu-search-wasm（v0.7 後に改名）
```

- **yuzu-core**: comrak パース → Document/サイトモデル（nav・TOC・slug・sourcepos・lint・リンク検査）。パーサは内部に隠蔽し、公開 API は comrak 非依存。`markdown/mod.rs` が **comrak を触る唯一の場所**で、配下に `fence.rs`（フェンス情報文字列 = title / 行ハイライト / 行番号 / `file=` / `lines=`）・`crossref.rs`（キャプション行の採番と参照補完）・`collapse.rs`（`[!NOTE]-` → `<details>`）。ほかに `include.rs`（インクルードの読み込みと行切り出し。canonicalize でルート配下強制）・`aliases.rs`（エイリアスの正規化と検証）
- **yuzu-render**: サイトモデル → HTML（minijinja テンプレート、syntect ハイライト、Mermaid 変換、数式は comrak math 出力を同梱 KaTeX がクライアント描画、base_url 解決）
- **yuzu-config**: `yuzu.toml` を cwd から上方向に探索してプロジェクトルートを確定し、kabosu で読んで既定値をマージする。`schema.rs` が構造体と既定値、`codec.rs` が kabosu の Decode / Encode（`table_codec!` で「キー名 => フィールド」を 1 行ずつ。**キーを足したらここにも足す** = 忘れると未知キーとして設定エラーになる）、`resolve.rs` が読み込み・パス検証・日本語のエラー文言。未知キー・型不一致は位置付きの設定エラー（exit 2）、重複キーは構文エラー。通常依存は kabosu だけ（serde / jsonc-parser / thiserror / tracing は使わない。ログは cli の `commands::load_project` が出す）
- **yuzu-theme**: デフォルトテーマを rust-embed でバイナリ埋め込み。プロジェクトの `theme/` に同じ相対パスのファイルを置くとファイル単位で上書き
- **tankan**: Mermaid 互換 SSR（sequence / flowchart / class / state / ER / gantt / pie / mindmap / timeline / packet → SVG）。render_svg が Err を返すと yuzu 側が自動でクライアント描画にフォールバックするので未対応でも壊れない。ただし**図種を足すと「従来フォールバックしていたページが SSR 成功へ変わる」＝本文 HTML が変わる**ため、tankan 内（`kind.rs::is_supported` / `lib.rs` の mod ＋ match / corpus）だけでなく yuzu 側の `CACHE_FORMAT_VERSION` とスナップショットも追随が要る（`tankan-add-diagram` スキル参照）

### 凍結した設計判断（docs `development/index.md`「凍結した設計判断」参照。差し替えないこと）

comrak（Markdown）/ minijinja（テンプレート）/ syntect + two-face（ハイライト、CSS クラス出力）/ clap derive / TOML 設定は自作の kabosu（依存ゼロ。v0.14 で serde + JSONC から移行。JSONC の互換読み込みは作らない）/ rust-embed / axum + notify + WebSocket（dev サーバ）/ rayon（ページ並列化。出力はスレッド数に依らずバイト同一）。comrak・syntect・two-face は onig（C 依存）を引かないよう **必ず `default-features = false`**（Cargo.toml のコメント参照）。**ネットワーク I/O は build / check / dev の既定経路に入れない**（外部リンク検査は `yuzu check --external-links` の opt-in で、HTTP は curl へ委譲 = HTTP クライアント・TLS を依存に持ち込まない。`commands/extlink.rs`）。

### 検索の最重要制約

index 時（ネイティブ）と query 時（wasm）で**同一トークナイザコード（mikan）＋同一モデルバイト**を使うこと。抜粋生成・ハイライトのロジックも mikan に 1 実装で native/wasm 共有する（別実装を作らない）。`yuzu search` はブラウザと同じエンジンを通るので整合検証に使える。検索 UI の動作確認は `yuzu preview` / `yuzu dev` 経由（`file://` では fetch が動かない）。

### 1 実装で共有する箇所（片方だけ直さない）

同じ規則を 2 箇所で解釈すると必ずズレるため、意図的に 1 実装へ寄せてある。触るときは対になる側も確認する。

- **特別レンダリング言語**: yuzu-core の `is_special_render_lang` と yuzu-render `highlight.rs::render` のディスパッチは**集合を同期**させる（openapi / jsonschema は `SPEC_LANGS` が唯一の定義で、render 側 `SpecKind` との一致は speccheck のテストが縛る）
- **URL 分類**: `linkcheck.rs` の判定と yuzu-render `urls.rs` の `UrlResolver::rewrite` を揃える
- **アンカー採番**: extract_meta / 本文 HTML 化 / extract_plain_sections の **3 経路とも全見出しを文書順に** Anchorizer へ通す（片方で見出しを飛ばすと id がずれる）
- **フェンス情報文字列**は `markdown/fence.rs`（描画・検索・lint が共有。lint 用に `parse_fence_info_detailed`）、**外部ファイル参照**は `include.rs`（コンテンツインクルードの `file=` と openapi / jsonschema の `file:` が同居。ルート配下強制の同じ規律 `read_under_root` を共有し、本文の解決 `resolve_spec_source`（読み込み口はクロージャで受ける）と参照の検証 `validate_spec_refs` もここ。**仕様の中身の検証だけが yuzu-render**）、**エイリアス**は `aliases.rs`（render と check）
- `comrak_options_keep_footnotes` は fmt / normalize / linkcheck 専用。**HTML レンダと extract_meta に使うと壊れる**

### インクリメンタルビルドの層構造

`RenderCtx` / `IndexCtx` の**全フィールド None = 従来のフルビルドと同一動作**（ライブラリ単体テストはこの形。キャッシュ配線は cli 層の責務）。

- **yuzu-core**: `cache.rs`（ページ派生物キャッシュ。envKey / routesKey / sourceHash の 3 層無効化）＋ `output.rs`（compare-before-write・出力マニフェスト・孤児掃除）
- **yuzu-render**: `RenderCtx`（cache / outputs / shared）と `RenderShared`（watch 間で再利用する minijinja Env・syntect）
- **yuzu-index**: `IndexCtx` と `IndexSession`（vaporetto トークナイザの遅延構築・再利用）
- **yuzu-cli** `commands/build.rs`: `BuildSession` が上記を束ね、envKey 計算・routesKey 設定・マニフェスト保存を行う唯一の場所

キャッシュするのは高価なページ派生物（メタ・本文 HTML・検索 tf・llms 正規化 md）だけで、nav / fst / llms 連結などの集約は毎回全実行する（クロスページ依存を依存解析なしで正しく保つための分離。docs `development/internals-build.md` 参照）。

**クロスページ依存を持ち込むときは routesKey へ入れる**。例: `markdown.crossref.numbering: "site"` は先行ページの図表増減で後続ページの番号が変わるため、cli が routesKey にラベル個数を含めて本文キャッシュを無効化している。

### tankan の設計原則

I/O なし・時刻/乱数非依存（wasm32 担保のため。gantt の today 線は意図的に描かない）。日付演算は `common/date.rs`（依存なし）。corpus テストは `crates/tankan/tests/corpus/<図種>/*.mmd` 全件受理＋代表例の insta スナップショット。SVG のテーマ追従は `<style>`＋CSS 変数方式（SVG 属性内の var() は仕様上不可）。ユーザ指定色（flowchart / state / ER / class の classDef / class(cssClass) / `:::` / style）はインライン style 属性で直接埋める（テーマ非追従が正。`<style>` 追記方式は同一ページの複数 SVG でルールが衝突するため不可）。パース・マージ・解決・属性生成・fill 明度からの文字色自動選択は `common/style.rs`（`Style` / `StyleCollector` / `box_attr` / `line_attr` / `text_attr`）に 1 実装で集約し、各図種パーサは薄いアダプタで呼ぶ。

## リリース手順（vX.Y.Z）

手順と罠（マイナーとパッチで ROADMAP.md の書き方が違う・**タグは打った時点の main を切り出す**ので機能コミットをタグの後に積まない等）は **`release` スキル**に集約してある。汎用ライブラリ（tankan / mikan / kabosu）の crates.io 公開は yuzu のリリースと非同期で、手順は **`publish-crate` スキル**（kabosu は publish 前に fuzz 必須）。mikan-wasm と yuzu 本体の crate は `publish = false`（名前 `yuzu`・`yuzu-core` は別プロジェクトに取得済み。本体を公開する将来構想は ROADMAP.md）。

## 罠・注意点

- `cargo test --workspace` は `target/debug/yuzu` を**更新しない**。CLI の実機確認前に `cargo build -p yuzu-cli` を忘れない
- `yuzu build` / `dev` は常時インクリメンタル（`.yuzu/cache/`）。キャッシュ起因の不具合を疑うときは `--force`（または `.yuzu/cache/` 削除。いつでも安全）。**キャッシュ内容の意味が変わる変更**（本文 HTML の生成ロジック・検索 tf の重み等）では `yuzu-core/src/cache.rs` の `CACHE_FORMAT_VERSION` を上げる
- yuzu-server の serve テストは TCP バインドするため、サンドボックス内では PermissionDenied で落ちる（コード起因ではない）
- **comrak の AST を構造変更するときは「走査で集めて後段で適用」する**。`descendants()` のイテレート中に木を変えると *tree modified during iteration* でパニックする。段落 → `HtmlBlock` 化は**子を先に detach しないと** `InvalidChildType` でパニックする（HtmlBlock は子を持てない）。URL 書き換えのような**値の変更だけ**は走査中で安全
- **`dev` / `build --watch` はプロジェクトルート全体を監視する**（インクルード `file=` の参照先が content 外にもあるため）。**出力ディレクトリの除外は必須** = 外すと「再ビルド → 変更検知 → 再ビルド」の無限ループになる。隠しディレクトリと `build.watch_ignore` の glob も除外する（`yuzu-server/src/watch.rs` の `WatchIgnore`。glob 判定は yuzu-core の `IgnoreMatcher` を**述語で**渡す = server は yuzu-core を知らない）。**除外はイベントのフィルタで監視登録は減らない**（notify にパス単位の除外が無い）
- **watch 中の `yuzu.toml` 変更は取り込むが、監視・配信の前提になる設定は起動時固定**（`build.rs` の `WatchBuild` / `pin_restart_only`）。`output.dir` を差し替えると新しい出力先が監視除外から外れて無限ループになるため、`output.dir` / `base_url` / `dev.host` / `dev.port` / `dev.live_reload` / `build.watch_ignore` は警告だけ出して起動時の値を使う。**サーバや監視スレッドへ起動時に渡す設定を増やしたらこの関数にも足す**
- **検索 tf のキャッシュはページ source ハッシュ＋インクルード参照先の内容ハッシュで判定する**（`PageCacheEntry::search_deps_sha256`。参照先だけの編集で検索結果が古いまま残る不具合の修正）。**参照先ハッシュを `source_sha256` へ畳み込んではいけない** = `BuildCache::store` がエントリを丸ごと作り直し、meta / body / llms まで巻き添えで毎ビルド全ミスになる
- **検索インデックスのフォーマット追加は `serde(default)` で足し、`FORMAT_VERSION` を安易に上げない**。`manifest.json` は毎回フェッチされるのに `search_bg.wasm` は `_search/` の固定 URL で HTTP キャッシュに残るため、**再デプロイ直後の再訪問者が「新 manifest ＋ 旧 wasm」に入る**。bump するとそこで `VersionMismatch` になり検索が全停止する（据え置けば新機能が出ないだけの縮退で済む）。同じ理由で **wasm の既存メソッドのシグネチャは変えず新メソッドを足す** — 引数を足すと旧 wasm が黙って無視して「効いていないのに効いたつもりの結果」を返す
- **テーマ JS の「外側クリックで閉じる」判定に `ev.target.closest()` を使わない**。イベントリスナーの実行ごとにマイクロタスクが処理されるため、**ハンドラ内の再描画で押した要素が DOM から外れた後に document へバブリングする**ことがあり、外れた要素の `closest` は必ず null になって誤判定する。`ev.composedPath()` で判定する（絞り込みチップと「さらに N 件を表示」の両方が踏んだ）
- **`display` を指定する要素には `[hidden]` の指定を必ず添える**（`.foo { display: flex }` は UA の `[hidden] { display: none }` を上書きしてしまう）
- **合成ページ（`Page.generated: Option<GeneratedKind>` = 用語集・検索結果ページ）は「リンク先としてだけ」有効にする**。`build_site_model` と `build_source_pages` の両方に混ぜてあるので route 衝突検査・linkcheck のターゲット・routesKey は無料で効くが、**`page.src` は実在しない**。`fs::write(&page.src)` する fmt / lint --fix は必ず除外する（`fs::write` は新規作成するので、忘れると `yuzu fmt` が `content/glossary.md` を実体化する）。lint・整形差分・診断のリンク元・集計行のページ数も同様（機械的な除外は `is_generated()`）。**集約（nav・検索索引・sitemap・ページ単位 .md）に載せるかは種別で違う** — 用語集は載る・検索結果ページは載らない。判定は `Page::in_nav / in_search_index / in_sitemap / emits_page_md` に集約してあり、呼び出し側で kind を直接見ない（llms だけは合成時に `frontmatter.llms = false` を立てて既存フィルタに乗せる）。診断文面の設定キー名は `GeneratedKind::config_key()` が唯一の定義
- **本文中の `<abbr>` 化（用語集）は `render_body_html` の適用 A〜D がすべて終わった後**に回す。この順序ならキャプション段落とコードブロックは `HtmlBlock` へ差し替え済みで除外が無料になる一方、前段で集めると**後で子を detach される段落のノードへ `insert_before` することになり置換が静かに消える**。画像の `alt` を除外するのは整合性上の必須条件（comrak が alt を生 HTML 不可の文脈で描くため `alt="&lt;abbr …"` に化ける）
- **`base.jinja` の 2 つのインライン script は外部 JS 化しない**（head の FOUC 回避 / サイドバーのスクロール位置復元）。どちらも**最初のペイントより前**に走る必要があるため。それ以外のテーマ JS は従来どおり `static/js/` の外部ファイル
- rust-embed は debug ビルドだとテーマをファイルシステムから読む（テーマ編集が再コンパイル不要で反映される一方、debug バイナリ単体を別マシンへ持ち出すとアセットを見失う）。リリースビルドは常に埋め込み。**埋め込みフォルダへの新規ファイル追加は cargo の再コンパイル判定に載らない**ため、yuzu-theme は build.rs の `rerun-if-changed=assets` で監視している（これが無いと「debug では動くのに release が古い埋め込みを使い回して template not found」になる。埋め込み crate を増やすときは同じ build.rs を付けること）
- minijinja はデフォルトで属性中の `/` をエスケープするため、テンプレートの URL 値には**自前の `| url` フィルタ**（`yuzu-render/src/templates.rs`）を通す。`| safe` は `page.body`（レンダ済み HTML）と `theme_css_vars`（`css.rs` で検証済み）だけに残す。**URL 値へ `| safe` を使わない** = yuzu は slug 化をせずファイル名がそのまま route → URL になるため、引用符や `<` を含むファイル名で属性・`<script>` を抜けられる（`<script>` 内は実体参照がデコードされないので HTML エスケープでは直らない）
- **出力ディレクトリ（と `.yuzu`）への書き込みは `yuzu_core::output::write_under`、削除は `remove_dir_all_under` を必ず通す**（`root.join(rel)` ＋ `fs::write` を直に書かない）。両者が `resolve_output_rel`（rel の字句検証）と `ensure_no_symlink_under`（経路のリンク検査）を内包している。`Path::join` は絶対パス引数で左辺を捨て `.` はファイルシステムが吸収するため文字列比較では防げず、**リンク検査は基点自身と出力ツリーの内部まで見る**（`dist/guide -> /outside` があると書き込みも孤児掃除もリンク先へ抜ける。基点自身がリンクなら配下をいくら検査しても無意味）。逆に**基点の祖先は見ない** — macOS の `/tmp -> private/tmp` があり、`current_dir()` は解決済みパスを返すのでプロジェクトルート自身は常に実体。`output.dir` 自体の字句検証（ルート外・`input.dir` / `public` / `theme` / `.yuzu` との重なり）は `yuzu-config` の `load`（唯一の変換点）にある
- comrak 0.53 API: `render.r#unsafe`（unsafe_ ではない）/ `header_id_prefix`（header_ids は deprecated）/ `format_html` は fmt::Write（String）出力
- fmt/lint/check は **draft ページも対象**（build_source_pages）。build_site_model は従来どおり draft を除外する。nav を作らないので `crossref.numbering: "site"` でも診断時のラベル番号はページ内番号のまま
- `yuzu fmt` の不変条件: 本文は format_commonmark の正規形・**frontmatter は生テキストをバイト温存**・冪等・差分なしなら書き込まない（mtime 温存）
- **fmt の独自記法温存は fmt 経路限定**。format_commonmark は `{#fig:x}` を `{\#fig:x}` へ、`[!NOTE]-` を `[!NOTE] -` へ変えるので `restore_yuzu_syntax` が書いた形に戻すが、これを呼ぶのは `format_document` だけ。`normalize_markdown`（llms-full.txt）は復元しないため、**llms-full.txt にエスケープ形が出るのは仕様**（バグと誤認して直さない）
- `docs/` はこのリポジトリ自身のドキュメントサイト（yuzu プロジェクト。`docs/yuzu.toml` がルート）。main push で `.github/workflows/docs.yml` が GitHub Pages へデプロイし、ci.yml でも check・build・SSR フォールバック検出を検証する。原稿は `yuzu fmt` の正規形・表記は長音符なし（`lint.terms` 準拠）で書く
- **ci.yml の docs ゲートは docs の原稿と結合している**（新機能ごとに `grep` を 1 行足す運用）。特に `docs/yuzu.toml` の 25-45 行目（`[markdown]` ブロック）はインクルードの `lines=` で引用されているので、この範囲を動かすと原稿の中身と CI が同時に壊れる（`verify` スキルに直す箇所の一覧あり）
- **ci.yml の否定ゲートに `! cmd` を書かない**。GitHub Actions の `bash -e` でも `!` で反転したコマンドは errexit の対象外で、失敗しても次の行へ進む（= ゲートになっていない）。`if cmd; then echo "…" >&2; exit 1; fi` の形で明示的に落とす（PR #7 のレビュー指摘で既存 7 箇所を置換済み）
- **`yuzu-index` は rust-embed（`assets/search/`）を使うのに build.rs が無い**。既存ファイルの更新は追跡されるので vendor スクリプトの通常運用では問題ないが、**新規ファイルを足すと release が古い埋め込みを使う**恐れがある（上記 yuzu-theme と同じ罠。足すときは build.rs を付ける）
- `docs/design/` は git 管理外のローカル設計ノート。公開物（コード・README・コミット）から参照しない
- 開発コンテナ内（`.devcontainer/`）は `CARGO_TARGET_DIR=/cargo-target` のため、CLI 実機確認は `"$CARGO_TARGET_DIR/debug/yuzu"` を使う（`./target/debug/yuzu` は**存在しない**）。環境定義は `.devcontainer/Dockerfile` が唯一で、devcontainer.json とラッパーの不変条件は `.devcontainer/README.md` の表を参照
