# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## プロジェクト概要

yuzu は Markdown の設計書を静的 HTML ドキュメントサイトに変換する Rust 製ツール
（Cargo workspace、MSRV 1.87 / edition 2024。公開ライブラリの kabosu・tankan は 1.85）。

- **日本語で書く** — 対話・コメント・ドキュメント・テスト名すべて
- **コミットはユーザの指示があるまで行わない**（push もユーザが行う運用）

プロジェクトスキル（`.claude/skills/`）:

| 目的 | スキル |
| --- | --- |
| 検証一式（CI 相当 ＋ e2e） | `verify` |
| 実機確認（ブラウザ配信） | `run` |
| リリース | `release` |
| 汎用ライブラリの crates.io 公開 | `publish-crate` |
| Markdown 記法・本文レンダリング機能の追加 | `add-markdown-feature` |
| テーマ JS / アセットの追加 | `add-theme-asset` |
| tankan の図種追加 | `tankan-add-diagram` |
| vendor 資産更新 | `vendor-update` |
| 依存の更新と監視（dependabot の PR・勧告・cargo-deny の失敗） | `dependency-update` |
| 開発コンテナ操作 | `dev-container` |

apple container CLI 自体の汎用リファレンスはユーザスキル `apple-container`。

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

- **CLI の終了コード規約**: 0 = 成功 / 1 = 違反あり（lint・check・fmt --check）/ 2 = 実行エラー
- **CLI の実行文脈は `cx.rs` の `Cx` に集約する**（サブコマンドをまたいで効く引数の器。
  `--root` の `canonicalize` は `Cx::new` が唯一の受け口 = 「`Cx` を持っている＝
  正規化済み絶対パス」が不変条件）。グローバル引数を足すときは
  `cli.rs` の `GlobalArgs` へ（`global = true` が必須）
  - **グローバル引数同士の排他は `conflicts_with` だけでは足りない** — clap は
    トップレベルとサブコマンドを別々に検証するので `yuzu -q build -v` が通る。
    `Cli::parse_validated` のパース後検証に足す（`main` とテストはこれを通す）
- **insta スナップショット**: 差分が出たら内容を必ず目視してから更新する
  - `cargo insta review` は cargo-insta が要る（**ホストに入っていないことがある**。
    開発コンテナには同梱）
  - 無ければ `INSTA_UPDATE=always cargo test -p <crate>` で直接更新して `git diff` で確認
  - CI は `INSTA_UPDATE=no` で未承認を失敗にする
- **vendor 更新スクリプト**:
  - `scripts/build-search-wasm.sh` — wasm-bindgen-cli は workspace の
    `wasm-bindgen = "=x.y.z"` と完全同一バージョン必須
  - `scripts/vendor-mermaid.sh` / `scripts/vendor-katex.sh` / `scripts/vendor-vaporetto-model.sh`
  - `scripts/vendor-toml-test.sh` — kabosu のテスト。**タグはスイートの版で仕様の版ではない**
    = 1.0 の選別は上流の `files-toml-1.0.0`
  - **vendor 資産を更新したら第三者ライセンスの通知も作り直される**（`scripts/third-party-licenses.sh`。
    Phase 81）。dist 用の 2 枚（`_assets/vendor/`・`_search/` の `THIRD-PARTY-LICENSES.txt`）は
    バイナリに埋め込むのでコミットし、アーカイブ用は release.yml の `licenses` ジョブが毎回作る
    （コミットしない）。cargo-about は版を固定（`--features cli` が要る）、mermaid の
    束ねたパッケージの取得に jq が要る。設定と two-face の一覧は `licenses/`（`README.md` 参照）
  - **cargo-about は本文の無い・認識できない crate で SPDX の雛形に戻り、`--fail` でも止まらない**
    （著作権表示が `<year> <copyright holders>` のまま出る）。スクリプトは雛形が残ると失敗するので、
    ファイルがあれば `licenses/about.toml` の clarify、無ければ `licenses/supplements.tsv` で補う。
    npm 側は LICENSE が無ければ README の License 節から取り、無ければ止まる
  - **使ってよいライセンスの一覧は `licenses/about.toml` と `deny.toml`（cargo-deny）の 2 か所**。
    片方だけ足すと生成か CI の片方だけが通る（`third-party-licenses.sh check` が一致を照合）
  - **検索 wasm に入る crate の版が Cargo.lock で変わったら wasm を作り直す**（コミット済みの
    wasm が古い版のまま残り、vaporetto ならトークナイザがずれる）。`third-party-licenses.sh check` が
    `_search/THIRD-PARTY-LICENSES.txt` の crate と版の集合を、mikan-wasm の wasm32 向け依存
    （`cargo tree`）と照合して知らせる（Phase 82）。**Cargo.lock に版があるかだけで照合しない** =
    同じ crate の旧版がネイティブ側に残ると見逃す（PR #32 のレビュー指摘）
  - vendor 資産の勧告は `scripts/vendor-advisories.sh`（deps.yml が週次）。版は
    `crates/yuzu-theme/licenses/` の記録から読み、除外は「GHSA の ID とパッケージ@版」の組

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

