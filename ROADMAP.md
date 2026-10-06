# ロードマップ

yuzu の開発計画と、これまでのリリースの内訳。**このファイルが Phase 状態の正**
（README には現在の版と概要だけを置く）。

## 現在: v0.18（Phase 76〜80）

**v0.17 まで公開済み**（kabosu 0.2.0 / tankan 0.2.0 / mikan 0.2.0 も crates.io で
公開済み。yuzu のリリースとは非同期。kabosu の publish 前に fuzz を回す規律は
CLAUDE.md にある）。

軸は「**公開サイトの仕上げ**」。v0.13 から持ち越していた 4 件（`<head>` メタ /
パーマリンクの到達性 / ページメタ / OS ダーク追従）を 4 つの Phase で消化する。
本文 HTML が変わる 2 件は Phase 78 に束ね、キャッシュ形式の bump を 1 回で済ませる。
その前に、10-04 の見直しで見つかった「初めて使う人が最初に踏む不具合」6 件を
Phase 77 で直す。

以下は策定の詳細。

- 動機: 公開物（HTML）の質は v0.12「読む体験の完成」以来手を入れておらず、
  v0.13 Phase 61 からの持ち越し 4 件（`<head>` メタ / 見出しパーマリンクのキーボード
  到達性 / ページメタ / OS ダーク追従）が候補欄に 4 版分残っている。v0.16・v0.17 は
  内部と CLI で、サイトの見え方は変わっていない
- ゴール: 共有（SNS カード）・検索エンジン（canonical）・支援技術（キーボード到達性）・
  OS 設定（ダーク追従）の 4 方向から「公開サイトとして過不足ない」状態にする。
  新しい記法は足さない
- **本文 HTML が変わるのは Phase 78 だけ**にまとめ、`CACHE_FORMAT_VERSION` の bump と
  スナップショット全更新を 1 回で済ませる（Phase 76・79 はテンプレート・CSS・JS だけ。
  Phase 77 の不具合修正も本文 HTML とキャッシュ形式を変えない）
- Phase は「テンプレートだけ → 不具合修正 → 本文 HTML（bump 1 回）→ CSS の 2 系統化 →
  dogfooding」の順。着手時に判断点を決めてから実装する
- 追加（10-04）: Phase 76 の完了後、上級プログラマー・上級マネージャー・一般利用者の
  3 つの視点でプロジェクト全体を見直した。Linux で `yuzu dev` の再ビルドが止まらない・
  Mermaid の入力でビルドが落ちる等、初めて使う人が最初の数分で踏む不具合が見つかり、
  公開サイトの仕上げより先に直すと決めて Phase 77 として差し込んだ
  （旧 77〜79 は 78〜80 へ繰り下げ）

### 76 `<head>` メタ（canonical / OGP） ✅

**概要**: `base.jinja` の `<head>` に canonical と OGP（`og:*` / `twitter:card`）と
`generator` を足した。新しい設定キーは `site.image`（og:image の素材）の 1 つ。
canonical / `og:url` / パス指定の og:image は `base_url` がフル URL のときだけ出す
（sitemap と同じゲート）。判断点 4 つはすべてユーザ確認のうえ推奨案。

- 着手時の実測
  - `<head>` にあるメタは `<title>` と `description` だけ。素材は `site.title` /
    `site.description` / `page.description` / `site.lang` / `site.logo`（docs では SVG）が
    既にあり、og:image だけ素材が無かった
  - sitemap.xml の「`base_url` がフル URL のときだけ生成」の判定が `pipeline.rs` に
    文字列比較で直書きされていた
  - `<head>` を含む insta スナップショットは 4 件
- やったこと
  - `site.image` を追加（schema / codec / config リファレンス / scaffold のコメント）。
    パスは `site.logo` と同じ `public_url` で解決し、パス指定は base がフル URL のときだけ
    `og:image` に出す。フル URL 指定は base に依らず出す
  - `UrlResolver::is_absolute_base` を足し、sitemap のゲートもこれに揃えた
  - `PageCtx::canonical_url`（フル URL 時だけ Some）と `SiteCtx` の `image_url` /
    `locale` / `generator`。404 ページは URL を持たないので canonical / og:url を出さない
  - `base.jinja`: meta description をページ → サイトの順にフォールバック（og:description と
    同じ値）、`og:type` = website / `og:site_name` / `og:title` / `og:description` /
    canonical ＋ `og:url` / `og:image` / `og:locale` / `twitter:card` = summary /
    `generator`。空行を混ぜないよう `{% set %}` とコメントは行末に置いた
  - テスト: パスだけの base では URL 系を出さない / フル URL なら canonical・og:url・
    og:image が絶対 URL / フル URL 指定の image は base に依らず出る / description の
    フォールバック（404 で見る）/ `og_locale` の単体テスト。スナップショット 4 件更新
  - docs: `guide/deploy.md` に「共有カード（OGP）と canonical」の節、config リファレンスの
    `site` 表。ci.yml の docs ゲートと e2e（パス base で og:image 無し → フル URL で
    canonical / og:url / og:image / description フォールバック）
- 決めたこと
  - **og:image は新キー `site.image`** — `site.logo` は docs も scaffold も SVG で、SVG を
    受け付けないクローラが多い。利用者が PNG / JPEG を用意する
  - **`og:type` は全ページ website** — ドキュメントサイトのページは記事ではなくサイトの
    一部。`article` は公開日・著者などの付随プロパティを期待されるが素材が無い
  - **`twitter:card` は summary 固定で出す** — 1 行で済み、og:image が無くてもタイトルと
    説明のカードは出る。他の値は `og:*` にフォールバックするので `twitter:*` を増やさない
  - **フル URL 無しの canonical は出さない** — RFC 6596 §3 は相対 IRI を許すが、ホストが
    分からない状態の canonical は同一性の宣言として弱く、`og:url`（絶対 URL 必須）とも
    揃わない。絶対 URL で出すのをプロジェクト方針にする
  - **`og:locale` は `site.lang` が地域付きのときだけ** — `ja` から `ja_JP` を推測すると
    `en` → `en_US` か `en_GB` かで誤りうる。推測しない
  - **`generator` にバージョンを含めない** — 含めるとリリースのバンプごとに HTML
    スナップショット 4 件が動き、「バンプコミットは Cargo.toml と Cargo.lock だけ」の
    規律と衝突する。ビルドの識別は `__yuzu/build_id` が担う
  - エイリアスのリダイレクト HTML が出している相対 canonical（移動先ページ）は今回
    触らない — 移動先の宣言としては相対でも機能しており、フル URL 時だけ絶対にする
    変更は Phase 80 の dogfooding で要否を見る
  - レビュー指摘 2 件: **`| url` フィルタは HTML 属性専用にして `&` を `&amp;` に**
    （生のままだと `?label=a&copy;b` がパーサで `a©b` に化けて別の画像 URL になる。
    `<script>` 内の文字列は実体参照がデコードされないので `| url_js` を新設して
    リダイレクト HTML の `location.replace` はそちら）/ **`og:locale` は地域サブタグ
    （2 文字のアルファベットか 3 桁の数字）があるときだけ**（`zh-Hant` の `Hant` は
    文字体系で、`zh_HANT` を出していた。`zh-Hant-TW` は `zh_TW`）

### 77 初めて使う人が最初に踏む不具合の修正 ✅

**概要**: 10-04 の見直しで見つかった、初めて使う人が最初の数分で踏む 6 件を直した。
Linux での再ビルドの繰り返し / Mermaid の入力によるビルドの停止 / frontmatter の
閉じ忘れ / 雛形 deploy.yml の版指定なし / 1.85 で動いていなかった MSRV ジョブ /
異常な `base_url` での panic と panic 時の終了コード。本文 HTML とキャッシュ形式は
変えていない（`CACHE_FORMAT_VERSION` の bump は Phase 78 の 1 回のまま）。判断点 7 つの
うち 4 つはユーザ確認のうえ推奨案、残り 3 つは確認の応答が無かったので推奨案で進めた。

