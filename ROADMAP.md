# ロードマップ

yuzu の開発計画と、これまでのリリースの内訳。**このファイルが Phase 状態の正**
（README には現在の版と概要だけを置く）。

## 現在: v0.18.1（パッチ）→ v0.19（Phase 81〜86）

**v0.18 まで公開済み**（kabosu 0.2.0 / tankan 0.2.0 / mikan 0.2.0 も crates.io で
公開済み。yuzu のリリースとは非同期。kabosu の publish 前に fuzz を回す規律は
CLAUDE.md にある。v0.18 の Phase 77 で直した tankan の 2 件は、Phase 81 で公開版に
入れる）。

次は 2 段階で進める。先に **v0.18.1**（パッチ）で、同梱している mermaid・KaTeX の
脆弱性の修正と、工数が小さく黙って誤った結果を出す不具合 4 件を出す。その後に
**v0.19**（テーマ「配布物と第一印象」）を 6 つの Phase で進める。この順序は
10-07 の見直し（下の「[10-07 見直し](#10-07-見直し)」）で 3 者が一致した結論。

### v0.18.1（パッチ） ⬜

**概要**: 同梱の mermaid と KaTeX を、公開済みの脆弱性勧告が直った版へ上げる。
あわせて、10-07 の見直しで見つかった「黙って誤った結果を出す」不具合のうち、
工数が小さい 4 件を直す。本文 HTML の形式・`CACHE_FORMAT_VERSION`・検索の
`FORMAT_VERSION` は変えない。

- 現状（実測。10-07 時点）
  - mermaid 11.16.0 に 5 件の勧告が該当する（08-06 公開。radar と XY Chart の DoS、
    architecture 図と設定 API のプロトタイプ汚染、図の兄弟要素へ及ぶ CSS 注入。
    すべて 11.16.1 で修正）。上流の最新は 12.1.0
  - KaTeX 0.17.0 に 1 件（GHSA-238p-pmpm-9mq7・low・10-05 公開。既存のプロトタイプ
    汚染があると `trust` の制限を回避できる。0.18.2 で修正）。上流の最新は 0.19.0
  - 図と数式を書くのはサイトの執筆者で、閲覧者の入力ではない。ただし執筆者が複数いる
    サイトや、外部の Markdown を取り込むサイトでは CSS 注入が問題になりうる
  - B1（10-07 見直しの番号。以下同じ）: `crossref.numbering = "site"` の本文キャッシュが、
    他ページの `order:` の変更や、図と表の差し替え（個数が同じ）で無効化されない。
    routesKey は rel・route・ラベルの個数だけ（`commands/build.rs:369`）なのに、採番の
    開始位置は nav の表示順で積み上げる（`yuzu-core/src/lib.rs:267` の
    `assign_crossref_offsets`。番号は種類別）。`incremental_test.rs` は routesKey を
    自前で組み立てていて、ラベル数も含めていない。`numbering = "site"` のキャッシュの
    テストは無い
  - B2: プロジェクトの祖先に `.` 始まりのディレクトリ（`~/.config/notes/` や
    `.claude/worktrees/…`）があると、dev / `build --watch` が全イベントを捨てて黙って
    再ビルドしなくなる。`WatchIgnore::is_ignored`（`yuzu-server/src/watch.rs:103`）が
    notify から届く絶対パスの全構成要素を見るため。テスト用の `visible_tempdir` は
    これをテストでだけ回避している。ci.yml の watch の e2e は「編集しないと再ビルド
    しない」側しか見ていない
  - A4: 通常の `yuzu build` でも `__yuzu/build_id`（ナノ秒の時刻）を書く
    （`yuzu-render/src/pipeline.rs:426`）。読むのは `--watch` 時だけ読み込まれる
    autorefresh.js（`base.jinja:128`）。dist が毎回 1 ファイル変わり、ビルド時刻も公開される
  - B7: 外部リンク検査の curl（`commands/extlink.rs:157`）の引数の先頭に `-q` が無く、
    利用者の `~/.curlrc` を読む。`--fail` が書いてあると 4xx で終了コード 22 になり、
    壊れたリンクが skipped として通る。リダイレクト先のプロトコル制限も無い
- やること
  - mermaid を 11.16.1 以降の 11 系へ、KaTeX を 0.18.2 以降の 0.18 系へ上げる
    （vendor-update スキル）。12.x / 0.19 への移行は v0.20 以降で、tankan の互換対象の
    Mermaid の版とあわせて決める
  - B1: routesKey にページの nav 上の順位と、ラベルの種類別の個数を入れる。キーの組み立てを
    core の関数にまとめ、cli と `incremental_test.rs` が同じ関数を通るようにし、
    `numbering = "site"` のキャッシュのテストを足す（キーの中身が変わるので、本文キャッシュは
    初回の build で 1 回だけ全部作り直される）
  - B2: watch の隠しディレクトリの判定を、監視ルートからの相対パスで行う。CLAUDE.md の
    「`tempfile::tempdir()` は監視が常に無視する」の罠も書き直す
  - A4: `__yuzu/build_id` を書くのは `--watch`（poll 方式のライブリロード）のときだけにする。
    既存サイトの dist に残っている分は孤児掃除で消える
  - B7: 外部リンク検査の curl の引数の先頭に `-q`、加えて
    `--proto =http,https --proto-redir =http,https`
- 確かめること: 数式のページ（docs）と、client 描画の図のページ（雛形）をブラウザで見比べ、
  描画が変わっていないこと
- リリースの手順は release スキルのパッチ版の節のとおり。リリースしたらこの節を削り、
  「これまでのリリース」の v0.18 の行に 1 文足す（パッチ版は内訳表を作らない）

### v0.19 の方針

テーマは「**配布物と第一印象**」。10-04 見直しの持ち越しのうち「配布物として必要な
もの」と、「利用者の体験と docs」のうち初めて使う人が最初に踏む食い違いに、
10-07 の見直しで見つかった「公開してはいけないものが外へ出る」3 件を加え、
6 つの Phase で消化する。本文 HTML とキャッシュ形式は変えないので
`CACHE_FORMAT_VERSION` の bump は無い。

以下は策定の詳細。

- 動機: 10-04 の見直しで 3 人が合意した優先順位の先頭が「配布物として必要なもの」で、
  どれも工数は小〜中。10-07 の見直しでは、想定する利用者（社内の設計書を書く人）に対して
  原稿の `.md` が必ず配信される・公開範囲の注意が無い・dev サーバが外部サイトから
  読まれうる、の 3 件が見つかり、3 者とも「第一印象」より先に扱うべきと判断した。
  配布物だけでは利用者に見える変化がほとんど無いので、docs の書き方の誤り・
  build が壊れたページを黙って出す件・既存フォルダからの始め方・雛形のナビも同じ版に入れる
- ゴール: 配布物（リリースのアーカイブ・利用者のサイトの dist・crates.io の 3 crate）に
  第三者ライセンスの表記があり、脆弱性の報告先と依存の監視がある。公開したくない原稿を
  出さずに済む設定と、社内で公開するときの注意が docs にある。README・docs・雛形の
  記述が実際の動作と一致し、build は壊れたページを出したときに件数と次の手順を知らせる
- 本文 HTML・キャッシュ形式・検索インデックスの形式は変えない。変わるのは dist に
  出るファイル（ライセンス文・`.md` の配信の有無）・nav（雛形と用語集の位置）・
  dev / preview サーバの応答・CLI の出力・docs・CI
- Phase は「配布物（81 ライセンス → 82 脆弱性の窓口と依存の監視）→ 83 公開範囲の制御 →
  利用者の体験（84 docs と雛形 → 85 build の報告）→ 86 dogfooding」の順。
  82 の cargo-deny の licenses 検査は 81 の一覧と許可リストを揃えるので 81 の後。
  83 は 84 の docs（社内で公開する節）が前提にするので 84 の前。
  着手時に判断点を決めてから実装する
- **リリース判定**（すべて満たしたらリリースする）
  - cargo-deny（検査の範囲は Phase 82 で決める）が CI で成功している
  - release.yml がアーカイブ内のライセンス一覧の有無を検証している
  - ci.yml の e2e が、dist のライセンス文・vendor の README が出ないこと・`.md` の配信の
    設定を照合している
  - dev / preview が許可していない Host に 403 を返すテストがある
  - docs ゲートの JSON 風表記の否定 grep が通っている
  - 仮決めのままの判断点が無い（残すならリリースノートに書く）
- **ユーザの作業**

  | 作業 | Phase | 時期 |
  | --- | --- | --- |
  | private vulnerability reporting・secret scanning・push protection を有効にする | 82 | 82 のマージ前 |
  | tankan 0.2.1 を crates.io へ publish する（`cargo login` が要る） | 81 | 81 のマージ後 |
  | 各 Phase の判断点を決める | 81〜86 | 着手時 |
  | ブラウザでの確認（Host の検査・雛形 deploy.yml の実行・ライセンス文の表示） | 86 | リリース前 |

- **時間が足りなければ v0.20 へ回すもの**: 82 の vendor 資産の勧告の定期照合 /
  84 の docs をタグで公開する変更 / 85 の client 描画の Mermaid 構文チェックと、
  check の整形差分を外す設定 / `yuzu init`

### 81 第三者ライセンスの表記 ⬜

**概要**: リリースのアーカイブ・利用者のサイトの dist・crates.io の 3 crate に、
同梱している第三者のライセンス表記を入れる。あわせて、テーマ資産として配信されて
いる vendor の更新メモ（`_assets/vendor/README.md`）を配信対象から外す。
主な判断点は一覧の生成方法（ツールと生成のタイミング）と、dist に何を出すか。

- 現状（実測。10-06 時点）
  - アーカイブに入れているのは `README.md` と yuzu 自身の `LICENSE-MIT` /
    `LICENSE-APACHE` だけ（`release.yml:142`）
  - バイナリに入る第三者 crate は約 190（Linux 向けの通常依存。proc-macro を含む。
    `cargo tree -e normal`）。大半は MIT / Apache-2.0 で、ほかに BSD-2-Clause
    （comrak）・MIT AND BSD-3-Clause（matchit）・Unicode-3.0（unicode-ident）・
    Unicode-DFS-2016（finl_unicode）・Zlib（foldhash）・ISC（inotify）・
    CC0-1.0（notify）がある。MIT や BSD のものは、バイナリで配るときに著作権表示と
    許諾文を添える必要がある
  - two-face（構文定義・テーマ）は表示用の `acknowledgement::listing()` を feature なしで
    公開している。yuzu は使っていない
  - dist へ配る vendor 資産: mermaid.min.js（MIT）・KaTeX の JS・CSS・woff2 フォント
    20 個（MIT）・分かち書きモデル `_search/model.zst`（MIT OR Apache-2.0）。
    mermaid.min.js と KaTeX の min ファイルの先頭にライセンスのコメントは無い
  - mermaid.min.js は複数の OSS を束ねたもので、「MIT 1 行」では済まない。中に
    `@license DOMPurify 3.4.0 | (c) Cure53 … Released under the Apache…` の表記があり
    （10-07 に確認）、D3・lodash・dagre・cytoscape の文字列も入っている（それぞれの
    ライセンスは未確認）
  - tankan の Cargo.toml に `exclude` が無く（kabosu は toml-test を除外している。
    10-07 に確認）、`tests/corpus` の .mmd が crate に入る。corpus には Mermaid 公式の
    例文の写しに見えるものがあり、出所の記録が無い
  - テーマ資産は `yuzu_theme::iter()` の全ファイルを `_assets/` へ書き出す
    （`yuzu-render/src/assets.rs:47`）ので、`static/vendor/README.md` も
    `_assets/vendor/README.md` に出る（このサイトでも
    `https://ai.implementer.net/yuzu/_assets/vendor/README.md` が 200 を返す）。
    検索資産（`yuzu-index`）はファイル名を列挙してコピーしているので README は出ない
  - 3 crate（tankan / mikan / kabosu）とも `cargo package --list` に LICENSE が 0 件
    （`license.workspace = true` だけで、ファイルはワークスペース直下にしか無い）
- やること
  - 第三者 crate のライセンス一覧（著作権表示と許諾文）を作り、アーカイブに入れる
    （ファイル名は仮に `THIRD-PARTY-LICENSES`）。two-face の acknowledgement も含める
  - dist に vendor 資産のライセンス文を出す。書き込みは `write_under` を通し、
    出力マニフェストに載せる。mermaid に束ねられた各 OSS のライセンス文は、
    `scripts/vendor-mermaid.sh` で mermaid と一緒に取得する
  - tankan の corpus は `exclude` で crate から外すか、出所を書いた README を corpus に置く
  - `static/vendor/README.md` を rust-embed の対象から外す（`#[exclude]`）。既存サイトの
    dist に残っている分は、次の build の孤児掃除で消える
  - 3 crate のディレクトリに LICENSE-MIT / LICENSE-APACHE を置く（シンボリックリンクを
    `cargo package` が実体で入れるかを確かめ、だめならコピー）
  - tankan は Phase 77 の修正 2 件と LICENSE を入れて 0.2.1 を公開する
    （publish-crate スキル）
- 判断点
  - **一覧の生成方法** — cargo-about・cargo-deny の `list`・自前スクリプトのどれを
    使うか。生成のタイミングは (a) release.yml でリリースのたびに生成する
    （CI にツールのインストールが要る）か、(b) リポジトリにコミットして CI で
    最新かを照合する（依存の更新ごとに差分が PR に出る）か。どちらも build の既定経路に
    ネットワーク I/O を入れない規律とは衝突しない
  - **dist に何を出すか** — vendor 資産のライセンス文だけか、yuzu 自身と第三者 crate の
    一覧まで出すか（利用者のサイトに出るのは vendor の JS・CSS・フォント・モデルなので、
    前者で足りる見込み）。テキストファイルにするか HTML のページにするか
  - mikan / kabosu も LICENSE だけのためにパッチ版を出すか（mikan は分かち書きモデルを
    同梱している）

### 82 脆弱性の窓口と依存の監視 ⬜

**概要**: 脆弱性の報告先（SECURITY.md と GitHub の private vulnerability reporting）を
用意し、依存の監視（dependabot・cargo-deny）と CI の権限の絞り込みを入れる。
mermaid・KaTeX の更新は v0.18.1 へ移した。主な判断点は cargo-deny の検査範囲と、
dependabot の運用（頻度・まとめ方・対象から外す依存）。

- 現状（実測。10-06 時点）
  - `SECURITY.md` / `.github/dependabot.yml` / `deny.toml` が無い。
    `security_and_analysis`（secret scanning・push protection・dependabot の
    セキュリティ更新）はすべて disabled で、private vulnerability reporting も無効
    （`gh api`）
  - `ci.yml` と `fuzz.yml` に `permissions:` が無い（`docs.yml` / `release.yml` /
    `docs-links.yml` には有る）。`ci.yml` の clippy / test / build に `--locked` が無い
    （package と msrv には有る）
  - dependabot が上げると壊れる・追随作業が要る依存がある: `wasm-bindgen = "=0.2.126"`
    （`Cargo.toml:60`。wasm-bindgen-cli と完全に同じ版で検索の wasm を作り直す必要がある）、
    comrak・syntect・two-face（本文 HTML が変わるとスナップショットの更新と
    `CACHE_FORMAT_VERSION` の bump が要る）。dependabot はどちらもしない
    （10-07 見直しの C3。`=` 固定を上げる PR を出すかは未確認）
  - `.claude/settings.local.json` が `.gitignore` に無く、未追跡のまま置かれている
- やること
  - `SECURITY.md`（報告先・対象の版・対応の目安）を置く。private vulnerability
    reporting・secret scanning・push protection を有効にする（リポジトリの設定。
    ユーザが操作する）
  - `.github/dependabot.yml`（cargo と github-actions）を置く。actions のフルコミット SHA
    でのピン留めは続ける
  - cargo-deny を CI に入れる（advisories と licenses。licenses の許可リストは
    Phase 81 の一覧と揃える）
  - `ci.yml` と `fuzz.yml` に `permissions: contents: read`。`ci.yml` の clippy /
    test / build に `--locked`
  - 依存更新の PR の扱い方（追随作業が要る依存の見分け方・マージ前の確認）を
    スキルに書く
  - `.gitignore` に `.claude/settings.local.json` を足す
- 判断点
  - **dependabot の運用** — 頻度（週次か月次か）・`groups` でまとめるか・`ignore` に
    入れて手動更新にする依存（wasm-bindgen・comrak・syntect・two-face）。1 人の運用で
    PR が溜まらない形にする
  - **cargo-deny の検査範囲** — advisories だけか、licenses・bans（重複版。sha2 の
    0.10 / 0.11）・sources まで入れるか。advisories は新しい勧告が出ると PR と無関係に
    CI が落ちるので、定期実行の別ジョブに分けるか
  - vendor 資産の勧告を続けて見る方法 — dependabot は vendor したファイルを見ない。
    定期実行のワークフローで `gh api /advisories` と照合するか、vendor-update スキルの
    手順に確認を足すだけにするか（v0.18.1 の更新は 10-06 に手で照合して見つけた）

### 83 公開範囲の制御 ⬜

**概要**: 社内の設計書を書く人が、公開したくないものを外へ出さずに済むようにする。
原稿の `.md`（frontmatter・HTML コメント込み）を配信しない設定を足し、dev / preview
サーバに Host / Origin の検査を入れ、社内で公開するときの注意と GitHub Pages 以外への
置き方を docs に書く。主な判断点は `.md` を止める設定の形と、止めたときのコピーボタン・
llms の扱い。

- 現状（実測。10-07 時点。「10-07 見直し」の A1〜A3）
  - 原稿の `.md` は全ページ必ず配信される。判定の `emits_page_md()`
    （`yuzu-core/src/model.rs:209`）は合成ページかどうかしか見ず、frontmatter の
    `llms: false` や `llms.enabled` は関係しない（`yuzu-render/src/pipeline.rs:305`）。
    `guide/llms.md` には「原文 Markdown（frontmatter 込み・バイトそのまま）が配信」と
    あるが、止める方法が無いことは書かれていない。HTML コメントは出力 HTML・`.md`・
    llms-full.txt に残る（`guide/quality.md`）
  - ページのコピーボタン（`page-copy.js`）は `data-md-url` の `.md` を読む。`.md` を
    止めるとコピーボタンも働かなくなる
  - dev / preview の `build_router`（`yuzu-server/src/serve.rs:152`）に Host / Origin の
    検査が無く、WebSocket の `/__livereload` も Origin を見ない。既定の bind は
    127.0.0.1 だが、DNS リバインディングでは loopback のサーバにも外部サイトから届く
    （コードで経路を確認。実機では未確認）
  - 公開の案内は GitHub Pages だけ（`README.md:60-62`・`guide/deploy.md`・
    `yuzu new` の終了メッセージ `commands/new.rs:79-80`・雛形 deploy.yml）。docs に
    公開範囲の注意や、社内の Web サーバ・GitLab Pages・S3 などへの置き方が無い
    （「非公開」「社内」「認証」等で 0 件）
- やること
  - `.md` の配信を止める設定を足す（サイト全体と、ページ単位）。止めたページでは
    コピーボタンを出さない
  - docs に「HTML コメントも公開される」注意を書く
  - dev / preview に Host の検査を入れる（`localhost` / `127.0.0.1` / `[::1]` / bind 先の
    アドレス / 設定の許可リスト以外は 403）。WebSocket は Origin も照合する
  - docs に「社内で公開する」ページ（公開範囲の注意・dist を nginx 等の Web サーバ・
    GitLab Pages・S3 に置く例・`base_url` の決め方）。README と `yuzu new` の終了
    メッセージに注意を 1 行
- 判断点
  - **`.md` を止める設定の形** — `llms.page_md = false` のような新キーか、ページの
    `llms: false` で `.md` も止めるか（後者は既存の意味を広げる）。既定は今のまま
    配信する案が有力（既存サイトの挙動を変えない）
  - HTML コメントを出力から除く機能は v0.19 では扱わず、注意書きだけにする。除くと
    本文 HTML が変わって `CACHE_FORMAT_VERSION` の bump が要り、この版の「キャッシュ形式を
    変えない」と衝突する。`<!-- yuzu-lint-disable-next-line … -->` 等の指示コメントとの
    関係の整理も要るので、採るなら v0.20 以降
  - Host の許可リストを設定キーにするか（`dev.allowed_hosts`）。`dev.host = "0.0.0.0"`
    で LAN に公開しているときの扱い

### 84 docs と雛形を実際の動作に合わせる ⬜

**概要**: README・docs・雛形・CLI のヘルプのうち、実際の動作と食い違う記述を直す。
設定例の JSON / YAML 風の書き方を TOML に揃え、図の描画方式の既定値などの誤りを正し、
既存の Markdown フォルダから始める手順と、dist をファイルとして開けないことを
ガイドに書く。10-07 の見直しを受けて、雛形 deploy.yml をリリースのバイナリを使う形に
替え、FAQ と問い合わせ先（Issues）への導線を足し、docs の目次を見直す。コードの変更は
雛形・用語集の nav 上の位置・CLI のヘルプ文言だけ。主な判断点は用語集の位置の決め方と、
docs サイトを main とタグのどちらから公開するか。

- 現状（実測。10-06 時点）
  - JSON / YAML 風の設定例: `guide/diagrams.md:11-12`・`guide/deploy.md:120`・
    `guide/writing.md:240`・`guide/writing.md:288`（`"abbr": false`）・
    `guide/code-and-math.md:175`・`development/index.md:72`・雛形の
    `scaffold/index.md:25`。コピーすると設定エラーになる
  - `guide/deploy.md:122` が「既定の `"ssr"`」と書いている（既定は client。
    `yuzu-config/src/schema.rs:288`）
  - README の「図は 10 図種をビルド時に SVG 化」（`README.md:21`）と「JS を使うのは
    検索とテーマ切替だけ」（`README.md:33`）が、既定の設定の動作と違う（図は
    mermaid.min.js、数式は KaTeX がブラウザで描く）
  - `guide/index.md:49` が「`content/` と `theme/` を監視」と書いている（実際は
    プロジェクトルート全体）。同じページのフォルダ構成図（33〜38 行）に
    `snippets/` が無い
  - 既存のフォルダから始めると: `yuzu new .` は「空ではありません」で止まり、
    `yuzu.toml` が無いと build は「`yuzu new` で作成するか…」と案内する。空の
    `yuzu.toml` を置いても、原稿がフォルダ直下だと `pages=0` で `404.html` だけが出る
    （10-06 に再現。`input.dir` の既定が `content`）。ガイドに記述が無い
  - フォルダ直下の原稿を `input.dir = "."` で読ませることはできない（出力先が
    原稿ディレクトリと重なるとして設定エラー・exit 2。PR #23 のレビューで確認）。
    原稿を `content/`（または任意のサブフォルダ ＋ `input.dir` の指定）へ移す必要がある
  - dist を `file://` で開けない（リンクがサイトのルートから始まる）ことは、
    `guide/search.md:119` の検索の注記にしか無い。`--base-url ./` は
    `/./_assets/…` を出力する（10-06 に再現）
  - 雛形のナビ: サイドバーに英小文字の「guide」が出る（`guide/index.md` が無い）。
    用語集は名前順で `guide` より前に並び、ホームの「次のページ」が用語集になる。
    本文の「次は はじめに へ」と食い違う
  - `yuzu --help` が「ロードマップと設計は README.md を参照」と書く。`dev` の説明に
    「WS ライブリロード」、`build --base-url` の説明に「baseUrl」「configure-pages の
    base_path」がある。雛形の `theme/README.md` がリポジトリ内のパス
    （`crates/yuzu-theme/assets/`）を指している
  - 以下は 10-07 時点の実測（「10-07 見直し」の C4・D1・D4・D5・D7・D9・D12・F6）
  - 雛形 deploy.yml は Rust を入れ、`cargo install --locked --git … --tag v__YUZU_VERSION__
    yuzu-cli` で yuzu をソースからコンパイルする。Releases のバイナリと SHA256SUMS は
    使っていない。対象は `branches: [main]` だけで、既定ブランチが別名だと動かない
  - FAQ・トラブルシューティング・問い合わせ先が無い（README・docs・雛形で「issue」
    「問い合わせ」「FAQ」「トラブル」が 0 件）
  - docs サイトは main への push ごとに HEAD の yuzu でビルドして公開する（`docs.yml`）。
    v0.19 の開発中は、v0.18 の利用者が見る docs に v0.19 の動作が先に出る
  - docs の目次: インストールの手順は「ガイド」の本体（h1 は「クイックスタート」）に
    あってサイドバーに見えない。テーマ・ダークモード・印刷が「配信とデプロイ」の中に
    ある。内部設計の「開発」セクションが利用者向けのページと同居し、検索結果にも混ざる
  - 図の client / ssr の選び方が書かれていない（client では図のあるページで
    mermaid.min.js 3.5 MB を読む）。節の見出しが英語の図種名だけで、「フローチャート」
    「ガントチャート」で検索しても 0 件
  - 利用者向けのガイドにライブラリ名・内部の言葉が多い（syntect・two-face・vaporetto・
    BM25・OPFS・kabosu・冪等・mtime 等）
  - 表示とヘルプの誤り: `reference/rules.md:14` の `\[\]` と `:117` の `error\[fmt\]` が
    コードスパンの中でバックスラッシュごと表示される・`yuzu new --help` が使えない
    `--root` を載せる・`yuzu lint` の説明が「見出し・frontmatter」で表記ゆれに触れない・
    `help` サブコマンドと `-h` の説明が英語
  - JSON 風表記は docs の外にも残っている（`.claude/skills/vendor-update/SKILL.md:30` の
    `backend: "ssr"`）
- やること
  - 設定例を TOML に揃える（`[markdown.mermaid]` ＋ `backend = "ssr"`、または
    `markdown.mermaid.backend = "ssr"`）。既定値・README・監視範囲・フォルダ構成図の
    誤りを直す
  - ガイドに「既存のフォルダで始める」節（原稿の置き場所の条件と移す手順）と、
    dist はサーバで配信する（`yuzu preview` で確認する・`file://` では開けない）旨を書く
  - 雛形に `content/guide/index.md` を足す。用語集を nav の末尾に置く
  - CLI のヘルプ文言から README への参照と内部の言葉を除く。雛形の `theme/README.md` は
    リポジトリのパスではなく docs のページを案内する
  - ci.yml の docs ゲートに JSON 風表記（`backend: "` 等）の否定 grep を足す
    （`if …; then …; exit 1; fi` で書く）。スキルの記述も直す
  - 雛形 deploy.yml を、リリースのアーカイブを取得して SHA256SUMS で照合する形に替える
    （`cargo install` は代替の手段としてコメントに残す）。対象ブランチの注意をコメントに
  - FAQ・トラブルシューティングのページ（検索が動かない・ポートが使用中・サブパスで
    リンクが切れる・図が出ない・CRLF で整形差分が出る 等）を作り、README と docs の
    トップに Issues へのリンクを置く
  - 目次: 「インストールと最初のサイト」と「見た目（テーマ・ダークモード・印刷）」を
    独立したページにする。「開発」セクションは開発者向けと明示する
  - 図のページに client / ssr の選び方（JS の有無・容量・対応図種・印刷）を足し、
    見出しに日本語の図種名を添える
  - 利用者向けのガイドの内部の言葉を、何ができるかの説明に置き換える（実装名は注記か
    開発セクションへ）
  - 表示とヘルプの誤りを直す
- 判断点
  - **docs サイトをどこから公開するか** — 今のまま main から公開するか、タグ（リリース）で
    公開し main は check だけにするか（`/next/` に出す案もある）。タグにすると docs の
    修正がリリースまで公開されない
  - **用語集の nav 上の位置** — 合成ページを常に末尾に置くか、位置の設定
    （`markdown.glossary` に順序のキー）を足すか。常に末尾に置くと既存サイトの nav の
    並びが変わる（全ページの HTML の nav が変わり、nav を含むスナップショットが動く）
  - `--base-url` に `.` 始まりを渡したときに警告するか、設定エラーにするか
    （相対パス出力への対応は大きいので v0.19 では扱わない）
  - `yuzu init`（`yuzu.toml` だけを作る）を足すか — 足さずにガイドの手順で済ませる案が
    有力

### 85 build が壊れたページを黙って出さない ⬜

**概要**: `yuzu build` の最後に、リンク切れ・インクルードの失敗などの件数と
「詳細は `yuzu check`」の 1 行を出す。原稿が 0 件のときは警告し、Mermaid の構文エラーの
警告にはページ名を付ける。あわせて build のログを利用者向けにする（色コードと
開発者向けの値）。10-07 の見直しを受けて、出力や原稿を黙って壊す 3 件（大文字小文字を
区別しない FS の孤児掃除・check と build のアセット判定の食い違い・原稿の書き込み）も
直す。終了コードは変えない（0 = 成功のまま）。主な判断点は数えるルールの範囲。

- 現状（実測。10-06 時点）
  - build の途中で警告するのは、リンク先の `.md` が見つからない場合（`urls.rs:113`。
    ページ名付き）と、SSR 時の Mermaid の構文エラー（`highlight.rs:297`。
    **ページ名なし**）など一部だけ。画像切れ・存在しない見出しへのリンク・
    インクルードの失敗は build では何も出ず、`yuzu check` で初めてエラーになる。
    README の手順（dev → build → preview）では check は CI 用に見えるので実行されない
  - Mermaid の書き間違いは、既定の描画方式（client）では build も check も何も言わない。
    SSR の警告も、行番号は図の中での行
  - 描画中の警告は、キャッシュに当たったページでは出ない（本文を作り直さないため）。
    2 回目以降の build では同じリンク切れが黙る
  - `yuzu check --root docs`（18 ページ）は約 0.03 秒、変更なしの
    `yuzu build --root docs` は約 0.35 秒、`--force` は約 1.1 秒（release ビルド・
    開発コンテナ・10-06）
  - 原稿 0 件の build は INFO の `pages=0` だけで終わる
  - build の最後のログは `インクリメンタルビルド body_hits=… body_misses=…
    search_hits=… orphans_removed=… elapsed=…`（`commands/build.rs:476`）。
    パイプ先にも ANSI の色コードが出る（10-06 に再現）
  - 以下は 10-07 時点（「10-07 見直し」の B3・B4・B6・D3。B3・B4 はコードで経路を確認し、
    実機では未確認）
  - 大文字小文字を区別しない FS（macOS の既定・Windows）で `Guide.md` を `guide.md` に
    改名すると、書いたばかりの `guide/index.html` を孤児掃除（`output.rs:275` の
    `remove_orphans`。render の後に走る）が前回の `Guide/index.html` として消す。
    route の衝突検査は文字列の完全一致だけで、`Guide.md` と `guide/index.md` の組を
    見逃す（並列の書き込みで勝者が実行ごとに変わる）
  - 同伴アセットの存在判定が食い違う: check は `content_dir.join(..).is_file()`
    （`linkcheck.rs:198`。リンクを辿り、隠しファイルや `input.ignore` の対象も「ある」）、
    build のコピー（`scan.rs:78` の `scan_content_assets`）はそれらを除く。check が通るのに
    公開サイトで画像が切れる
  - `yuzu fmt` / `lint --fix` は原稿を `std::fs::write` で直接書く（`commands/fmt.rs:44`・
    `commands/lint.rs:61`）。作り直せる `.yuzu/cache/global.json` の方は tmp → rename
  - `yuzu check` は整形差分を必ず error にする（`rules.rs:80` の `FMT`）。`lint.rules` で
    外せるのは warning だけなので、既存の原稿を持ち込むと全ファイルが赤になる
- やること
  - build の最後にリンク系の診断（check と同じルール）を数え、1 件以上なら
    「リンク切れ N 件・インクルードの失敗 M 件。詳細は `yuzu check`」を warn で出す。
    終了コードは 0 のまま
  - 原稿 0 件の build で警告する（`input.dir` の値と、プロジェクトルート直下に
    `.md` があればその旨）
  - Mermaid の構文エラーの警告にページ名を付ける
  - build のログ: 色コードは端末のときだけ、キャッシュの内訳（`body_hits` 等）は
    `-v` のときだけにする（`-q` / `-v` の扱いは v0.17 Phase 73 のとおり）
  - 孤児掃除は、今回書いたパスと大文字小文字を無視して一致するものを消さない。route と
    エイリアスの衝突検査に、大文字小文字を無視した比較（警告）を足す
  - check の画像の存在判定に、build のコピー対象（`scan_content_assets` の結果）を使う
  - 原稿の書き込みを同じディレクトリへの tmp → rename にし（パーミッションを引き継ぐ）、
    書く直前に内容が読み込み時から変わっていないかを照合する
- 判断点
  - **check の整形差分を外せるようにするか** — `check --no-fmt` か設定で外すか、
    warning に下げるか。あわせて fmt が変える項目の一覧を docs に書く
  - **件数の数え方** — build の後に check のリンク系ルール（`check_links` /
    `validate_includes` / `validate_spec_refs`）を全ページへ回す（キャッシュと関係なく
    毎回正しい。docs の規模で約 0.03 秒の追加）。描画中の検出を数える案は、キャッシュに
    当たったページの分が抜け、抜けないようにするには検出結果をキャッシュに載せる
    （= `CACHE_FORMAT_VERSION` の bump）必要があり、この版の「キャッシュ形式を
    変えない」と衝突するので採らない。決めるのはどのルールを数えるか（画像切れ・
    見出しへのリンク・仕様の `$ref` まで含めるか）
  - **数える対象のページ集合** — check は draft 込み（`build_source_pages`）、build は
    draft を除く（`build_site_model`。`--drafts` のときだけ含む）。check のルールを
    そのまま回すと、draft へのリンクが check では正常・出力物では切れている、という
    食い違いが出るので、build のページ集合で回す
  - watch / dev の再ビルドでも毎回出すか（変更が無くても同じ警告が出続ける）
  - ログの時刻を消すか（watch / dev では再ビルドの区切りとして時刻が役に立つ）
  - Mermaid の構文エラーを client 描画のときも check で報告するか — tankan のパースだけ
    通すと、tankan が未対応の図種・構文との切り分けが要る（`is_unsupported` で区別
    できる範囲だけ報告する案）。v0.19 で扱うかも含めて決める

### 86 dogfooding ⬜

**概要**: Phase 81〜85 を docs サイト・雛形・CI・リリースの手順で実運用する。
ライセンス一覧がアーカイブと dist に入ること、`.md` の配信設定と Host の検査、
build の件数表示、雛形のナビと deploy.yml を実物で確かめ、CI のゲートと
verify・release スキルを追随させる。最後に「v0.19 の方針」のリリース判定を確かめる。

- docs サイト: dist のライセンス文と `_assets/vendor/README.md` が出ないことを確かめる。
  ライセンスの扱い（利用者のサイトに何が出るか）を docs に書く
- release.yml: アーカイブにライセンス一覧が入っていることを検証条件に足す
  （release スキルの検証条件も）
- ci.yml の e2e: 雛形の build が件数の警告を出さないこと・ライセンス文が出ること・
  vendor の README が出ないこと・`.md` を止める設定で `.md` が出ないことを照合する。
  dev / preview に許可していない Host を送って 403 になることを確かめる。docs ゲートに
  新しい節の grep を足す
- 雛形 deploy.yml を実際の GitHub リポジトリで動かし、リリースのバイナリでデプロイ
  できることを確かめる（ユーザの作業）
- verify スキルの追随
- 判断点: 雛形の原稿にわざとリンク切れを置いて件数表示の実例にするか（雛形の
  `yuzu check` が失敗するようになるので、置かない案が有力）

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

v0.19 で扱う項目は取り消し線にして移動先の Phase を書いた。現状は 10-06 時点で
測り直し、詳細とともに各 Phase 節（上の「現在: v0.19」）へ移した。

### 配布物として必要なもの

4 件とも v0.18.1 と v0.19 の Phase 81・82 へ移した。

- ~~**第三者ライセンスの表記が配布物に無い**~~ → v0.19 Phase 81
- ~~**vendor 資産の更新メモが公開サイトに配信されている**~~ → v0.19 Phase 81
- ~~**脆弱性の窓口と監視が無い**~~ → v0.19 Phase 82（見直しの時点で未確認だった
  mermaid のセキュリティ修正は、10-06 に勧告 5 件が該当すると確かめた。KaTeX も 1 件。
  この 2 つの更新は v0.18.1 で先に出す）
- ~~**tankan の修正（Phase 77）が crates.io に出ていない**~~ → v0.19 Phase 81

### 利用者の体験と docs

- ~~**build が壊れたページを黙って出す**~~ → v0.19 Phase 85
- ~~**docs と雛形の設定の書き方が TOML になっていない**~~ → v0.19 Phase 84
- ⬜ **frontmatter の型エラーが英語で、1 件ずつ、行番号がずれる**（工数: 中）
  - `order: "最初"` で `invalid type: string "最初", expected i64 at line 2 column 8`。
    `i64` が利用者に分からない。行番号は frontmatter の中での行で、ファイル上の行と
    ずれる。直すと次のエラーが出る（yuzu.toml のエラーは位置付き・日本語・全件で、
    落差が大きい）
  - 対処案: serde_yaml_ng のエラーを日本語に読み替え、行をファイル上の行へ足し直す。
    キーごとの型検査で全件出す（kabosu の decode と同じ考え方）
- ~~**手元の Markdown フォルダから始める方法が書かれていない**~~ → v0.19 Phase 84
  （原稿が 0 件の build の警告は Phase 85）
- ~~**dist をファイルとして直接開けないことが書かれていない**~~ → v0.19 Phase 84
- ~~**雛形のナビが不自然**~~ → v0.19 Phase 84
- ⬜ **検索で 2 文字の語に無関係な結果が混ざる**（工数: 小〜中）
  - `yuzu search "単語"` に「英語」を含む節が出る。誤字補正（1 文字違いまで許す）が
    2 文字の語にも効くため
  - 対処案: 語の長さで誤字補正を切る（3 文字以上だけ等）。mikan の変更なので
    native / wasm が同じ実装を通ることと、`FORMAT_VERSION` を上げずに済むかを確かめる
- ⬜ **細かな分かりにくさ**（工数: 小）
  - うち、ビルドのログの UTC 時刻・開発者向けの値・パイプ先の色コードは v0.19 Phase 85、
    `yuzu --help` の文言・ガイドのフォルダ構成図・雛形の `theme/README.md` は
    v0.19 Phase 84 へ移した
  - ページ数が build では 4・検索インデックスでは 3・check では 2 と食い違う。
    何も抑制していないのに「抑制 3 件」と出る（雛形の原稿の分）
  - 雛形の原稿が機能カタログになっていて、自分の原稿を書き始めるには大半を消す必要がある

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
  - ~~`ci.yml` の test / build に `--locked` が無い~~ → v0.19 Phase 82
  - 公開リポジトリの定期実行（docs-links.yml）は、60 日間動きが無いと GitHub が止める
  - 対処案: windows / macos の `cargo test` を matrix に / tankan の `render_svg` を
    fuzz 対象に足し、月次の定期実行
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
- ~~**`.claude/settings.local.json` が .gitignore に入っていない**~~ → v0.19 Phase 82

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

## 10-07 見直し

**概要**: 10-07 に、10-04 と同じ 3 つの視点（上級プログラマー・上級マネージャー・
一般利用者）でもう一度全体を見直した結果。10-04 の持ち越し・v0.19 の計画・候補に
すでにある項目は除き、新しい指摘は 42 件（2 件は別々の視点から独立に挙がった）。
3 者は次の 3 点で一致した。

1. 想定する利用者は社内の設計書を書く人なのに、公開してはいけないものが外へ出る経路が
   3 つある（A1〜A3）。v0.19 の他の Phase より先に扱う → 新しい Phase 83
2. 同梱の mermaid・KaTeX の脆弱性の修正は、v0.19 を待たずにパッチで出す → v0.18.1
3. 利用者からの声が届く導線が無い。Release の各アセットのダウンロード数は 1 回で、
   これは release.yml 自身が SHA256SUMS を作るために取得した分。issue は 0 件
   → FAQ と Issues への導線は v0.19 Phase 84、利用者を増やす取り組みは v0.20 の候補

優先順位は 3 者で違った。プログラマーは黙って誤った結果を出す不具合（B1〜B3）、
マネージャーは計画の範囲と完了条件（F4）と、雛形 deploy.yml・docs の先行公開、
利用者は公開範囲・deploy のコンパイル待ち・検索の順位・FAQ を先に挙げた。
工数の小さい不具合は v0.18.1、公開範囲は新しい Phase、deploy.yml と FAQ は docs の
Phase、検索の順位は既知の「2 文字の語」とあわせて v0.20 の候補に置いた。

各項目は 10-04 節と同じく「現状（根拠）・対処案・工数」の順に書く。確認の仕方は
3 段階で書き分ける: 手元で確かめたもの（**10-07 に確認**）、コードで経路を追って
断定したもの（**実機未確認**）、確かめていないもの（**未確認**）。ファイルの行番号は
10-07 時点。v0.18.1・v0.19 へ移した項目は取り消し線にして移動先を書き、詳細は移動先に
書いた。

### A. 公開範囲・情報の出し過ぎ

- ~~**A1 案内が GitHub Pages だけで、公開範囲の注意も社内での置き方も無い**~~ → v0.19 Phase 83
- ~~**A2 原稿の `.md` が frontmatter・HTML コメントごと必ず配信され、止める設定が無い**~~
  → v0.19 Phase 83
- ~~**A3 dev / preview サーバが Host / Origin を検査しない**~~ → v0.19 Phase 83
- ~~**A4 通常の build でも `__yuzu/build_id`（ビルド時刻）が dist に出る**~~ → v0.18.1

### B. 黙って誤った結果を出す不具合

- ~~**B1 サイト通し番号の本文キャッシュが、他ページの順序の変更で無効化されない**~~ → v0.18.1
- ~~**B2 プロジェクトの祖先に隠しディレクトリがあると、dev が黙って再ビルドしない**~~ → v0.18.1
- ~~**B3 大文字小文字を区別しない FS で、孤児掃除が書いたばかりの出力を消す**~~ → v0.19 Phase 85
- ~~**B4 同伴アセットの存在判定が check と build で食い違う**~~ → v0.19 Phase 85
- ⬜ **B5 watch 中に辞書ファイルの中身だけを変えると、索引とブラウザのトークナイザが
  ずれる**（工数: 小。**実機未確認**）
  - モデルのバイト列は build のたびに読み直して dist へコピーするが、Tokenizer は
    セッションの `OnceLock`（`yuzu-index/src/builder.rs:94`）に保持されて作り直されない。
    辞書の指紋を含む envKey を計算する `BuildSession::new` は、watch では `yuzu.toml` の
    文字列が変わったときにしか呼ばれない（`commands/build.rs:231`）
  - 旧モデルで作った索引に対して、ブラウザだけが新モデルでクエリする。CLAUDE.md の
    「検索の最重要制約」（index と query で同じモデルバイト）に反する。dev 中に辞書を
    差し替えたときだけ起き、再起動で直る
  - 対処案: 再ビルドのたびに辞書の指紋を比べ、違えばセッションを作り直す。または
    Tokenizer をモデルのハッシュをキーにして保持する
- ~~**B6 `yuzu fmt` / `lint --fix` が原稿を直接上書きする**~~ → v0.19 Phase 85
- ~~**B7 外部リンク検査の curl が `~/.curlrc` を読む**~~ → v0.18.1
- ⬜ **B8 用語集の合成で、用語を Markdown として解釈する**（工数: 小。**実機未確認**）
  - 用語集ページは `## {term}` を生のまま埋め込み（`markdown/glossary.rs:168`）、
    `render.unsafe = true` で描くので、`__init__` は太字の「init」に、`Vec<T>` の `<T>` は
    生 HTML として消える。見出しのアンカーもずれる。本文の `<abbr title>` 側では
    エスケープした生テキストなので、表示が食い違う
  - 対処案: 用語の Markdown 記号をエスケープしてから合成する

### C. 配布物と依存

- ~~**C1 mermaid.min.js に DOMPurify など他の OSS のライセンスが入っている**~~ → v0.19 Phase 81
- ~~**C2 tankan の crate に corpus が入り、出所の記録が無い**~~ → v0.19 Phase 81
- ~~**C3 dependabot が、追随作業の要る依存（wasm-bindgen・comrak 系）の PR を積む**~~
  → v0.19 Phase 82 の判断点
- ~~**C4 雛形 deploy.yml が毎回 yuzu をソースからコンパイルする**~~（プログラマー以外の
  2 者が独立に指摘）→ v0.19 Phase 84

### D. 初めての利用者の体験

- ~~**D1 FAQ・トラブルシューティング・問い合わせ先が無い**~~（2 者が独立に指摘）
  → v0.19 Phase 84
- ~~**D2 docs で「設定」と検索しても設定のページが上位に出ない**~~ → v0.20 以降の候補
- ~~**D3 `yuzu check` は整形差分を必ず error にし、外せない**~~ → v0.19 Phase 85 の判断点
- ~~**D4 公開 docs が main から作られ、配布しているバイナリより先に進む**~~
  → v0.19 Phase 84 の判断点
- ~~**D5 docs の目次で、インストール・見た目の設定が見つけにくい**~~ → v0.19 Phase 84
- ⬜ **D6 他ツールから来た人がつまずく違いが、どこにもまとまっていない**（工数: 小）
  - ナビは自動生成だけ（`nav.auto = false` は予約で効果なし）。サイドバーの表示名と
    ページタイトルを分けられず、ナビから隠すキーも無い。ファイル名がそのまま URL に
    なる（`01-intro.md` → `/01-intro/`）。frontmatter のキーは camelCase
    （`readingTime`・`lintDisable`）で yuzu.toml は snake_case。生 HTML が書けるか・
    画像の大きさを指定できるかが docs に無い。`README.md` を index として扱うかは**未確認**
  - 対処案: 「他ツールとの違い・よくある書き方」のページ。手動ナビ・ナビから隠す機能は
    別の判断（大）
- ~~**D7 図の client / ssr の選び方が無く、日本語の図種名で検索できない**~~ → v0.19 Phase 84
- ⬜ **D8 favicon・フッター・ヘッダーのリンクを設定で足せない**（工数: 小〜中。
  **10-07 に確認**）
  - `base.jinja` に `<link rel="icon">` が無い。`guide/deploy.md:184` は「favicon は
    `public/` に置く」と書くが、サブパス配信ではブラウザがドメイン直下の
    `/favicon.ico` を探すので効かない（このサイトの `/yuzu/favicon.ico` も 404）
  - フッターはテンプレートに無く、ヘッダーはタイトル・検索・テーマ切替だけ。「社外秘」
    表示や社内ポータルへのリンクを出すにはテンプレートの上書きが要り、上書きした
    テンプレートの追随は利用者の責任になる
  - 対処案: `site.favicon`・フッターの文言・ヘッダーのリンクの設定キー
- ~~**D9 利用者向けのガイドに内部の言葉が多い**~~ → v0.19 Phase 84
- ⬜ **D10 読者側の UI の小さな穴**（工数: 小。**10-07 に確認**。iOS の挙動は**未確認**）
  - ヘッダーの検索欄は `font-size: 0.9rem`（`theme.css:269`）。iOS Safari は 16px 未満の
    入力欄でフォーカス時に画面を拡大する
  - ヘッダーの検索欄に `aria-label` が無い（`partials/header.jinja:12`。検索結果ページの
    入力欄には有る）
  - テーマ切替ボタンに押した状態を示す属性が無く、一度選ぶと「OS に従う」へ戻れない
  - ショートカットの表記が「Cmd+K」と「Cmd/Ctrl+K」で揃っていない（JS は Ctrl も受ける）
  - 対処案: 16px・`aria-label`・切替ボタンの状態と「OS に従う」・表記を「Ctrl/Cmd+K」に
- ⬜ **D11 読者に見える文言が開発者向け**（工数: 小。i18n の候補とも関係する）
  - 検索の初期化に失敗すると「検索を初期化できませんでした（コンソール参照）」
    （`search-ui.js:237`・`search-page.js:228`）。Admonition のラベルは英語
    （NOTE / TIP 等）で、日本語にする設定が無い
  - 対処案: 読者向けの文言（再読み込み・管理者への連絡）にする。ラベルの設定キー
- ~~**D12 表示とヘルプの小さな誤り**~~ → v0.19 Phase 84

### E. 公開 crate・テスト・コード

- ⬜ **E1 公開 crate（tankan / mikan）の semver 上の危うさ**（工数: 小〜中）
  - tankan の `Error`（`tankan/src/error.rs`）・`Options` / `Theme` に `#[non_exhaustive]`
    が無い（`DiagramKind` には有る）。mikan の `FormatError` は `#[from] serde_json::Error` /
    `fst::Error`（`mikan/src/error.rs:5,13`）で、serde_json と fst が公開依存になる。
    mikan の `BuildOptions` 等は pub フィールドだけの構造体で、フィールドを足すたびに
    Rust の API として破壊的変更になる（CLAUDE.md の「フォーマットは `serde(default)` で
    足す」とは別の話）。publish-crate スキルに `cargo semver-checks` が無い
  - 対処案: `#[non_exhaustive]` ＋ `Default` かビルダー・エラーの中身を隠す・
    スキルに semver-checks
- ⬜ **E2 mikan の `Shard::parse` が不正なバイト列で panic・巨大確保をしうる**
  （工数: 小。既知「CI の穴」の新しい側面。**実機未確認**）
  - `(term_count as usize + 1) * 4`（`mikan/src/shard.rs:96`）が wasm32 で桁あふれし、
    長さ検査を通った後に範囲外の添字で panic する。`Vec::with_capacity(doc_freq)`
    （同 137）は入力値をそのまま使う。wasm は panic で abort するので検索全体が止まる
  - 対処案: `checked_*` と確保量の上限。`Shard::parse` を fuzz の対象に足す
- ⬜ **E3 大事な経路のテストが無い**（工数: 中。既知「CI の穴」の新しい側面）
  - envKey / routesKey / `build_once` にテストが無い（routesKey は v0.18.1 の B1 で
    テストを足す）。ci.yml の e2e に「編集すると再ビルドが起きる」・dev / preview の
    HTTP 取得・`lint --fix`・`--drafts` が無い。tankan の corpus テストの検証は座標だけで、
    「元の文のラベルが SVG にあるか」を見ていない
  - 対処案: e2e に正方向の watch を 1 本・tankan にラベルの存在の検証
- ⬜ **E4 rel を `/` 区切りにする変換が `rel_to_slash` の外に 8 か所ある**
  （工数: 小。既知「同じ処理が 2 か所にある」の新しい側面）
  - `urlpath::rel_to_slash`（`yuzu-core/src/urlpath.rs:117`）を使わない手書きが
    lib.rs・cache.rs・pipeline.rs・assets.rs・build.rs・fmt.rs・diag.rs にある。メタと本文の
    キャッシュキーが別実装なのに一致が前提。routesKey は `p.rel.display()`（OS 依存）
  - 対処案: `rel_to_slash` にまとめる
- ⬜ **E5 静的ファイルを毎回全バイト読み、ページの sha256 を 1 ページ 4 回計算する**
  （工数: 小。既知「性能の気になる点」の新しい側面。**未計測**）
  - 対処案: サイズと mtime が同じなら読まない・ハッシュは `Page` に 1 回だけ持たせる
- ⬜ **E6 ブラウザ側に自動の検証が無い**（工数: 中）
  - テーマの JS は 13 ファイル・1,041 行。JS のテストも CI でのブラウザ実行も無く、
    v0.18 の Phase 78・79 の確認は人手（Phase 80）だった
  - 対処案: `docs/dist` に対するスモークテスト（検索・テーマ切替・キーボード操作）を
    5〜10 本。Node は CI のジョブの中だけで使う

### F. 運営と持続性

- ~~**F1 外部の利用者が実測でほぼ 0 で、測る運用も無い**~~ → v0.20 以降の候補
- ⬜ **F2 判断点が応答の無いまま推奨案で出荷されている**（工数: 小。**10-07 に確認**）
  - PR #22（panic の出力・回帰テストの方法・`base_url`）と PR #25（パーマリンクの形・
    読了時間を既定で出すか）は、確認に応答が無かったので推奨案で進めたと PR 本文に
    ある。`base_url` は策定時の案（設定エラー）と違う結論になった。v0.19 の判断点は
    16 件ある
  - 対処案（提案。採否はユーザが決める）: 判断点を「承認済み」と「仮決め」に分けて
    PR と ROADMAP に記録し、仮決めが残るならリリースノートに書く規則を CLAUDE.md に足す
- ⬜ **F3 レビューの記録が GitHub に残らない**（工数: 小）
  - 28 の PR すべてで GitHub 上の reviews・comments が 0 件。PR 本文と ROADMAP には
    「レビュー指摘」が出てくるので、別の経路でレビューしていると考えられるが、誰が
    どの観点で見たかは残っていない
  - release スキルはバンプコミットと ROADMAP の整理を main へ直接 push する手順なので、
    既知の「main の保護（必須チェック）」をそのまま入れると衝突する
  - `.claude/settings.local.json` が `Bash(gh pr *)` を許可しており、`gh pr merge` も含む
  - 対処案: レビューの手順と記録の残し方をスキルにする・release のバンプを PR 経由に
    してから main を保護する
- ~~**F4 v0.19 の計画にリリース判定・ユーザの作業・後回しにできる印が無い**~~
  → 「v0.19 の方針」に追加済み
- ⬜ **F5 性能と、リリース後の不具合件数の推移を測っていない**（工数: 小〜中）
  - ベンチマークが無く、「保存から約 1 秒」（README・docs）を続けて測っていない。
    リリース後に見つかった不具合は ROADMAP の文章の中にしか無い。最後の PR のマージ
    当日にタグを打つ（v0.9.1・v0.10.1 のようなすぐ後のパッチが出やすい）
  - 対処案: 決まった入力の build 時間を CI で測ってリリース行に記録・リリース行に
    「リリース後に見つかった不具合 N 件」・マージからタグまで 1 日空ける
- ⬜ **F6 文書と CI のゲートを二重・三重に管理している**（工数: 小〜中）
  - ci.yml の `grep -q` は 136 個で、verify スキルに 53 個を手で写している。
    `docs/yuzu.toml` の「25-45 行目」が CLAUDE.md・スキル・docs・ci.yml の 5 ファイルに
    直書きされている。release スキルの「過去 11 回」「過去 11 タグ」は古い（v0.18.0 で
    21 リリース目）
  - CLAUDE.md は約 420 行で、セッションのたびに読み込まれる。公開 docs の開発者向け
    ページより詳しく、人が読む順番がどこにも無い
  - 対処案: docs のゲートを `scripts/docs-gates.sh` にまとめて CI とスキルから呼ぶ・
    数値や行番号を「何を見れば分かるか」に書き換える。スキルの JSON 風表記は
    v0.19 Phase 84 の否定 grep で直す
- ~~**F7 名前「yuzu」が他のプロジェクトと重なる**~~ → v0.20 以降の候補
- ⬜ **F8 docs の URL が独自ドメインと org 側のリポジトリに依存し、雛形に焼き込まれて
  いる**（工数: 小。ドメインの管理は**未確認**）
  - `ai.implementer.net` は org の `ai-implementer.github.io` リポジトリの CNAME で
    決まる（10-07 に確認）。雛形の `yuzu.toml:2` と `getting-started.md:433` に URL が
    あり、利用者のプロジェクトにコピーされる。ドメインの保有者と更新期限の記録が無い
  - 対処案: ドメインの保有者と更新日を運用メモに残す・URL を変えるときの
    リダイレクトの方針を決めておく

## v0.20 以降の候補

### dogfooding 候補（v0.13 Phase 61 からの持ち越し）

- ~~**OS ダーク追従** / **見出しパーマリンクのキーボード到達性** / **`<head>` メタ** /
  **ページメタの拡充（読了時間・文字数）**~~ — 4 件とも v0.18 の Phase 76・78・79 で
  実装済み（内訳は「これまでのリリース」の「完了済み: v0.18」）

### 10-07 見直しからの候補

- **最初の外部利用者**（10-07 見直しの F1）— v0.20 のテーマ候補。外部の利用者を得て、
  その使い方を観察する
  - 現状（10-07 に確認）: Release の各アセットのダウンロード数は v0.16〜v0.18 で 1 回
    ずつで、これは release.yml の publish-release が SHA256SUMS を作るために
    `gh release download` した分。直近 14 日の閲覧は約 80 回・閲覧者 1 人、issue は
    累計 0 件（`gh api`）
  - 対処案: リリースごとに、自分の取得分を引いたダウンロード数と閲覧数をリリース行に
    記録する。FAQ と Issues への導線（v0.19 Phase 84）の後に、既知の「外部への露出」
    （英語の README・他ツールとの比較・cargo-binstall）とあわせて進める
- **検索の順位**（10-07 見直しの D2）— 既知の「2 文字の語に無関係な結果が混ざる」と
  同じ mikan のクエリ側の変更なので、1 つの Phase にまとめる
  - 現状（10-07 に確認）: docs で「設定」と検索すると、1 位は `guide/search.md` の
    同義語の設定例（スコア 7.17）で、2 位以下は 2.4 前後。設定のページは節が 9 位に
    出るだけ。docs の `synonyms = [["図", "ダイアグラム"], ["設定", "コンフィグ"]]` に
    対して、同義語の語の点を元の語と同じ重みで足し合わせる（`mikan/src/engine.rs`）ため、
    同義語の組をそのまま書いた節が 2 語分の点を得る。「シーケンス図」「ER図」でも
    同じ節が 1 位になる。ページや節を検索の対象から外す手段も無い
  - 対処案: 同義語のグループの中では最大の点だけを数える・ページタイトルとの一致を
    優遇する・frontmatter に `search: false`。スコアの計算は wasm 側の変更なので、
    `FORMAT_VERSION` は据え置ける見込み（新しい manifest と古い wasm の組でも壊れない）
- **名前「yuzu」の調査**（10-07 見直しの F7）
  - 現状: GitHub でリポジトリ名に yuzu を含むものは約 2,850 件で、上位は Switch
    エミュレータの関連。crates.io の名前が取られている件は既知。商標は**未調査**
  - 対処案: 改名するか、区別のための副名を付けるかを早めに決める（URL・雛形・crate 名が
    増えるほど改名の費用が上がる）。商標を簡単に調べる

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
- **v0.18**（Phase 76〜80）公開サイトの仕上げ
  - `<head>` に canonical・共有カード（OGP）・`generator`（新キー `site.image`）
  - 初めて使う人が最初に踏む不具合 6 件の修正（10-04 の見直しで発見）— Linux で
    `yuzu dev` の再ビルドが止まらない / Mermaid の入力でビルドが落ちる / frontmatter の
    読み違い（新ルール `frontmatter-unrecognized`）/ 雛形 deploy.yml の版の固定 /
    MSRV の検査（ワークスペースは 1.87 へ）/ `base_url` での panic と panic 時の終了コード
  - 見出しのパーマリンクをキーボードと支援技術から届く形に・読了時間と文字数
    （`theme.reading_time` / frontmatter `readingTime`）
  - OS ダーク追従 — JS 無効でも切替ボタンなしでも OS の設定に従う。`theme.dark` を
    `"toggle"` / `"auto"` / `"light"` の 3 値に（旧形式の bool も読む）
  - dogfooding — 読了時間を本文の前へ・雛形 deploy.yml でも共有カードが出る・
    docs サイトの og:image
  - `CACHE_FORMAT_VERSION` 22 → 23（Phase 78 の 1 回）

検索エンジン本体 **mikan**（旧 yuzu-index-format）と wasm ラッパ **mikan-wasm**
（旧 yuzu-search-wasm）は v0.7 リリース後に yuzu- プレフィックスを外して改名し、
mikan は crates.io で単独公開している（tankan と同じく独立バージョン）。

各版の Phase 内訳:

<details>
<summary>完了済み: v0.18（Phase 76〜80）の内訳</summary>

軸は「**公開サイトの仕上げ**」。公開物（HTML）の質は v0.12 以来手を入れておらず、
v0.13 Phase 61 からの持ち越し 4 件（`<head>` メタ / 見出しパーマリンクのキーボード到達性 /
ページメタ / OS ダーク追従）が候補欄に 4 版分残っていた。共有（SNS カード）・
検索エンジン（canonical）・支援技術（キーボード到達性）・OS 設定（ダーク追従）の 4 方向から
「公開サイトとして過不足ない」状態にした。Phase 76 の完了後、上級プログラマー・
上級マネージャー・一般利用者の 3 つの視点でプロジェクト全体を見直し、初めて使う人が
最初の数分で踏む不具合を Phase 77 として先に直した（残りの指摘は
「10-04 見直しの持ち越し」）。本文 HTML が変わる 2 件は Phase 78 に束ね、
`CACHE_FORMAT_VERSION` の bump は 22 → 23 の 1 回で済ませた。ブラウザが要る確認
（Tab 操作・ダーク表示・SNS カード）は、開発環境にブラウザが無いのでユーザが行った。

- **76 `<head>` メタ（canonical / OGP）** — `base.jinja` の `<head>` に canonical・
  `og:*`・`twitter:card`・`generator` を足し、新キー `site.image`（og:image の素材。
  `site.logo` は SVG で受け付けないクローラが多い）を追加。canonical / `og:url` /
  パス指定の og:image は `base_url` がフル URL のときだけ出す（sitemap と同じゲートを
  `UrlResolver::is_absolute_base` に揃えた。相対の canonical は同一性の宣言として弱い）。
  `og:type` は全ページ website、`twitter:card` は summary 固定、`og:locale` は `site.lang`
  が地域付きのときだけ（推測しない）、`generator` に版を含めない（バンプごとに
  スナップショットが動く）。レビュー指摘で `| url` を HTML 属性専用（`&` を `&amp;`）にし、
  `<script>` 内用の `| url_js` を新設した
- **77 初めて使う人が最初に踏む不具合の修正** — 6 件。
  (1) Linux の inotify は開いた・読んだだけのイベントも届け、種類を捨てる
  notify-debouncer-mini では `yuzu dev` の再ビルドが止まらなかった（10 秒に 33 回。macOS
  では起きない）→ notify を直接使い、監視スレッドで種類を絞って debounce する。
  監視スレッドの panic は `WatchFailure` で `serve` に知らせて配信ごと止め exit 2。
  (2) tankan の gantt が `5日` で panic・state の入れ子の循環でメモリを使い切った →
  パースエラーにし、描画側で panic を回収してクライアント描画へ（回収の仕組みは
  yuzu-core の `recover`。comrak の整形もこれに揃え、hook の差し替えをやめた。
  **hook の中では終了させない**）。
  (3) frontmatter の閉じ忘れが無言で、`yuzu fmt` が痕跡を消した → 新ルール
  `frontmatter-unrecognized`（error）で閉じ忘れ・TOML 形式・区切り線の誤読（comrak が
  切り出した中身が YAML のマッピングでない）を報告し、fmt はそのページを書き換えない。
  先頭の区切り線で始まる正常な文書は誤検出しない（レビュー指摘で、引用符付きキーや
  フロー形式の frontmatter を誤読と判定しないよう YAML として読む判定に直した）。
  (4) 雛形 deploy.yml が main を版指定なしでインストールしていた → `yuzu new` した版の
  `--locked --tag v<版>` に固定。
  (5) MSRV ジョブが `rust-toolchain.toml` に負けて stable で動いていた →
  `RUSTUP_TOOLCHAIN` で版を指定。1.85 では mikan の依存 ruzstd 0.8.2 が通らないので
  ワークスペースは 1.87、kabosu・tankan は 1.85。
  (6) `base_url = "/:x/"` で preview が panic・panic 時の終了コードが 101 →
  `without_v07_checks` と `{` `}` のエンコードで拒まずに配信し、panic は exit 2
- **78 本文 HTML の到達性とページメタ** — comrak の header_ids の既定出力（先頭の
  `aria-hidden` 付き空 `<a>`）は CSS の `visibility: hidden` と合わせてキーボードからも
  支援技術からも届かなかった。comrak の HeadingAdapter（yuzu-core の
  `markdown/heading.rs`）で id を見出し自身に付け、パーマリンクを末尾に `aria-label` 付きで
  出す（採番の入力は comrak と同じなので既存の `#id` は変わらない）。CSS は opacity と
  `:focus-visible`。読了時間と文字数は `markdown/reading.rs` が本文の文章を数え（日本語
  1 分 500 字・英数 200 語。コード・図・数式は数えない）、`theme.reading_time` と
  frontmatter `readingTime` で消せる。レビュー指摘で、数える場所を Markdown 断片の展開後
  （`render_body_html` の中）へ移し、本文キャッシュに載せた（原文で数えると ` ```include `
  で取り込んだ文章が抜けていた）
- **79 OS ダーク追従** — `data-theme="light"` を無条件で書いていたため、JS 無効や
  `theme.dark = false` では OS がダークでもライトだった。`data-theme` を付けない状態を
  「OS に従う」にし、ダーク定義を明示の選択（`html[data-theme="dark"]`）と OS 追従
  （`prefers-color-scheme` の `html:not([data-theme])`）の 2 系統にした（theme.css の
  手書きの 2 ブロックはテストで一致を縛り、syntect.css と `css_vars_dark` は `css.rs` の
  `dark_two_ways` が同じ生成関数から出す）。`theme.dark` は 3 値で、旧 `true` は toggle、
  `false` は light として読む（既存サイトの見た目は変わらない）
- **80 dogfooding** — 読了時間を本文の前（パンくずの下）へ移した。雛形 deploy.yml も
  docs.yml と同じく `https://<host><base_path>/` を渡し、利用者のサイトでも canonical・
  共有カード・sitemap.xml が出るようにした。雛形の原稿に新機能の実例を足し、docs サイトの
  og:image（`docs/public/images/og.png`。開発環境にフォントも画像ツールも無いので SVG を
  手で描いて resvg で書き出した）を設定。リダイレクト HTML の canonical は既に絶対 URL で
  変更不要と確認。ci.yml の `run:` に GitHub Actions の式を書くと bash より先に評価される
  罠を踏みかけ、CLAUDE.md に記録した。ユーザがブラウザで Tab 到達・読了時間・ダークの
  各経路・SNS カードを確認した（問題なし）

</details>

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