- **yuzu-core** — comrak パース → Document / サイトモデル（nav・TOC・slug・sourcepos・
  lint・リンク検査）。パーサは内部に隠蔽し、公開 API は comrak 非依存
  - `markdown/mod.rs` が **comrak を触る唯一の場所**
  - 配下: `fence.rs`（フェンス情報文字列 = title / 行ハイライト / 行番号 / `file=` /
    `lines=`）・`crossref.rs`（キャプション行の採番と参照補完）・
    `collapse.rs`（`[!NOTE]-` → `<details>`）
  - ほかに `include.rs`（インクルードの読み込みと行切り出し。canonicalize でルート配下強制）・
    `aliases.rs`（エイリアスの正規化と検証）
- **yuzu-render** — サイトモデル → HTML（minijinja テンプレート、syntect ハイライト、
  Mermaid 変換、base_url 解決）。数式は comrak math 出力を同梱 KaTeX がクライアント描画
- **yuzu-config** — `yuzu.toml` を cwd から上方向に探索してプロジェクトルートを確定し、
  kabosu で読んで既定値をマージする
  - `schema.rs` が構造体と既定値
  - `codec.rs` が kabosu の Decode / Encode（`table_codec!` で「キー名 => フィールド」を
    1 行ずつ。**キーを足したらここにも足す** = 忘れると未知キーとして設定エラーになる）
  - `resolve.rs` が読み込み・パス検証・日本語のエラー文言
  - 未知キー・型不一致は位置付きの設定エラー（exit 2）、重複キーは構文エラー
  - 通常依存は kabosu だけ（serde / jsonc-parser / thiserror / tracing は使わない。
    ログは cli の `commands::load_project` が出す）
  - **`--root` 指定時は上方向探索をしない**（探索すると「指定したのに親の
    `yuzu.toml` を拾う」事故になる）。指定先に無いときの `ConfigFileNotFound` は
    `find_project_root` 専用の `ProjectRootNotFound` と別物で、**文言に「上方向に探索」を
    含めない**（探索していないため）
- **yuzu-theme** — デフォルトテーマを rust-embed でバイナリ埋め込み。プロジェクトの
  `theme/` に同じ相対パスのファイルを置くとファイル単位で上書き
- **tankan** — Mermaid 互換 SSR（sequence / flowchart / class / state / ER / gantt / pie /
  mindmap / timeline / packet → SVG）
  - render_svg が Err を返すと yuzu 側が自動でクライアント描画にフォールバックするので
    未対応でも壊れない
  - ただし**図種を足すと「従来フォールバックしていたページが SSR 成功へ変わる」＝
    本文 HTML が変わる**
  - 追随先は tankan 内（`kind.rs::is_supported` / `lib.rs` の mod ＋ match / corpus）
    だけでなく yuzu 側の `CACHE_FORMAT_VERSION` とスナップショットも
    （`tankan-add-diagram` スキル参照）

### 凍結した設計判断（docs `development/index.md`「凍結した設計判断」参照。差し替えないこと）