- 着手時の実測（コードを読んで確認し、1・2・3・6 は開発コンテナで実行して再現した）
  1. **Linux で `yuzu dev` / `build --watch` の再ビルドが止まらない** — notify 8.2 の
     inotify は監視マスクに `OPEN`（ファイルを開いただけ）を含む（`inotify.rs:427`）。
     notify-debouncer-mini はイベントの種類を捨ててパスだけを渡し、yuzu 側も種類で
     絞っていなかったので、ビルドが原稿を読むたびに「変更」と判定された。`yuzu new`
     直後の `yuzu dev` で 10 秒に 33 回（Linux 6.18・ext4）。macOS（FSEvents）では
     起きないため気付かなかった
  2. **Mermaid の入力でビルドが落ちる（tankan）** — gantt は `parse_duration_days` が
     末尾 1 バイトで単位を切るため `5日` で panic（exit 101）。state は `state A {` の
     内側で `state A {` を開くと親子が循環し、レイアウトのスコープの辿り上げが
     メモリを使い切る。yuzu 側は `render_svg` の Err はクライアント描画へ切り替えるが、
     panic は受け止めていなかった
  3. **frontmatter の閉じ忘れが無言で、`yuzu fmt` すると痕跡が消える**。一方で先頭の
     `---` は CommonMark では正当な区切り線でもあり、「先頭が `---` で閉じが無い」だけ
     では区別できない。さらに comrak は 1 行目が `---` で後ろに `---` だけの行があれば
     中身を問わず frontmatter とみなすため、区切り線を 2 本使う文書は、YAML が壊れて
     exit 2 になるか、**間が YAML のコメントとして通って本文が黙って消える**
     （後者は着手後に見つけた）
  4. **雛形 deploy.yml が main を版指定なしでインストールする**（リリース前の非互換が
     利用者のデプロイに届く）
  5. **MSRV ジョブが 1.85 で動いていない** — `rust-toolchain.toml`（stable）が rustup の
     既定より優先され、実際は stable で検査していた。**1.85 で通すと、mikan の依存
     ruzstd 0.8.2 が 1.87 で安定化した API（`is_multiple_of`）を使っていて通らない**。
     1.87 ならワークスペース全体が通り、kabosu・tankan は 1.85 でも通る
  6. **`base_url = "/:x/"` で `yuzu preview` が panic** — axum 0.8 は `:` / `*` で始まる
     セグメントを旧構文としてルート登録時に panic で拒む。`{` / `}` はルートの
     パラメータ構文。panic 時の終了コードは 101。`dev` / `build --watch` の再ビルドは
     監視スレッドで動くため、そこでの panic は main に届かず、監視だけが止まって配信が
     残る
- やったこと
  1. 監視（yuzu-server `watch.rs`）: notify-debouncer-mini を外し、notify を直接使う。
     監視スレッドで「受信 → 種類で絞る → 静かになるまで待つ」を行う。開いた・読んだ
     だけのイベントは捨て、書き込みを終えて閉じた（`CLOSE_WRITE`）は残す。変更が
     途切れず届いても debounce 間隔の 10 倍で区切る。依存が 1 つ減った
  2. 監視スレッドの panic: コールバックを `catch_unwind` で受けて監視を止め、
     `WatchHandle::take_failure` の合図（`WatchFailure`）で `serve` に知らせる。
     `serve` は配信を止めて `ServerError::WatchStopped` を返し、`dev` / `build --watch`
     は exit 2 で終わる。執筆中の構文エラーのような Err は従来どおりログだけで続ける
  3. tankan: gantt は `strip_suffix` で単位を判定（有限でない値も拒否）。state は
     複合状態を開くとき、新しい親から辿った鎖に自分が含まれればパースエラー
     （`is_within`）。flowchart レイアウトの `scope_chain` にクラスタ数の上限を付けた
     （万一の循環でも止まる）。回帰テストは公開 API（`render_svg`）経由で 3 件
  4. panic の回収（yuzu-core `recover.rs`）: `catch` と、回収区間にいるかを示す
     スレッドごとの印 `is_recovering`。Mermaid の SSR（yuzu-render）と comrak の整形
     （`catch_formatter_panic`）の 2 か所で使う。後者はこれまで hook を差し替えて
     黙らせていたが、並列に動く他スレッドの本物の panic まで黙らせるのでこちらに
     寄せた。描画の panic は警告 1 行を出してクライアント描画へ切り替える
  5. CLI（`main.rs`）: `run_catching` で panic を exit 2 に（rayon の並列処理中の panic
     もここで受かることをテストで確認）。`install_panic_hook` は回収区間では何も出さず、
     それ以外は「内部エラー（yuzu の不具合）」を 1 行で出す。**hook の中では終了
     させない**。stderr へ書けなくても panic しない
  6. frontmatter（yuzu-core `frontmatter.rs` の `detect_unrecognized`）: lint の新ルール
     `frontmatter-unrecognized`（error・抑制不可。lint で唯一の error）で 3 つを報告し、
     build でも警告する。fmt はそのページを書き換えない
     - 閉じ忘れ: 1 行目が `---` ちょうどで、2 行目が「ASCII のキーとコロン」の形
       （`title:` 等）なのに閉じが無い。2 行目が空行・見出し・日本語の文なら対象外
     - TOML 形式: `+++` で始まり 2 行目が `キー =`
     - 区切り線の誤読: 1 行目が `---` で、comrak が切り出した中身が YAML のマッピングと
       して読めない（見出しだけ = コメント扱いで null・本文の文・箇条書き）。YAML が
       壊れて exit 2 になる側は、エラー文に「先頭の区切り線は `***` で」の案内を足した
  7. 雛形 deploy.yml: `cargo install --locked --git … --tag v<版> yuzu-cli`。版は
     `yuzu new` が雛形の印（`__YUZU_VERSION__`）を自分の `CARGO_PKG_VERSION` で埋める
  8. MSRV: ワークスペースの `rust-version` を 1.87 に上げ、kabosu・tankan は各
     Cargo.toml で 1.85 を宣言。ci.yml の `msrv` ジョブを 2 段（1.85 で kabosu・
     tankan、1.87 でワークスペース全体）にし、`RUSTUP_TOOLCHAIN` で版を指定して
     `rustc --version` も確かめる。README・リリースノート・docs の「1.85」を直し、
     インストール手順に `--locked` を付けた
  9. `base_url`: preview / dev のルート登録で `without_v07_checks` を使い、`{` / `}` は
     `%7B` / `%7D` にする（`literal_route`）。`/:x/`・`/*x/` は字句どおり、`/a{b}/` は
     ブラウザが送るエンコード済みの `/a%7Bb%7D/` で配信する（どれも panic しない）
  10. docs: 診断ルール一覧（`frontmatter-unrecognized` の節）・デプロイガイド（版の固定）・
      内部設計（監視イベントの種類と panic）。ci.yml に docs ゲート 2 行と e2e
      （deploy.yml のタグ・`build --watch` を 6 秒動かして再ビルド 0 回・frontmatter の
      閉じ忘れが check で error / 区切り線の文書は誤検出しない）
- 決めたこと
  - **監視は notify を直接使う**（debouncer-full に替えない）— 依存を増やさず、監視
    スレッドが自前になるのでコールバックの panic もそこで受けられる
  - **雛形は `--tag` を埋め込む**（バイナリ取得にしない）— 変更が小さく、rust-cache が
    `~/.cargo` を保存するので 2 回目以降は「インストール済み」で飛ばされる。版を雛形に
    直書きしないのは「バンプコミットは Cargo.toml と Cargo.lock だけ」の規律のため。
    未リリースの main からビルドした `yuzu new` は直前のリリースのタグを指す
  - **frontmatter は「キーらしい行」なら error** — 既知キーに限ると `titel:` のような
    打ち間違いを拾えない。日本語の文（`用語: 説明`）は ASCII キーの形に当たらないので
    候補にならない。区切り線のつもりで 2 行目に英字のキーの形を書いた文書は誤検出
    するが、`---` の次に空行を入れれば外れる（文面で案内）
  - **区切り線の誤読は判定を変えずに報告だけ足す** — comrak の切り出しを前段で
    上書きすると、空行やコメントで始まる正しい frontmatter を壊しうる。誤読の判定は
    中身を YAML として読んだ結果で行う（マッピングなら正しい frontmatter）。当初は
    「キーの形の行が 1 つも無い」で判定していたが、引用符付きキー `"title": x` や
    フロー形式 `{title: x}` の正しい frontmatter を誤検出した（PR #22 のレビュー指摘）
  - **MSRV は本体と mikan を 1.87、kabosu・tankan は 1.85** — 公開ライブラリの対応範囲を
    不必要に狭めない。ruzstd を古い版に固定する案は、`cargo install`（`--locked` なし）が
    最新を選ぶので利用者の環境では効かない
  - **panic の出力は回収区間だけ黙らせ、それ以外は 1 行**。回帰テストは単体テストだけ
    （本番コードにテスト専用の口を作らない）
  - **`base_url` は検証して拒むのでなく、字句どおりに配信する** — `/:x/` は URL として
    正当で build の出力にも問題が無いので、preview / dev 側を直した（ROADMAP の当初案
    「設定の読み込みで設定エラー」から変更）。`{` / `}` は `{{` で重ねてただの文字にする
    方法もあるが、axum はエンコードされたままのパスで照合し、ブラウザは `{` / `}` を
    必ずエンコードして送るので、エンコード済みの形でマウントしないと一致しない
  - **監視の停止は graceful shutdown にしない** — 接続が閉じるまで待つので、ライブ
    リロードの WebSocket が開いている限り終わらない。プロセスはすぐ終わるので打ち切る
- 確認
  - 開発コンテナ（Linux）で `yuzu new` 直後の `build --watch` / `dev` を 8 秒ずつ
    動かして再ビルド 0 回、1 回保存すると 1 回だけ再ビルド
  - ホスト（macOS）でも `yuzu dev` が動くことをユーザが確認
  - CI の `msrv` ジョブが rustc 1.85.1（kabosu・tankan）と 1.87.0（ワークスペース）で通ることを確認