- comrak（Markdown）/ minijinja（テンプレート）/ syntect + two-face（ハイライト、
  CSS クラス出力）/ clap derive ＋ clap_complete（補完は実行時生成のみ・`unstable-dynamic` は
  使わない）/ rust-embed / axum + notify + WebSocket（dev サーバ）
- TOML 設定は自作の kabosu（依存ゼロ。v0.14 で serde + JSONC から移行。
  JSONC の互換読み込みは作らない）
- rayon（ページ並列化。出力はスレッド数に依らずバイト同一）
- comrak・syntect・two-face は onig（C 依存）を引かないよう **必ず
  `default-features = false`**（Cargo.toml のコメント参照）。`deny.toml` の bans が onig・
  TLS・HTTP クライアントの混入を止める。syntect は `default-fancy` も使わず feature を並べる
  （plist-load・yaml-load が勧告付きの依存を引く。Phase 82）
- **ネットワーク I/O は build / check / dev の既定経路に入れない** — 外部リンク検査は
  `yuzu check --external-links` の opt-in で、HTTP は curl へ委譲 = HTTP クライアント・
  TLS を依存に持ち込まない（`commands/extlink.rs`）

### 検索の最重要制約

- index 時（ネイティブ）と query 時（wasm）で**同一トークナイザコード（mikan）＋
  同一モデルバイト**を使う
- 抜粋生成・ハイライトのロジックも mikan に 1 実装で native/wasm 共有する（別実装を作らない）
- `yuzu search` はブラウザと同じエンジンを通るので整合検証に使える
- 検索 UI の動作確認は `yuzu preview` / `yuzu dev` 経由（`file://` では fetch が動かない）

### 1 実装で共有する箇所（片方だけ直さない）

同じ規則を 2 箇所で解釈すると必ずズレるため、意図的に 1 実装へ寄せてある。触るときは対になる側も確認する。

- **特別レンダリング言語** — yuzu-core の `is_special_render_lang` と yuzu-render
  `highlight.rs::render` のディスパッチは**集合を同期**させる
  - openapi / jsonschema は `SPEC_LANGS` が唯一の定義で、render 側 `SpecKind` との一致は
    speccheck のテストが縛る
- **URL 分類** — `linkcheck.rs` の判定と yuzu-render `urls.rs` の `UrlResolver::rewrite` を揃える
- **アンカー採番** — extract_meta / 本文 HTML 化 / extract_plain_sections の
  **3 経路とも全見出しを文書順に** Anchorizer へ通す（片方で見出しを飛ばすと id がずれる）
  - 本文 HTML の見出しは `markdown/heading.rs` の HeadingAdapter が描く（id は見出し自身・
    パーマリンクは末尾に aria-label 付き。comrak の header_ids の既定出力は使わない）。
    採番の入力は comrak と同じ `HeadingMeta::content`。見出しの描画を変えるときは
    ここだけを直し、HTML 文字列の後処理で `class="anchor"` を書き換えない
- **フェンス情報文字列** — `markdown/fence.rs`（描画・検索・lint が共有。
  lint 用に `parse_fence_info_detailed`）
- **外部ファイル参照** — `include.rs`
  - コンテンツインクルードの `file=` と openapi / jsonschema の `file:` が同居し、
    ルート配下強制の同じ規律 `read_under_root` を共有する
  - 本文の解決 `resolve_spec_source`（読み込み口はクロージャで受ける）と参照の検証
    `validate_spec_refs` もここ
  - **仕様の中身の検証だけが yuzu-render**
- **エイリアス** — `aliases.rs`（render と check）
- `comrak_options_keep_footnotes` は fmt / normalize / linkcheck 専用。
  **HTML レンダと extract_meta に使うと壊れる**

### インクリメンタルビルドの層構造

`RenderCtx` / `IndexCtx` の**全フィールド None = 従来のフルビルドと同一動作**（ライブラリ単体テストはこの形。キャッシュ配線は cli 層の責務）。

- **yuzu-core** — `cache.rs`（ページ派生物キャッシュ。envKey / routesKey / sourceHash の
  3 層無効化）＋ `output.rs`（compare-before-write・出力マニフェスト・孤児掃除）