### 78 本文 HTML の到達性とページメタ（CACHE bump を 1 回に束ねる） ✅

**概要**: 見出しのパーマリンクをキーボードと支援技術から到達できる形にし、読了時間と
文字数をページメタに出した。どちらも本文 HTML / キャッシュ形式が変わるので 1 つの
Phase に束ね、`CACHE_FORMAT_VERSION` を 22 → 23 の 1 回で済ませた。判断点 3 つは
確認の応答が無かったので推奨案で進めた（作り方は comrak の HeadingAdapter / id は
見出し自身・リンクは末尾 / 読了時間は既定で表示・速度は固定）。

- 着手時の実測
  - comrak の `header_ids` の出力は見出しの先頭に
    `<a href="#id" aria-hidden="true" class="anchor" id="id"></a>` で固定（comrak 0.53
    `html.rs:623`）。`theme.css` が `.anchor` を `visibility: hidden` にしており、
    `aria-hidden` と合わせてキーボードからも支援技術からも到達できなかった
  - comrak には見出しの描画を差し替える `HeadingAdapter`（`plugins.render.heading_adapter`）
    がある。渡される見出し文（`HeadingMeta::content`）は comrak 自身の採番と同じ
    `collect_text` で作られる
  - id を探す JS は `scrollspy.js`（`getElementById` → `closest("h1…h6")`）と
    `details-target.js`（`getElementById` → 祖先の details を開く）。どちらも id が
    見出し自身に移っても動く。`scroll-margin-top` は `.anchor` に掛けていた
  - ページメタの表示場所は `page.jinja` の `.page-meta`（最終更新・編集リンク）
- やったこと
  1. 見出しの描画（yuzu-core `markdown/heading.rs` の `PermalinkHeadings`）: comrak の
     HeadingAdapter で `<h2 id="x">見出し<a class="anchor" href="#x" aria-label="「見出し」へのリンク"></a></h2>`
     を出す。id は comrak の既定と同じ入力を同じ `Anchorizer` に文書順で通すので、
     既存の `#id` リンク・TOC・検索の位置情報は変わらない（重複見出しの `-1` も同じ）。
     ラベルの見出し文は属性用にエスケープする
  2. CSS: `.anchor` を `visibility: hidden` から `opacity: 0` にし、見出しの `:hover` と
     `.anchor:focus-visible` で出す（`#` は見出しの右）。`scroll-margin-top` は id を
     持つ見出しへ移した。`scrollspy.js` はコメントだけ直した（`closest` は自身も含む）
  3. 本文の分量（yuzu-core `markdown/reading.rs`）: 本文 HTML 化（`render_body_html`）の
     中で、Markdown 断片を展開した直後・パス1 がコードブロック等を HtmlBlock へ差し替える
     前に、`Text` と行内コードを数える（コードブロック・図・数式・生 HTML・frontmatter は
     ノードの種類で外れ、画像の代替テキストは配下を除外）。日本語（かな・漢字・和文の
     約物・全角英数）は 1 分 500 字、英数字は 1 分 200 語として足して切り上げる。
     `RenderedBody::reading` と本文キャッシュ（`CachedBody.reading`）に載せる。
     断片を使うページは本文ごとキャッシュしないので、参照先だけの編集でも数え直る
     - 当初は `extract_meta`（原文だけを読む）で数えており、` ```include ` で取り込んだ
       文章が丸ごと抜けていた（同じ 1,200 字が直接なら 1,202 字、取り込むと 2 字。
       PR #25 のレビュー指摘）
  4. 表示: `.page-meta` に「約 N 分で読めます（M 文字）」（文字数は 3 桁区切り）。
     `theme.reading_time`（既定 true）と frontmatter `readingTime`（既定 true）の両方が
     有効で、文章があり、合成ページでないときだけ。`.page-meta` は情報を左に並べ、
     編集リンクだけ右端に寄せる形にした
  5. 設定: `theme.reading_time`（schema / codec / 雛形 yuzu.toml / config リファレンスの
     2 箇所）と frontmatter `readingTime`（`KNOWN_KEYS`・執筆ガイドの frontmatter 一覧）
  6. `CACHE_FORMAT_VERSION` 22 → 23。スナップショットは本文 4 件・ページ 3 件を目視して
     更新（見出しの形と読了時間の行だけが変わり、検索結果ページには読了時間が出ない）
  7. docs: 執筆ガイドに「見出しへのリンク」「読了時間と文字数」の節。ci.yml の docs
     ゲート（見出しの新しい形・旧形式の `aria-hidden` アンカーが残っていない・
     `.anchor:focus-visible`・読了時間の行）と verify スキル
- 決めたこと
  - **パーマリンクは comrak の HeadingAdapter で描く** — 出力 HTML の文字列を後処理すると
    comrak の出力形式に依存し、ラベル用の見出し文を id から引き直す必要がある。
    HeadingAdapter なら採番の入力が comrak 自身と同じで、Anchorizer の 4 経路目も
    作らない
  - **id は見出し自身に付け、リンクは末尾に置く** — 飛んだ先が見出しそのものになり、
    読み上げも「見出し文 → リンク名」の順になる。リンクを見出しの外に出す案は、
    見出しごとに包む要素が要り本文の CSS やテーマ上書きへの影響が大きいので採らない。
    見出しの読み上げにリンク名が混ざる点は残る
  - **読了時間は既定で表示し、速度は固定** — 設定キーは表示の有無（`theme.reading_time`）
    だけにした。文字数は検索の manifest と llms.txt には載せない（検索の
    `FORMAT_VERSION` の話になるため）
  - **表示は `.page-meta`（ページ末尾）** — 当初の計画どおり。ページ先頭のほうが
    読む前の目安として役に立つので、Phase 80 の dogfooding で位置を見直す
    → **Phase 80 でページ先頭（パンくずの下・本文の前）の `.page-reading` へ移した**
- 確認
  - 単体テスト（数え方 4 件・3 桁区切り）・本文の結合テスト（ラベルのエスケープ・
    装飾を平らにしたラベル・TOC と本文の id の一致・断片で取り込んだ文章を直接書いた
    ときと同じに数える）・描画の結合テスト（既定で表示 / frontmatter で消える /
    `theme.reading_time = false` で消える）・インクリメンタルビルドのテスト（断片の
    参照先だけを書き換えても読了時間・文字数が数え直る）
  - docs サイトのビルドとゲート（執筆ガイドで「約 10 分で読めます（5,611 文字）」）
  - **未確認**: ブラウザで hover 無しに Tab だけで `#` に到達し、Enter で見出しへ移ること
    （この環境にブラウザが無い。Phase 80 の dogfooding で見る）

### 79 OS ダーク追従（JS 無効時・`theme.dark = false` 時） ✅

**概要**: JS が無効でも、切替ボタンを出さない設定でも、OS のダーク設定に従うようにした。
`data-theme` を付けない状態を「OS の設定に従う」にし、ダーク定義を CSS の
`prefers-color-scheme` 側にも置いた。`theme.dark` は 3 値（`"toggle"` / `"auto"` /
`"light"`）にし、旧形式の bool も同じ見た目になる値で読む。判断点 2 つはユーザ確認の
うえ推奨案（3 値化・旧値も受ける / CSS は 3 か所とも 2 系統化）。本文 HTML は変えない
（キャッシュ形式の bump なし）。

- 着手時の実測
  - `base.jinja` が `data-theme="light"` を無条件で書き、`theme.dark = true` のときだけ
    head のインライン script が localStorage → OS の設定の順で `data-theme` を差し替えて
    いた。**JS 無効なら常にライト、`theme.dark = false` なら OS がダークでもライト**
  - ダーク定義は 3 か所: `theme.css` の変数ブロック / yuzu.toml の `css_vars_dark`
    （`css.rs` が生成）/ `syntect.css` のダーク配色（`css.rs` が生成）。いずれも
    `@media screen` 内の `html[data-theme="dark"]` スコープ
  - `data-theme` を見る JS は `theme.js`（ボタン）と `mermaid-init.js`（クライアント描画の
    図の配色・属性の変化を監視して再描画）
- やったこと
  1. 設定: `theme.dark` を `DarkMode`（`Toggle` 既定 / `Auto` / `Light`）にした。
     codec は bool も受け、`true` = toggle、`false` = light（従来の false と同じ見た目）。
     不正値は位置付きの設定エラー（指定できる値の一覧入り）
  2. テンプレート: `<html>` に `data-theme` を付けないのを既定にし、light のときだけ
     `data-theme="light"`。head の script（toggle のときだけ）は**保存済みの選択が
     あるときだけ**付ける。切替ボタンも toggle のときだけ
  3. JS: `theme.js` は「今見えている配色」（明示の選択が無ければ OS の設定）の反対へ
     切り替える。`mermaid-init.js` は同じ規則で配色を決め、明示の選択が無いときは
     OS 側の切替（`matchMedia` の change）でも再描画する
  4. CSS: ダーク定義を 2 系統にした。1 つ目は明示の選択（`html[data-theme="dark"]`）、
     2 つ目は選択が無いときの OS 追従（`@media screen and (prefers-color-scheme: dark)` の
     `html:not([data-theme])`）。どちらも画面専用で、印刷は従来どおりライト
     - `theme.css` の手書きの変数ブロックは 2 つ並べ、中身の一致を yuzu-theme の
       テストが縛る。両方に `color-scheme: dark` を足した（スクロールバー・フォーム部品）
     - `syntect.css` と `css_vars_dark` は `css.rs` の `dark_two_ways` が同じ生成関数から
       2 系統を出す。syntect.css は docs サイトで 26 KB（gzip 2.4 KB）
  5. docs: デプロイガイドに「ダークモード」の節（3 値の表・JS 無効でも効くこと・旧値の
     読み方）、config リファレンスの `dark` の行と全キー例、雛形と docs の yuzu.toml を
     `dark = "toggle"` に。ci.yml の docs ゲートと verify スキル
- 決めたこと
  - **`theme.dark` は 3 値にし、旧形式の bool も受ける** — bool のまま「false でも OS が
    ダークならダーク」にすると、ライト固定のつもりで false にしていたサイトの見た目が
    変わる。旧 `true` / `false` は従来と同じ見た目になる値（toggle / light）で読む
  - **CSS は 3 か所とも 2 系統** — 変数だけにすると JS 無効でコードブロックだけライトの
    配色が残る。生成側は 1 つの関数から作るので食い違わない。手書きの theme.css だけは
    テストで一致を縛る
  - **`data-theme` を付けない状態を「OS 追従」にする** — 以前の script のように OS の
    値を `data-theme` に書き込むと、JS 無効では効かず、表示中の OS の切替にも追従しない
  - ダーク固定（`"dark"`）は足していない（要望が無い）
- 確認
  - 設定の読み込み（3 値・旧 bool・不正値）・3 値ごとの `<html>` とボタンと script の
    出し分け・CSS 生成（2 系統が同じ規則数・`css_vars_dark` の OS 側）・theme.css の
    2 ブロックの一致の各テスト。スナップショット 4 件（`<html>` と head の script だけが
    変わる）を目視して更新
  - docs サイトのビルドとゲート
  - **未確認**: ブラウザでの実際の切り替わり（OS のダーク設定・JS 無効・各値・ボタンでの
    切替と再読込後の保持・クライアント描画の図の追従）。この環境にブラウザが無いので
    Phase 80 の dogfooding で見る

### 80 dogfooding ⬜

**概要**: Phase 76〜79 を docs サイト・雛形（`yuzu new`）・CI で実際に使って仕上げる。
読了時間をページ先頭へ移し、雛形の deploy.yml でも共有カードが出るようにし、雛形の
原稿に新機能の実例を足した。ブラウザが要る確認（Tab 操作・ダーク表示・SNS カード）は
開発環境にブラウザが無いのでユーザが行い、結果をここに記録する。判断点 3 つは
ユーザ確認のうえ推奨案（og:image の PNG はユーザが用意し雛形には同梱しない /
読了時間は先頭へ / 雛形 deploy.yml はフル URL）。

- 着手時の実測
  - 画像を作る道具（rsvg-convert・ImageMagick・Inkscape）が開発環境に無く、og:image の
    PNG は作れない
  - リダイレクト HTML の canonical（Phase 76 で「Phase 80 で要否を見る」とした件）は、
    移動先が `resolver.page_url()` なので base がフル URL なら既に絶対 URL。**変更不要**
  - docs.yml は既に `https://<host><base_path>/` を渡しているが、雛形の deploy.yml は
    base path だけを渡しており、利用者のサイトには canonical・共有カード・sitemap.xml が
    出ていなかった
- やったこと
  1. 読了時間をページ先頭へ: `page.jinja` で draft バナーの後・本文の前に
     `<p class="page-reading">` として出し、`.page-meta`（最終更新日・編集リンク）からは
     外した。CSS は `.page-reading`。テンプレートだけの変更なのでキャッシュ形式は
     変わらない。スナップショット 2 件（読了時間の行が本文の前へ移るだけ）を目視して更新
  2. 雛形 deploy.yml: docs.yml と同じく `https://<host><base_path>/` を渡す（configure-pages の
     `base_url` 出力は Enforce HTTPS が無効だと http になるので host から組み立てる）。
     雛形のテストと ci.yml の e2e で形を縛る。デプロイガイドの GitHub Pages 節も直した
  3. 雛形の原稿（getting-started.md）: frontmatter の例に `readingTime: false`、
     「見出しへのリンク」「読了時間」の節、「ダークモード」節を 3 値と OS 追従に
  4. 文言: 執筆ガイド・config リファレンス・雛形 yuzu.toml・schema のコメントの
     「ページ末尾」を「ページの先頭」に。ci.yml の docs ゲートと verify スキルの
     読了時間の grep を新しい class に
  5. og:image: `docs/public/images/og.png`（1200×630・231 KB）を作り、`docs/yuzu.toml` の
     `[site]` に `image = "/images/og.png"` を足した。画像はネイビーの背景に、ロゴの
     ゆずの実（陰影と葉を足して拡大）・線で描いたワードマーク「yuzu」・
     「Markdown マーク → ブラウザ窓」の絵。開発環境にフォントも画像変換ツールも無いので、
     SVG を手で描き、使い捨ての resvg で PNG に書き出した（文字はフォントを使わず線で
     描いた）。`docs/yuzu.toml` の 25〜45 行目（インクルードの `lines=` で引用）は、冒頭の
     コメントを 1 行まとめて行番号を保った。ci.yml の docs ゲートに
     `test -f docs/dist/images/og.png`
     - 注意: `twitter:card` は summary（Phase 76 の判断）なので、X では画像が正方形に
       切り抜かれた小さなサムネイルになる（中央の 630×630 = 実の右半分と「yu」あたり）。
       Slack・Facebook・LinkedIn などは横長のまま出る
- 残り
  - **ユーザによるブラウザ確認**: Tab だけで見出しの `#` に届き Enter で移るか / 読了時間の
    見た目 / ダーク（OS 追従・JS 無効・◐ の切替と再読み込み後の保持・`"auto"` と
    `"light"`・クライアント描画の図の追従）/ SNS カード（og.png を置いて公開した後）

## v0.10.1 レビューの持ち越し

v0.10.1（外部コードレビュー対応）で「今回は入れない」と判断したもの。
**判断の根拠ごと残す**（同じ検討を繰り返さないため）。

- ✅ **URL のパーセントエンコード全面対応** — v0.15 Phase 64 で実装済み
  （内訳は下の「完了済み: v0.15」）
- ✅ **`syntect.css` の無条件出力** / **`ServeDir` が dist 内のリンクを辿る** —
  v0.15 Phase 65 で実装済み
- ✅ **キャッシュ保存の原子性** — v0.11 Phase 53 で実装済み
  - `write_atomic_under` が global.json のみ tmp → rename = 当時の見積もりどおり
    安価な側だけ（詳細は v0.11 の内訳）
- ⬜ **`.devcontainer/post-create.sh` の Claude Code インストーラ** — 取得した
  `install.sh` を検証せず bash へ渡している
  - 緩和されている点: **インストーラ自身がダウンロードしたバイナリを SHA-256 検証する**
    （バージョンごとの `manifest.json` の値と照合し、不一致なら削除して終了）
  - 残るギャップ: スクリプトの TOFU のみ
  - 固定を見送った理由: `install.sh` に公開チェックサムが無く、ベンダ更新のたびに
    devcontainer のビルドが壊れる
  - 着手するなら: バージョン指定（`bash -s -- <version>`）は可能なのでそこから

## 10-04 見直しの持ち越し

**概要**: 10-04 にプロジェクト全体を上級プログラマー・上級マネージャー・一般利用者の
3 つの視点で見直した結果のうち、v0.18 Phase 77 で直した 6 件以外の指摘。
3 人とも「作りの規律は高い」と評価したうえで、配布物の体裁（ライセンス表記・
脆弱性窓口）・利用者の体験と docs の食い違い・1 人運用の持続性・露出に穴がある、
という見立てで一致した。優先度は「配布物として必要なもの」→「利用者の体験と docs」→
「運用と持続性」→「急がないコード整理」の順（3 人の合意）。

各項目は「現状（根拠）・対処案・工数（小 / 中 / 大）」の順に書く。根拠のうち、
コード・ファイル・`gh` の結果で確かめたものは断定し、確かめていないものは
**未確認**と書く。ファイルの行番号は 10-04（Phase 77 マージ後）時点。

### 配布物として必要なもの