- **yuzu-render** — `RenderCtx`（cache / outputs / shared）と `RenderShared`
  （watch 間で再利用する minijinja Env・syntect）
- **yuzu-index** — `IndexCtx` と `IndexSession`（vaporetto トークナイザの遅延構築・再利用）
- **yuzu-cli** `commands/build.rs` — `BuildSession` が上記を束ね、envKey 計算・
  routesKey 設定・マニフェスト保存を行う唯一の場所

- キャッシュするのは高価なページ派生物（メタ・本文 HTML・検索 tf・llms 正規化 md）だけ。
  nav / fst / llms 連結などの集約は毎回全実行する（クロスページ依存を依存解析なしで
  正しく保つための分離。docs `development/internals-build.md` 参照）
- **クロスページ依存を持ち込むときは routesKey へ入れる** — 組み立ては core の
  `SiteModel::routes_key` が唯一の定義（cli とテストが同じ関数を通る）。例:
  `markdown.crossref.numbering: "site"` は先行ページの図表の増減・並び順で後続ページの
  番号が変わるため、各ページの採番開始位置（`crossref_offset`）を含めている
  （v0.18.1 まではラベル個数だけで、`order` の変更や図⇄表の差し替えを取りこぼした）

### tankan の設計原則

- **I/O なし・時刻 / 乱数非依存**（wasm32 担保のため。gantt の today 線は意図的に描かない）。
  日付演算は `common/date.rs`（依存なし）
- corpus テストは `crates/tankan/tests/corpus/<図種>/*.mmd` 全件受理 ＋
  代表例の insta スナップショット
- **SVG のテーマ追従は `<style>` ＋ CSS 変数方式**（SVG 属性内の var() は仕様上不可）
- **ユーザ指定色はインライン style 属性で直接埋める**（flowchart / state / ER / class の
  classDef / class(cssClass) / `:::` / style）
  - テーマ非追従が正。`<style>` 追記方式は同一ページの複数 SVG でルールが衝突するため不可
- パース・マージ・解決・属性生成・fill 明度からの文字色自動選択は `common/style.rs`
  （`Style` / `StyleCollector` / `box_attr` / `line_attr` / `text_attr`）に 1 実装で集約し、
  各図種パーサは薄いアダプタで呼ぶ

## リリース手順（vX.Y.Z）

- **yuzu 本体** — 手順と罠は **`release` スキル**に集約（マイナーとパッチで ROADMAP.md の
  書き方が違う・**タグは打った時点の main を切り出す**ので機能コミットをタグの後に
  積まない等）
- **汎用ライブラリ（tankan / mikan / kabosu）** — yuzu のリリースと非同期。手順は
  **`publish-crate` スキル**（kabosu は publish 前に fuzz 必須）
- **公開しない crate** — mikan-wasm と yuzu 本体の crate は `publish = false`
  （名前 `yuzu`・`yuzu-core` は別プロジェクトに取得済み。本体を公開する将来構想は ROADMAP.md）

## 罠・注意点

### ビルドとキャッシュ

- `cargo test --workspace` は `target/debug/yuzu` を**更新しない**。CLI の実機確認前に
  `cargo build -p yuzu-cli` を忘れない
- `yuzu build` / `dev` は常時インクリメンタル（`.yuzu/cache/`）
  - キャッシュ起因の不具合を疑うときは `--force`（または `.yuzu/cache/` 削除。いつでも安全）
  - **キャッシュ内容の意味が変わる変更**（本文 HTML の生成ロジック・検索 tf の重み等）では
    `yuzu-core/src/cache.rs` の `CACHE_FORMAT_VERSION` を上げる
- **検索 tf のキャッシュはページ source ハッシュ ＋ インクルード参照先の内容ハッシュで
  判定する**（`PageCacheEntry::search_deps_sha256`。参照先だけの編集で検索結果が古いまま
  残る不具合の修正）
  - **参照先ハッシュを `source_sha256` へ畳み込んではいけない** = `BuildCache::store` が
    エントリを丸ごと作り直し、meta / body / llms まで巻き添えで毎ビルド全ミスになる