- ⬜ **第三者ライセンスの表記が配布物に無い**（工数: 小〜中）
  - リリースのアーカイブに入れているのは `README.md` と yuzu 自身の `LICENSE-MIT` /
    `LICENSE-APACHE` だけ（`release.yml:142` の `cp`）。バイナリには 200 余りの
    crate が入っており、comrak（BSD-2-Clause）や MIT の crate は、バイナリで配るときに
    著作権表示と許諾文を添える必要がある。two-face は「同梱する構文定義に個別の
    ライセンスがある」として表示用の `acknowledgement` モジュールを用意しているが、
    yuzu は使っていない
  - `yuzu build` が利用者のサイトへ配る mermaid.min.js・KaTeX（JS・CSS・フォント）・
    分かち書きモデル（`_search/model.zst`）にもライセンス文が付いていない
  - crates.io で公開している tankan / mikan / kabosu の各 crate ディレクトリに
    LICENSE ファイルが無い（ワークスペース直下にしか無い）。パッケージにも入らない
    （`cargo package --list` で 3 crate とも LICENSE が 0 件）。mikan は分かち書き
    モデルも同梱している
  - 対処案: cargo-about 等で THIRD-PARTY-LICENSES を生成してアーカイブに入れる /
    dist にもライセンス一覧（vendor 資産の分）を出す / 3 crate のディレクトリに
    LICENSE-* を置く（シンボリックリンクは `cargo package` が実体を入れるか要確認）
- ⬜ **vendor 資産の更新メモが公開サイトに配信されている**（工数: 小）
  - `crates/yuzu-theme/assets/static/vendor/README.md`（取得元・版・ハッシュの記録）が
    テーマ資産として埋め込まれ、`dist/_assets/vendor/README.md` に出る。このサイトでも
    `https://ai.implementer.net/yuzu/_assets/vendor/README.md` で配信されている
    （見直し時に確認）。利用者のサイトでも同じ
  - 対処案: rust-embed の対象から外す（`#[exclude]`）。上のライセンス一覧で置き換える
- ⬜ **脆弱性の窓口と監視が無い**（工数: 小）
  - `SECURITY.md` が無い。GitHub の secret scanning・push protection・dependabot の
    セキュリティ更新はすべて disabled（`gh api repos/…` の `security_and_analysis`）
  - CI に cargo-deny / cargo-audit が無い。`ci.yml` と `fuzz.yml` に `permissions:` が
    無い（既定の権限で動く。`docs.yml` / `release.yml` / `docs-links.yml` には有る）
  - 利用者のサイトへ配る mermaid は 11.16.0（`scripts/vendor-mermaid.sh:10`）。上流は
    先へ進んでいる。間にセキュリティ修正があるかは**未確認**
  - 対処案: SECURITY.md と private vulnerability reporting を有効化 / secret scanning を
    有効化 / dependabot（cargo と github-actions）/ cargo-deny を CI へ / ci.yml と
    fuzz.yml に `permissions: contents: read` / vendor 資産の定期確認
- ⬜ **tankan の修正（Phase 77）が crates.io に出ていない**（工数: 小）
  - gantt の `5日` での panic と state の循環は、公開中の tankan 0.2.0 には残っている
  - 対処案: publish-crate スキルでパッチ版（0.2.1）を出すか判断する

### 利用者の体験と docs

- ⬜ **build が壊れたページを黙って出す**（工数: 小〜中）
  - リンク切れ・画像切れ・存在しない見出しへのリンク・インクルードの失敗を含んでも、
    `yuzu build` は一部の警告だけで終了コード 0。`yuzu check` で初めてエラーとして出る。
    README の手順（dev → build → preview）では check は「CI 用」に見えるので実行されない
  - Mermaid の書き間違いは、既定の描画方式（client）では build も check も何も言わない。
    ssr にすると build は警告を出すが、**ページ名が無く**、行番号は図の中での行
  - 対処案: build の最後に「リンク切れ N 件。詳細は `yuzu check`」の 1 行を出す /
    SSR の警告にページ名を付ける / client 描画でも tankan のパースだけ通して構文エラーを
    check で報告するか（tankan 未対応の図種との切り分けが要る）
- ⬜ **docs と雛形の設定の書き方が TOML になっていない**（工数: 小）
  - v0.14 で設定を TOML に移した後も、JSON / YAML 風の書き方が残っており、
    コピペすると設定エラーになる: `guide/diagrams.md:11-12`（`backend: "client"`）/
    `guide/deploy.md:103`（`markdown.mermaid.backend: "client"`）/
    `guide/writing.md:217`（`markdown.crossref.numbering: "site"`）/
    `guide/writing.md:265`（`"abbr": false`）/ `guide/code-and-math.md:175`
    （`markdown.math.enabled: false`）/ `development/index.md:72` /
    雛形 `crates/yuzu-cli/scaffold/index.md:25`（`backend: "ssr"`）
  - 図の描画方式の既定値が食い違っている: 既定は client（`yuzu-config/src/schema.rs:283`。
    `guide/diagrams.md:11` も client）なのに、`guide/deploy.md:105` は「既定の `"ssr"`」
  - README の「図は 10 図種をビルド時に SVG 化」（`README.md:21`）は既定の設定では
    そうならない（mermaid.min.js をブラウザで読む）。「JS を使うのは検索とテーマ切替だけ」
    （`README.md:33`）も、数式は KaTeX がブラウザで描く（`base.jinja:124-125`）ので
    正しくない
  - `guide/index.md:49` の「`content/` と `theme/` を監視して」は、実際はプロジェクト
    ルート全体を監視している
  - 対処案: TOML の表記（`[markdown.mermaid]` + `backend = "ssr"`、または
    `markdown.mermaid.backend = "ssr"`）に揃える。docs ゲートに JSON 風表記の
    否定 grep を足すと再発を防げる
- ⬜ **frontmatter の型エラーが英語で、1 件ずつ、行番号がずれる**（工数: 中）
  - `order: "最初"` で `invalid type: string "最初", expected i64 at line 2 column 8`。
    `i64` が利用者に分からない。行番号は frontmatter の中での行で、ファイル上の行と
    ずれる。直すと次のエラーが出る（yuzu.toml のエラーは位置付き・日本語・全件で、
    落差が大きい）
  - 対処案: serde_yaml_ng のエラーを日本語に読み替え、行をファイル上の行へ足し直す。
    キーごとの型検査で全件出す（kabosu の decode と同じ考え方）
- ⬜ **手元の Markdown フォルダから始める方法が書かれていない**（工数: 小）
  - `yuzu new .` は「空ではありません」で止まり、`yuzu build` は「yuzu new で作成
    するか…」と案内する。どこにも書かれていない
  - 空の `yuzu.toml` 1 枚で動くのは、**原稿が `content/` 配下にある場合だけ**
    （`input.dir` の既定が `content`）。原稿がフォルダ直下にあると、build は成功するが
    原稿は 1 件も取り込まれず、HTML は `404.html` しか出ない（PR #23 のレビュー指摘。
    実機で確認）
  - フォルダ直下の原稿を `input.dir = "."` で読ませることはできない。出力先
    （`output.dir`）が原稿ディレクトリと重なるとして設定エラー（exit 2）になる。
    原稿を `content/`（または任意のサブフォルダ ＋ `input.dir` の指定）へ移す必要がある
  - 原稿が 0 件でも build は成功し、知らせるのは INFO の `pages=0` だけ（警告が無い）
  - 対処案: ガイドに「既存のフォルダで始める」節（原稿の置き場所の条件と、移す手順）/
    原稿が 0 件の build で警告を出す（`input.dir` の値と、直下に `.md` があればその旨）/
    `yuzu init`（yuzu.toml だけ作る）を足すかは判断
- ⬜ **dist をファイルとして直接開けないことが書かれていない**（工数: 小）
  - リンクがサイトのルートから始まる形（`/_assets/…`）なので、`index.html` を
    ダブルクリックで開くと CSS もナビも効かない。触れているのは `guide/search.md:119`
    （検索の `file://`）だけ。社内でフォルダごと渡す使い方ができるかが分からない
  - `--base-url ./` を黙って受け付け、`/./_assets/…` を出力する
  - 対処案: ガイドに明記する。相対パス出力に対応するかは別の判断（大）。
    `--base-url` に `.` 始まりを渡したら警告する
- ⬜ **雛形のナビが不自然**（工数: 小）
  - サイドバーに英小文字の「guide」が出る（`guide/index.md` が無いため）。ホームの
    「次のページ」が用語集で、本文の「次は はじめに へ」と食い違う
  - 対処案: 雛形に `guide/index.md` を足す。用語集の nav 上の順序を末尾に固定する
- ⬜ **検索で 2 文字の語に無関係な結果が混ざる**（工数: 小〜中）
  - `yuzu search "単語"` に「英語」を含む節が出る。誤字補正（1 文字違いまで許す）が
    2 文字の語にも効くため
  - 対処案: 語の長さで誤字補正を切る（3 文字以上だけ等）。mikan の変更なので
    native / wasm が同じ実装を通ることと、`FORMAT_VERSION` を上げずに済むかを確かめる
- ⬜ **細かな分かりにくさ**（工数: 小）
  - ビルドのログに UTC 時刻と `body_misses=4` のような開発者向けの値が出る。
    パイプ先にも色コードが混ざる。ページ数が build では 4・検索インデックスでは 3・
    check では 2 と食い違う。何も抑制していないのに「抑制 3 件」と出る（雛形の原稿の分）
  - `yuzu --help` が「README.md を参照」と書く（バイナリだけの利用者は持っていない）。
    「WS ライブリロード」「baseUrl」（設定名は `base_url`）、`--base-url` の説明の
    「CI から configure-pages の base_path を渡す用途」など、内部の言葉が出る
  - 雛形の原稿が機能カタログになっていて、自分の原稿を書き始めるには大半を消す必要がある
  - ガイドのフォルダ構成図（`guide/index.md:33-38`）に `snippets/` が無い。雛形の
    `theme/README.md` がリポジトリ内のパス（`crates/yuzu-theme/assets/`）を指している
    （バイナリの利用者は持っていない）

### 運用と持続性

- ⬜ **権限と知識が 1 人に集中している**（工数: 小）
  - org のメンバーも、tankan / mikan / kabosu の crates.io の owner も 1 人
    （見直し時に確認）。main にブランチ保護も ruleset も無い（`gh api` で確認）
  - 開発コンテナの任意の OTEL 設定（New Relic への送信）が個人の環境に固定されている。
    1Password の Vault 名・項目名を `scripts/dev-container.sh` に直書きしている。
    なお開発コンテナ自体は apple container 専用ではなく（`scripts/dev-container.sh` は
    docker にも対応し、`.devcontainer/devcontainer.json` にも Docker 経路がある）、
    キーが取れなければテレメトリを無効にして続行するので、他の人の開発を妨げはしない
    （PR #23 のレビュー指摘で記述を限定）
  - 対処案: crates.io に 2 人目の owner（GitHub team）/ main の保護（必須チェック
    `check` / `msrv`）/ OTEL 用の 1Password の項目名を環境変数で差し替え可能にする
- ⬜ **運用文書の古い記述**（工数: 小）
  - `.claude/skills/release/SKILL.md:95-98`: 見出しが「tankan / mikan」で kabosu が
    抜けている / 「実行回数はまだ 0 回」（実際は 3 crate とも公開済み）/ 参照先の
    CLAUDE.md「汎用ライブラリの crates.io 公開」の節がもう無い（publish-crate スキルへ
    移った）
  - 7 crate の Cargo.toml（yuzu-cli / core / render / config / index / server / theme）の
    「README ロードマップ参照」は、ロードマップが ROADMAP.md に分かれた後も残っている
  - tankan / mikan / kabosu の Cargo.toml・`fuzz.yml:5`・`ci.yml:46` は
    CLAUDE.md「リリース手順」を参照しているが、その節は今はスキルへの案内だけで
    1 段遠回り（壊れてはいない）
  - `crates/mikan/assets/model/README.md:20` が git 管理外の「設計ノート」を参照して
    いる（CLAUDE.md の「公開物から docs/design を参照しない」に反する）
  - 対処案: 直す。参照先の節・ファイルが実在するかを verify で grep する検査を足すと
    再発を防げる
- ⬜ **0.x の互換性方針・CHANGELOG・アップグレードガイドが無い**（工数: 小）
  - パッチ版の v0.10.1 に非互換の変更が入っていた。v0.14 の TOML 移行の手順は
    リリースノートにしかなく、docs に無い。リリースノートは内部実装の説明が多く、
    利用者に関係する変更が埋もれる
  - 対処案: docs に「互換性の方針と移行手順」1 ページ / リリースノートを
    「利用者への影響」と「内部」に分ける / CHANGELOG.md を置くかは判断
- ⬜ **ROADMAP.md の肥大化**（工数: 小）
  - 完了済みの内訳が全体の約 8 割を占める。候補（i18n・VS Code 拡張など）に
    「誰の何を解決するか」が書かれていない
  - 対処案: 完了済みの内訳を別ファイル（例 `docs/` の開発履歴ページ）へ移す。
    候補ごとに対象の利用者と解決することを 1 行書く
- ⬜ **CI の穴**（工数: 中）
  - テストは Linux だけ（`ci.yml` の `runs-on` は ubuntu-latest のみ）。配布している
    macOS・Windows のバイナリは、release.yml で `--version` を確かめるだけ。Windows での
    パスの正規化やリンク検査の不具合の有無は**未確認**
  - fuzz は手動起動のみで、対象は kabosu だけ。実行履歴は 09-06 の 2 回。Phase 77 の
    tankan の 2 件は、コーパスを変異させる簡単なハーネスで 90 秒以内に見つかった
  - `ci.yml` の test / build に `--locked` が無い
  - 公開リポジトリの定期実行（docs-links.yml）は、60 日間動きが無いと GitHub が止める
  - 対処案: windows / macos の `cargo test` を matrix に / tankan の `render_svg` を
    fuzz 対象に足し、月次の定期実行 / `--locked` を足す
- ⬜ **macOS バイナリが未署名・未公証**（工数: 説明だけなら小・署名するなら中）
  - ブラウザでダウンロードすると Gatekeeper に止められる可能性がある（**未確認**）。
    docs に回避方法の説明が無い
- ⬜ **外部への露出が無い**（工数: 中）
  - repo の description・topics・homepage が空、Discussions 無効、star 0・fork 0
    （`gh repo view` で確認）。README・docs・ライブラリの README が日本語のみ。
    mdBook / VitePress / MkDocs 等との比較が無い。インストールは手動配置か
    `cargo install --git` だけ（Homebrew・cargo-binstall のメタデータ・install
    スクリプトが無い）
  - 対処案: 想定する利用者（例「日本語で設計書を書くチーム」）と他ツールとの比較を
    1 ページに / 英語の README を最低限 / topics・description / cargo-binstall の
    メタデータ。品質側の指摘（上の「配布物」「利用者の体験」）を先に片付けてから
- ⬜ **外部からの貢献の受け口が無い**（工数: 小）
  - CONTRIBUTING・CODE_OF_CONDUCT・issue / PR テンプレートが無く、ラベルも既定のまま。
    貢献を受けるかどうかの表明が無い
- ⬜ **`.claude/settings.local.json` が .gitignore に入っていない**（工数: 小）
  - 未追跡のまま置かれていて、誤ってコミットする余地がある

### 急がないコード整理

- ⬜ **長い関数**（工数: 中）
  - `crates/yuzu-render/src/pipeline.rs:88` の `render_site`（約 335 行）/
    `crates/tankan/src/sequence/layout.rs` のレイアウト関数（約 430 行）/
    `crates/yuzu-index/src/builder.rs` のビルド関数（約 220 行）/
    `crates/yuzu-core/src/suppress.rs` の抑制の適用（約 220 行）。
    `crates/yuzu-core/src/markdown/mod.rs` は 1094 行
  - 対処案: `render_site` はページ単位の処理と集約を別関数に。`markdown/mod.rs` を
    分けるなら、「comrak を触るのは mod.rs だけ」の規則をディレクトリ単位に
    定義し直す必要がある
- ⬜ **同じ処理が 2 か所にある**（工数: 小）
  - `escape_html` が `yuzu-core/src/markdown/mod.rs:22` と
    `yuzu-render/src/highlight.rs:438` に同じ中身で 2 つ
  - mermaid / math の言語の集合は `yuzu-core/src/lib.rs:143` の
    `is_special_render_lang` と `yuzu-render/src/highlight.rs:324-333` の分岐を
    コメントで同期させているだけ（openapi / jsonschema は `SPEC_LANGS` とテストで縛って
    いるのと非対称）
  - 対処案: `escape_html` を core に 1 つ / 言語を enum にして網羅 match で縛る
- ⬜ **依存とビルド設定**（工数: 小）
  - sha2 が 0.10.9 と 0.11.0 の 2 版入っている（Cargo.lock）
  - axum を既定の features のまま使っており、使っていない json / form / query が入る
  - `yuzu-index` は rust-embed（`assets/search/`）を使うのに build.rs が無い（CLAUDE.md
    の罠に記載済み。新規ファイルを足すと release が古い埋め込みを使う恐れ）
- ⬜ **性能の気になる点**（工数: 小〜中。**どちらも未計測**）
  - `yuzu-render/src/context.rs` がページごとに nav 全体の URL を作り直している
  - `yuzu-cli/src/commands/build.rs` の git 連携メタが、watch の再ビルドのたびに全履歴を
    `git log` している

## v0.19 以降の候補

### dogfooding 候補（v0.13 Phase 61 からの持ち越し）

- ~~**OS ダーク追従** / **見出しパーマリンクのキーボード到達性** / **`<head>` メタ** /
  **ページメタの拡充（読了時間・文字数）**~~ — 4 件とも v0.18 の Phase 76・78・79 へ移した
  （上の「現在」を参照。実測と判断点もそちらに移設）

### その他の候補

- **i18n** — テーマ UI 文字列の多言語化
  - 規模（実測）: jinja 18 ＋ テーマ JS 19 ＋ apispec 35 ＋ crossref 3 文字列。
    `site.lang` は `<html lang>` の 2 箇所でしか使われていない
  - 半端になる懸念: **検索（vaporetto の分かち書き）と `lint.rules`（全角英数・
    半角カナ・長音符）は日本語固有**なので、UI だけ多言語化しても中途半端
  - 代替にならない: テーマ上書き（`theme/templates/`）で文言は今でも変えられるが、
    粒度がファイル単位でアップストリームから fork することになる
    （`search-ui.js` は 470 行）
  - 最小案: `theme.strings` の部分上書き辞書（`theme.css_vars` / `glossary.terms` と
    同型）。既定を日本語のまま据え置けばスナップショットは動かない
  - 線引きが要る: apispec の文言は**描画のエラーボックスと `yuzu check` の診断で共有**
    しているので、翻訳すると `--format json` の出力も言語で変わる