- **ログのサブスクライバは `log_internal_errors(false)` 必須**（`main.rs`）。
  `tracing_subscriber::fmt()` の既定は true で、stderr への書き込み失敗時に同じ stderr へ
  `eprintln!` して panic する = `yuzu build 2>&1 | head` のように読み手が先に閉じると
  ビルドが途中で落ち、`--force` なら dist を作り直した後なので `_search` が消えたままになる
  （`-q` / `-v` は `GlobalArgs` → `log_filter` で解決し、`RUST_LOG` より優先する）

### comrak

- **AST を構造変更するときは「走査で集めて後段で適用」する**
  - `descendants()` のイテレート中に木を変えると *tree modified during iteration* でパニック
  - 段落 → `HtmlBlock` 化は**子を先に detach しないと** `InvalidChildType` でパニック
    （HtmlBlock は子を持てない）
  - URL 書き換えのような**値の変更だけ**は走査中で安全
- **本文中の `<abbr>` 化（用語集）は `render_body_html` の適用 A〜D がすべて終わった後**に回す
  - この順序ならキャプション段落とコードブロックは `HtmlBlock` へ差し替え済みで除外が無料
  - 前段で集めると**後で子を detach される段落のノードへ `insert_before` することになり
    置換が静かに消える**
  - 画像の `alt` を除外するのは整合性上の必須条件（comrak が alt を生 HTML 不可の文脈で
    描くため `alt="&lt;abbr …"` に化ける）
- comrak 0.53 API: `render.r#unsafe`（unsafe_ ではない）/ `header_id_prefix`
  （header_ids は deprecated）/ `format_html` は fmt::Write（String）出力

### watch と dev サーバ

- **`dev` / `build --watch` はプロジェクトルート全体を監視する**（インクルード `file=` の
  参照先が content 外にもあるため）
  - **出力ディレクトリの除外は必須** = 外すと「再ビルド → 変更検知 → 再ビルド」の
    無限ループになる
  - 隠しディレクトリと `build.watch_ignore` の glob も除外する（`yuzu-server/src/watch.rs` の
    `WatchIgnore`。glob 判定は yuzu-core の `IgnoreMatcher` を**述語で**渡す =
    server は yuzu-core を知らない）
  - **除外はイベントのフィルタで監視登録は減らない**（notify にパス単位の除外が無い）
  - **イベントは種類でも絞る** — Linux の inotify は開いた・読んだだけのイベントも届け、
    絞らないとビルドが原稿を読むたびに再ビルドして止まらない（macOS では起きないので
    気付きにくい）。種類を捨てる debouncer（notify-debouncer-mini 等）を戻さない
  - **監視スレッドの panic は main の `catch_unwind` に届かない** — `watch` が受けて
    `WatchFailure` で `serve` に知らせ、配信ごと止めて exit 2 にする
- **panic hook の中で終了させない**（`main.rs` の `install_panic_hook`）。hook は
  `catch_unwind` で回収する panic でも先に呼ばれるので、exit すると Mermaid 描画・
  comrak 整形の回収（`yuzu_core::recover`）が動かなくなる。回収区間の出力抑制は
  スレッドごとの印（`is_recovering`）で行い、hook の差し替えで黙らせない
- **watch 中の `yuzu.toml` 変更は取り込むが、監視・配信の前提になる設定は起動時固定**
  （`build.rs` の `WatchBuild` / `pin_restart_only`）
  - `output.dir` を差し替えると新しい出力先が監視除外から外れて無限ループになるため、
    `output.dir` / `base_url` / `dev.host` / `dev.port` / `dev.live_reload` /
    `dev.allowed_hosts` / `build.watch_ignore` は警告だけ出して起動時の値を使う
  - **サーバや監視スレッドへ起動時に渡す設定を増やしたらこの関数にも足す**
- **dev / preview の Host / Origin の検査は IP アドレスを通すのが仕様**（`yuzu-server/src/host.rs`。
  Phase 83）。DNS リバインディングはドメイン名を使うので、拒否するのは `localhost`・
  `*.localhost`・IP 以外で `dev.allowed_hosts` に無いホスト名だけ（`dev.host = "0.0.0.0"` で
  LAN の IP から開く使い方を壊さない。Vite と同じ考え方）。検査は Router の最後の
  `layer` で全経路に掛けるので、**経路を足すときは layer より前に足す**。WebSocket は
  Origin も照合し、Origin の無い接続（ブラウザ以外・テスト）は通す