- **ドキュメントバージョニング** — 要否含め保留中
- **VS Code 拡張** — wasm プレビュー
  - 前提: `yuzu-core` / `yuzu-render` が 9 ファイルで `std::fs` に依存しており
    I/O 抽象化が要る
- **yuzu 本体の crates.io 公開** — 汎用ライブラリ層は tankan・mikan まで公開済み
  - 障害: 名前 `yuzu`・`yuzu-core` が別プロジェクトに取得済み
  - 単一パッケージ化するか名称を再検討する必要がある（Phase 37 の決定事項）

## これまでのリリース

- **v0.1**（Phase 1〜6）
  - build / dev サーバ / 日本語検索 / llms.txt / tankan SSR / fmt・lint・check
- **v0.2**（Phase 7〜12）
  - 執筆表現 / 数式 / ページナビ / 検索のセクション単位化
  - デプロイ雛形 / インクリメンタルビルド
- **v0.3**（Phase 13〜18）
  - 執筆の即効改善 / ページ Markdown 配信とコピー / 用語統一 lint
  - tankan class・pie / git 連携メタ / dogfooding 改善
- **v0.4**（Phase 19〜23）
  - 表記ゆれの組み込み lint / 検索の同義語・タイポ改善
  - OpenAPI・JSON Schema SSR / flowchart スタイル構文
  - **v0.4.1**: content 同伴アセットの自動コピー
- **v0.5**（Phase 24〜29）
  - tankan スタイル構文の全図種展開 / コードブロックの opt-in 索引
  - OpenAPI Swagger 2.0・スキーマ一覧 / tankan mindmap・timeline
  - 形態素トークナイザ PoC は実測の結果見送り
  - dogfooding 改善 — 404 ページと `lint --fix`
- **v0.6**（Phase 30〜35）
  - 検索インデックスの位置情報化（フォーマット v3）/ フレーズ検索
  - ビルドのページ並列化（render・index）
  - 検索スタックのライブラリ化と OPFS キャッシュ
  - dogfooding 改善 — 近接ブースト・フレーズヒント・ビルド時間表示