- yuzu-server の serve テストは TCP バインドするため、サンドボックス内では
  PermissionDenied で落ちる（コード起因ではない）
- **監視の隠しディレクトリ判定は監視ルートからの相対パスで行う**（`WatchIgnore::is_ignored`）。
  イベントは絶対パスで届くので、全構成要素を見るとルートの祖先（`~/.config/notes/`・
  `.claude/worktrees/…`・`tempfile::tempdir()` の `.tmpXXXX`）に `.` 始まりがあるだけで
  dev が黙って再ビルドしなくなる（v0.18.1 で修正。それまではテストだけが
  `.tmpXXXX` を避けて回避していた）

### 検索インデックスと wasm

- **フォーマット追加は `serde(default)` で足し、`FORMAT_VERSION` を安易に上げない**
  - `manifest.json` は毎回フェッチされるのに `search_bg.wasm` は `_search/` の固定 URL で
    HTTP キャッシュに残るため、**再デプロイ直後の再訪問者が「新 manifest ＋ 旧 wasm」に入る**
  - bump するとそこで `VersionMismatch` になり検索が全停止する（据え置けば新機能が
    出ないだけの縮退で済む）
- 同じ理由で **wasm の既存メソッドのシグネチャは変えず新メソッドを足す** — 引数を足すと
  旧 wasm が黙って無視して「効いていないのに効いたつもりの結果」を返す

### テーマ（JS / CSS / テンプレート / 埋め込み）

- **「外側クリックで閉じる」判定に `ev.target.closest()` を使わない**。`ev.composedPath()` で判定する
  - イベントリスナーの実行ごとにマイクロタスクが処理されるため、**ハンドラ内の再描画で
    押した要素が DOM から外れた後に document へバブリングする**ことがある
  - 外れた要素の `closest` は必ず null になって誤判定する（絞り込みチップと
    「さらに N 件を表示」の両方が踏んだ）
- **`display` を指定する要素には `[hidden]` の指定を必ず添える**（`.foo { display: flex }` は
  UA の `[hidden] { display: none }` を上書きしてしまう）
- **`base.jinja` の 2 つのインライン script は外部 JS 化しない**（head の FOUC 回避 /
  サイドバーのスクロール位置復元）
  - どちらも**最初のペイントより前**に走る必要があるため
  - それ以外のテーマ JS は従来どおり `static/js/` の外部ファイル
- **ダーク定義は 2 系統**（Phase 79）: 明示の選択 `html[data-theme="dark"]` と、選択が無い
  ときの OS 追従 `@media (prefers-color-scheme: dark)` の `html:not([data-theme])`
  - **`data-theme` を付けない状態が「OS の設定に従う」**。FOUC 回避の script は保存済みの
    選択があるときだけ付ける（OS の値を書き込むと JS 無効で効かず、OS 側の切替にも追従しない）
  - theme.css の手書きの 2 ブロックは同じ中身にする（yuzu-theme のテストが縛る）。
    syntect.css と `css_vars_dark` は `css.rs` の `dark_two_ways` が 2 系統を出す
  - JS で配色を判定するときは「`data-theme` があればそれ、無ければ `matchMedia`」
    （`theme.js` / `mermaid-init.js`）
- minijinja はデフォルトで属性中の `/` をエスケープするため、テンプレートの URL 値には
  **自前の `| url` フィルタ**（`yuzu-render/src/templates.rs`）を通す
  - **`| url` は HTML 属性専用**（`&` を `&amp;` にする。生のままだと `&copy;` を
    パーサがデコードして別の URL になる）。`<script>` 内の文字列は実体参照がデコード
    されないので **`| url_js`**（`&` を残す）を使う。文脈で使い分ける
  - `| safe` は `page.body`（レンダ済み HTML）と `theme_css_vars`（`css.rs` で検証済み）
    だけに残す
  - **URL 値へ `| safe` を使わない** = yuzu は slug 化をせずファイル名がそのまま
    route → URL になるため、引用符や `<` を含むファイル名で属性・`<script>` を抜けられる
    （`<script>` 内は実体参照がデコードされないので HTML エスケープでは直らない）
- rust-embed は debug ビルドだとテーマをファイルシステムから読む（テーマ編集が再コンパイル
  不要で反映される一方、debug バイナリ単体を別マシンへ持ち出すとアセットを見失う）。
  リリースビルドは常に埋め込み
  - **埋め込みフォルダへの新規ファイル追加は cargo の再コンパイル判定に載らない**ため、
    yuzu-theme は build.rs の `rerun-if-changed=assets` で監視している
  - これが無いと「debug では動くのに release が古い埋め込みを使い回して template not found」
    になる。埋め込み crate を増やすときは同じ build.rs を付けること
- `yuzu-index`（rust-embed で `assets/search/` を埋め込む）にも同じ build.rs がある
  （Phase 81 で `THIRD-PARTY-LICENSES.txt` を足すときに付けた）。**埋め込み crate を増やすときは
  同じ build.rs を付ける**
- テーマの `static/` 配下は全部 `dist/_assets/` へ出るので、開発用の記録は `#[exclude]` で
  埋め込みから外す（`static/vendor/README.md`。ワークスペースの rust-embed は `include-exclude`
  feature 付き）

### 合成ページ

- **合成ページ（`Page.generated: Option<GeneratedKind>` = 用語集・検索結果ページ）は
  「リンク先としてだけ」有効にする**
  - `build_site_model` と `build_source_pages` の両方に混ぜてあるので route 衝突検査・
    linkcheck のターゲット・routesKey は無料で効く
  - ただし **`page.src` は実在しない**。`fs::write(&page.src)` する fmt / lint --fix は
    必ず除外する（`fs::write` は新規作成するので、忘れると `yuzu fmt` が
    `content/glossary.md` を実体化する）
  - lint・整形差分・診断のリンク元・集計行のページ数も同様（機械的な除外は `is_generated()`）
  - **集約（nav・検索索引・sitemap・ページ単位 .md）に載せるかは種別で違う** —
    用語集は載る・検索結果ページは載らない
  - 判定は `Page::in_nav / in_search_index / in_sitemap / emits_page_md` に集約してあり、
    呼び出し側で kind を直接見ない（llms だけは合成時に `frontmatter.llms = false` を
    立てて既存フィルタに乗せる）
  - **`emits_page_md(site_page_md)` はサイトの `llms.page_md` を引数で受ける**（core は設定を
    知らない。frontmatter `pageMd` も中で見る。Phase 83）。`.md` の書き出し・コピーボタンの
    `data-md-url`・llms.txt のリンク先の 3 か所がこれを通るので、呼び出し側で設定や
    frontmatter を直接見ない（1 か所だけ見ると「.md は無いのにリンクが .md を指す」になる）
  - 診断文面の設定キー名は `GeneratedKind::config_key()` が唯一の定義

### 出力先への書き込み

- **出力ディレクトリ（と `.yuzu`）への書き込みは `yuzu_core::output::write_under`、
  削除は `remove_dir_all_under` を必ず通す**（`root.join(rel)` ＋ `fs::write` を直に書かない）
  - 両者が `resolve_output_rel`（rel の字句検証）と `ensure_no_symlink_under`
    （経路のリンク検査）を内包している
  - `Path::join` は絶対パス引数で左辺を捨て `.` はファイルシステムが吸収するため
    文字列比較では防げない
  - **リンク検査は基点自身と出力ツリーの内部まで見る**（`dist/guide -> /outside` があると
    書き込みも孤児掃除もリンク先へ抜ける。基点自身がリンクなら配下をいくら検査しても無意味）
  - 逆に**基点の祖先は見ない** — macOS の `/tmp -> private/tmp` があり、`current_dir()` は
    解決済みパスを返すのでプロジェクトルート自身は常に実体
  - `output.dir` 自体の字句検証（ルート外・`input.dir` / `public` / `theme` / `.yuzu` との
    重なり）は `yuzu-config` の `load`（唯一の変換点）にある