- **v0.7**（Phase 36〜38）公開・配布の整備
  - [ドキュメントサイト](https://ai.implementer.net/yuzu/)を GitHub Pages へ公開
  - tag push で 4 プラットフォームのバイナリを配布する release.yml
  - [tankan の crates.io 単独公開](https://crates.io/crates/tankan)
  - 名前 `yuzu`・`yuzu-core` の取得済み判明により、本体の crates.io 公開は将来構想へ再定義
- **v0.8**（Phase 39〜41）執筆機能の拡充
  - コードブロックの表示メタ（title / 行ハイライト / 行番号。JS ゼロ維持）
  - リダイレクト・エイリアス
  - dogfooding 改善 — エイリアス診断の行番号・コードメタ lint・sitemap.xml・
    `git.lastUpdated` のサブディレクトリ運用バグ修正
- **v0.9**（Phase 42〜45）執筆機能の拡充 第 2 弾
  - コンテンツインクルード（`file=`）/ 図表番号と相互参照 / 折りたたみ（`> [!NOTE]-`）
  - dogfooding 改善 — 折りたたみの自動展開・fmt の独自記法温存・
    図表番号のサイト全体通し番号
  - **v0.9.1**: サイドバーのスクロール位置維持
- **v0.10**（Phase 46〜49）実運用の質を上げる
  - 診断の機械可読出力（`--format {human,json,github}`）
  - 検証の網羅性 — API 仕様の `file:` 参照・`yuzu.jsonc` のキー診断
  - watch・キャッシュの正しさ
  - dogfooding 改善 — 検索の追加読み込み・`yuzu fmt --diff`・scaffold 刷新・SIGPIPE 対応
  - **v0.10.1**: 外部コードレビュー指摘の修正 — 出力先の境界検証・ページ URL の検証・
    エイリアス `.` の拒否・ハイライト無効時のインクルード欠落修正・走査エラーの伝播・
    URL エスケープ・vendor 取得のバージョンとアーカイブのチェックサム固定
    - **非互換**: `output.dir` がルート外・ルート自身・`input.dir` / `public/` /
      `theme/` / `.yuzu` と重なる場合はエラー / ルートから出力先（と `.yuzu`）までの
      経路にシンボリックリンクがあればエラー / `x.md` と `x/index.md` の共存・
      エイリアス `"."`・ファイル名の URL 危険文字（`#` `?` `%` `"` 等）もエラー
- **v0.11**（Phase 50〜53）執筆機能の拡充 第 3 弾
  - タブ / コードグループ（JS ゼロ）
  - Markdown 断片のインクルード（` ```include `）
  - 用語集・略語 — 設定の辞書から `<abbr>` 化とページ自動生成
  - dogfooding 改善 — 約物に隣接した強調・定義リスト・検索結果のセクション絞り込み
    （エンジン側）・ポート衝突の案内と `build --watch` のポート指定・
    キャッシュ保存の原子化
- **v0.12**（Phase 54〜57）読む体験の完成
  - 全文検索の結果専用ページ — `?q=` / `?section=` を URL で共有。
    ドロップダウンはサジェストへ格下げ
  - 印刷・PDF 対応 — 画面 UI 非表示・常にライト配色・折りたたみとタブの全展開・thead 再掲
  - ナビと目次の規模対応 — サイドバー折りたたみ・入れ子 TOC・`theme.toc.levels`・
    scrollspy の基準線修正
  - dogfooding 改善 — サイト URL 更新
- **v0.13**（Phase 58〜61）lint の制御性
  - ページ単位の抑制（frontmatter `lintDisable`）
  - 行単位の抑制（`<!-- yuzu-lint-disable-next-line -->` コメント）
  - `lint.rules` の「ルール ID → bool」化による全ルールの enable/disable
  - dogfooding 改善 — 抑制記法を docs・scaffold で実運用・SSR 図のモバイル対応
  - Phase 外 — ビルド進捗ログ（処理中ページ・watch の変更ファイル表示）と
    comrak 整形パニックの防御（該当ページを原文へ縮退）
- **v0.14**（Phase 62〜63）設定基盤の刷新 = TOML 化
  - 依存ゼロ・`no_std + alloc` の TOML ライブラリ **kabosu** を新設（設計は
    [docs/content/development/kabosu.md](docs/content/development/kabosu.md)。
    [crates.io で単独公開](https://crates.io/crates/kabosu)）
  - 設定を `yuzu.jsonc`（JSONC）から `yuzu.toml`（snake_case キー）へ全面移行
  - **非互換**: JSONC の互換読み込み・変換コマンドは無し / 未知キー・型違い・重複キーは
    設定エラー（exit 2）で停止 / `config-unknown-key`・`config-duplicate-key` ルールは
    廃止 / `.yuzu/settings.json` は廃止 / envKey が変わるため移行後の初回ビルドは
    フルビルド
- **v0.15**（Phase 64〜67）正しさ・堅牢性
  - URL のパーセントエンコード — route → URL の変換点を 1 つに決め、非 ASCII も含めて
    本文・ナビ・llms・sitemap・検索索引で同じ表記。著者のエンコード済み参照と
    aliases はデコードして照合
  - 配信のシンボリックリンク遮断と `syntect.css` の条件出力 — テーマ上書きは
    デフォルトテーマの変更へ追随する契約を明文化
  - 外部リンク切れ検査の opt-in（`yuzu check --external-links`）— HTTP は curl へ委譲し、
    4xx だけ warning・環境要因は `summary.skipped` へ
  - dogfooding 改善 — docs の外部リンク検査を週次実行・preview のリンク遮断 e2e
  - **非互換**: `unsafe-page-path` はファイル名では `\` と制御文字だけに縮小
    （`#` `?` `%` 等を含むファイル名が受理される）/ 一方
    `markdown.glossary.page`・`search.page`・`aliases` は Windows 予約文字を全 OS で拒否 /
    非 ASCII を含む URL がパーセントエンコード形になる（本文リンクは従来どおり）/
    `highlight.enabled = false` で `syntect.css` を出力しない（`base.jinja` を
    上書きしている利用者は追随が要る）/ preview・dev がシンボリックリンクを辿らない
- **v0.16**（Phase 68〜71）kabosu の TOML 1.0 完全対応
  - 未対応だった 6 構文（float / date-time / 16,8,2 進整数 / 複数行文字列 /
    インラインテーブル / テーブルの配列）を実装
  - 公式 [toml-test](https://github.com/toml-lang/toml-test) の TOML 1.0.0 対象ケース
    （valid 205 / invalid 474）を全通過して
    [kabosu 0.2.0](https://crates.io/crates/kabosu) を公開
  - TOML 1.1 でだけ妥当な記法（`\e` / `\xHH`・インラインテーブルの改行と末尾カンマ・
    秒を省略した時刻）は `Unsupported(TomlV11)` として「1.0 には無い記法」と案内する
  - **yuzu 本体の機能追加は無し** — `yuzu.toml` に書ける構文が増えるだけ
    （インラインテーブル・テーブルの配列・日時・小数・複数行文字列が
    「未対応の構文」エラーにならなくなった）
- **v0.17**（Phase 72〜75）CLI の使い心地
  - グローバル引数 `--root <DIR>`（指定時は上方向探索をしない）と実行文脈 `Cx` の集約
  - `-q` / `-v`（`RUST_LOG` より優先。`-q` は warn を残す）と
    `search --format {human,json}`（`--json` は非表示の互換エイリアス）
  - `yuzu completions <shell>` — bash / zsh / fish / powershell / elvish の補完スクリプトを
    clap の定義から実行時に生成（同梱しない・動的補完はしない）
  - dogfooding — docs のビルド・検証を `--root docs` へ統一、scaffold の deploy.yml に
    サブディレクトリ運用の案内
  - **記法・テーマ・レンダリング結果は変えていない**（`CACHE_FORMAT_VERSION` の bump 無し）
  - Phase 外の修正 — stderr への書き込みに失敗するとビルドが途中で落ちていた
    （`yuzu build 2>&1 | head` で再現。`--force` なら `_search` が消えたまま残る）

検索エンジン本体 **mikan**（旧 yuzu-index-format）と wasm ラッパ **mikan-wasm**
（旧 yuzu-search-wasm）は v0.7 リリース後に yuzu- プレフィックスを外して改名し、
mikan は crates.io で単独公開している（tankan と同じく独立バージョン）。

各版の Phase 内訳:

<details>
<summary>完了済み: v0.17（Phase 72〜75）の内訳</summary>

軸は「**CLI の使い心地**」。v0.16 は kabosu の内部完成で利用者に見える変化がゼロだった。
CLI は v0.10 の `--format` 以来まとまった手入れがなく、オプションが増えるたびに 9 つの
`run()` を個別に触る構造のまま伸びていた。実行文脈（プロジェクトルート・出力形式・
静粛さ）を CLI の一段目で決めて各コマンドへ渡す形にし、`--root` と shell 補完を入れた。
記法・テーマ・レンダリング結果は変えていないので `CACHE_FORMAT_VERSION` の bump は無い。
Phase は基盤 → 一貫性 → 追加機能 → dogfooding の依存順で、各 Phase は着手時に判断点を
決めてから実装し、レビュー指摘（計 2 件）と CI で発覚した既存の不具合（1 件）を
同じ PR で取り込んだ。

- **72 グローバルオプションの基盤（`--root` と実行文脈の集約）** — 着手時の実測で
  `Cli` にグローバル引数が 1 つも無く、`run()` 9 箇所が位置引数を個別に取り、
  `MarkdownOptions` の構築が 8 箇所にコピーされていた。`--root <DIR>` を `GlobalArgs`
  （clap の `global = true` でサブコマンドの前後どちらでも書ける）に足し、実行文脈を
  `cx.rs` の `Cx` にまとめて各 `run()` の第 1 引数へ渡す。**`--root` 指定時は上方向探索を
  しない**（探索すると「指定したのに親の `yuzu.toml` を拾う」事故になる）。
  **`--root` は受け口 `Cx::new` で 1 回 `canonicalize`**（相対のまま流すと診断のパス表示まで
  相対で伝播し、ルート自身がシンボリックリンクだと `.yuzu` のリンク検査を通る build・dev
  だけが落ちる）。`.yuzu` のリンク検査の非対称は揃えない（書き込む側の事前条件で、
  読み取りコマンドまで落とすのは過剰）。指定先に `yuzu.toml` が無いときの
  `ConfigError::ConfigFileNotFound` は `ProjectRootNotFound` と別物で、文言に
  「上方向に探索」を含めない。`yuzu new --root` は明示エラー（既存プロジェクトを読まない
  唯一のコマンドで、黙殺しない）。`MarkdownOptions` の 8 箇所は
  `yuzu_render::markdown_options` へ集約した
- **73 出力の一貫性（`--format` と静粛モード）** — `--format` は lint と check だけ、
  `search` は `--json` の bool、ログ制御は `RUST_LOG` のみで v0.13 のビルド進捗ログを
  CLI から黙らせられなかった。`-q` / `-v` をグローバル引数に足し、引数のパースをログ
  初期化より先に動かして、フィルタの決定を `main.rs` の `log_filter` に集約。
  **CLI フラグは `RUST_LOG` より優先**（シェルに残った環境変数に黙って負けると
  「`-q` を付けたのに進捗が出る」になる）。**`-q` は info 以下だけ黙らせ warn は残す**
  （yuzu.toml の注意・ハイライト失敗・mermaid の構文エラーは黙らせると気づけない。
  標準出力の契約物と `fmt --check` の集計行は変えない）。`search --format {human,json}`
  を足し、**`--json` は `hide = true` の互換エイリアス**（`--format` との同時指定は矛盾
  エラー。優先順位を決めない）。`build --format` は広げない。検索の `Format` は診断の
  `diag::Format` と別 enum（検索に `github` は無い）。`GlobalArgs` に `next_display_order`
  を付けてヘルプで固有の引数の後ろへまとめた。**CI で既存の不具合が発覚**:
  `tracing_subscriber::fmt()` の `log_internal_errors` 既定 true が、stderr への書き込み
  失敗時に同じ stderr へ `eprintln!` して panic していた。`yuzu build 2>&1 | head` で
  再現し、`--force` は dist を作り直すので `_search` が消えたまま残る。
  `log_internal_errors(false)` で「書けなければ捨てる」（stdout の `out.rs` と同じ規律）。
  レビュー指摘 1 件: `conflicts_with` はサブコマンドの境界をまたぐ global 引数を検証しない
  （`yuzu -q build -v` が通る）ため、`Cli::parse_validated` のパース後検証で同じ clap
  エラー（終了コード 2）にした
- **74 shell 補完** — `yuzu completions <bash|zsh|fish|powershell|elvish>` を追加。
  判断は 3 点ともユーザ確認のうえ: **clap_complete を依存に入れる**（clap と同じ
  リポジトリの公式クレートで、default features の依存は clap だけ・MSRV 1.85 で workspace
  と同じ。凍結した設計判断は「clap derive ＋ clap_complete」に広げた）/ **実行時生成だけ**
  （リポジトリにもリリースアセットにも同梱しない。同梱するとオプション変更のたびに
  再生成が要り、クロスビルドのターゲットはランナーで実行できないので生成元も分かれる）/
  **動的補完はしない**（`unstable-dynamic` で API が安定しておらず、静的補完で足りないのは
  `--section` のセクション名と検索クエリだけ）。clap の定義 `Cli::command()` から生成する
  ので、単体テストは全シェルの出力にグローバル引数（`--quiet`）が入ることを見る
  （= 手書きではなく定義から生成している証拠）。プロジェクトを読まないので `--root` は
  `new` と同じ規律で明示エラー。docs ゲートはコードブロック内の文字列が syntect の span で
  分断されるため、見出し id で照合する
- **75 dogfooding** — docs のビルド・検証を `--root docs` に統一し、`cd docs` /
  `working-directory: docs` を ci.yml・docs.yml・docs-links.yml・`verify` スキル・
  docs の例から無くした（出力の参照は `docs/dist/`）。**両方通す形にはしない**
  （「どちらが正か」が曖昧になる。cwd からの上方向探索の経路は scaffold の e2e が
  `cd` して検証する）。`-q` は CI に付けない（進捗ログは落ちたときの切り分けに使う）。
  scaffold の deploy.yml には check ステップを足さず（利用者のデプロイが lint 違反で
  止まる挙動変更になる）、既存リポジトリの `docs/` へ生成した場合の案内
  （リポジトリルートの `.github/workflows/` へ移して `--root docs`）をコメントで足した。
  e2e に「`--root` 指定で cwd がプロジェクト外でも github 形式の注釈パスが
  `GITHUB_WORKSPACE` 相対になる」検査を追加（レビュー指摘 1 件: `cd` 後で cwd が
  プロジェクト内のままだったので、サブシェルで親へ移動してから相対 `--root` で実行する）

**教訓**: clap の `conflicts_with` はトップレベルとサブコマンドを別々に検証するので、
global 引数同士の排他は後検証が要る。tracing-subscriber の `fmt()` は既定で書き込み失敗を
stderr へ再報告するので、stderr が閉じた瞬間に panic する。どちらも「ライブラリの既定が
CLI の規約（終了コード・書けなければ捨てる）と食い違う」型の罠で、CI の e2e が最初の一致で
読み手を閉じる `grep -q` を使っていたことで表面化した。

**非互換**は無い。`search --json` は非表示の互換エイリアスとして残り、`RUST_LOG` も
従来どおり効く（`-q` / `-v` を付けたときだけ CLI フラグが優先する）。

</details>

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