### fmt と診断

- `yuzu fmt` の不変条件: 本文は format_commonmark の正規形・**frontmatter は生テキストを
  バイト温存**・冪等・差分なしなら書き込まない（mtime 温存）
- **fmt の独自記法温存は fmt 経路限定**
  - format_commonmark は `{#fig:x}` を `{\#fig:x}` へ、`[!NOTE]-` を `[!NOTE] -` へ変えるので
    `restore_yuzu_syntax` が書いた形に戻すが、これを呼ぶのは `format_document` だけ
  - `normalize_markdown`（llms-full.txt）は復元しないため、**llms-full.txt にエスケープ形が
    出るのは仕様**（バグと誤認して直さない）
- fmt / lint / check は **draft ページも対象**（build_source_pages）。build_site_model は
  従来どおり draft を除外する
  - nav を作らないので `crossref.numbering: "site"` でも診断時のラベル番号はページ内番号のまま

### docs サイトと CI

- `docs/` はこのリポジトリ自身のドキュメントサイト（yuzu プロジェクト。`docs/yuzu.toml` が
  ルート）
  - main push で `.github/workflows/docs.yml` が GitHub Pages へデプロイし、ci.yml でも
    check・build・SSR フォールバック検出を検証する
  - **リポジトリルートから `--root docs` で実行する**（`cd docs` しない。ci.yml・docs.yml・
    docs-links.yml・`verify` スキルすべて。出力は `docs/dist/`）。cwd からの上方向探索の
    経路は scaffold の e2e が検証する
  - 原稿は `yuzu fmt` の正規形・表記は長音符なし（`lint.terms` 準拠）で書く
- **ci.yml の docs ゲートは docs の原稿と結合している**（新機能ごとに `grep` を 1 行足す運用）
  - 特に `docs/yuzu.toml` の 25-45 行目（`[markdown]` ブロック）はインクルードの `lines=` で
    引用されているので、この範囲を動かすと原稿の中身と CI が同時に壊れる
    （`verify` スキルに直す箇所の一覧あり）
- **ci.yml の否定ゲートに `! cmd` を書かない**
  - GitHub Actions の `bash -e` でも `!` で反転したコマンドは errexit の対象外で、失敗しても
    次の行へ進む（= ゲートになっていない）
  - `if cmd; then echo "…" >&2; exit 1; fi` の形で明示的に落とす（PR #7 のレビュー指摘で
    既存 7 箇所を置換済み）
- **ci.yml の `run:` に GitHub Actions の式の開始記号（`$` と `{{`）を書かない**
  - 雛形 deploy.yml の中身を grep で縛るときなどに、`${{ steps… }}` を文字列として
    書くと Actions が bash より先に評価して空文字に置き換え、照合が別物になる。
    式を含まない断片（`steps.pages.outputs.host }}`）を `grep -F` で見る
- **MSRV の検査は `RUSTUP_TOOLCHAIN` で版を指定する**（ci.yml の `msrv` ジョブ）
  - リポジトリの `rust-toolchain.toml`（stable）が rustup の既定より優先されるので、
    `dtolnay/rust-toolchain` で古い版を入れるだけでは stable で検査してしまう
    （Phase 77 まで実際にそうだった）。`rustc --version` の確認も外さない
  - MSRV はワークスペース 1.87（mikan の依存 ruzstd の都合）・kabosu と tankan は 1.85。
    依存を上げて MSRV が上がったら、README・リリースノート（release.yml）・docs も直す
- `docs/design/` は git 管理外のローカル設計ノート。公開物（コード・README・コミット）から
  参照しない

### 開発コンテナ

- 開発コンテナ内（`.devcontainer/`）は `CARGO_TARGET_DIR=/cargo-target` のため、CLI 実機確認は
  `"$CARGO_TARGET_DIR/debug/yuzu"` を使う（`./target/debug/yuzu` は**存在しない**）
- 環境定義は `.devcontainer/Dockerfile` が唯一で、devcontainer.json とラッパーの不変条件は
  `.devcontainer/README.md` の表を参照
