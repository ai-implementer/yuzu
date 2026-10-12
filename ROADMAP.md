# ロードマップ

yuzu の開発計画と、これまでのリリースの内訳。**このファイルが Phase 状態の正**
（README には現在の版と概要だけを置く）。

## 現在

**v0.19 まで公開済み**。次の版（v0.20）は未策定で、候補は下の
「[10-04 見直しの持ち越し](#10-04-見直しの持ち越し)」・「[10-07 見直し](#10-07-見直し)」・
「[10-12 レビュー](#10-12-レビュー)」と「[v0.20 以降の候補](#v020-以降の候補)」にある。
10-12 レビューの R1〜R3 は手元で不具合を再現している。着手時に軸を 1 つ選んで Phase を切る。
kabosu 0.2.0 / tankan 0.2.1 / mikan 0.2.1 は crates.io で公開済み（yuzu のリリースとは
非同期。kabosu の publish 前に fuzz を回す規律は CLAUDE.md にある）。

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
測り直し、詳細とともに各 Phase 節へ移した（v0.19 のリリース後、各 Phase 節は「これまでの
リリース」の「完了済み: v0.19」に要約した）。

### 配布物として必要なもの

4 件とも v0.18.1 と v0.19 の Phase 81・82 へ移した。

- ~~**第三者ライセンスの表記が配布物に無い**~~ → v0.19 Phase 81
- ~~**vendor 資産の更新メモが公開サイトに配信されている**~~ → v0.19 Phase 81
- ~~**脆弱性の窓口と監視が無い**~~ → v0.19 Phase 82（見直しの時点で未確認だった
  mermaid のセキュリティ修正は、10-06 に勧告 5 件が該当すると確かめた。KaTeX も 1 件。
  この 2 つの更新は v0.18.1 で先に出した）
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
書いた（v0.18.1 の分はリリース後、「これまでのリリース」の「完了済み: v0.18」へ、v0.19 の分は
「完了済み: v0.19」へ移した）。

### A. 公開範囲・情報の出し過ぎ

- ~~**A1 案内が GitHub Pages だけで、公開範囲の注意も社内での置き方も無い**~~ → v0.19 Phase 83
- ~~**A2 原稿の `.md` が frontmatter・HTML コメントごと必ず配信され、止める設定が無い**~~
  → v0.19 Phase 83
- ~~**A3 dev / preview サーバが Host / Origin を検査しない**~~ → v0.19 Phase 83
- ~~**A4 通常の build でも `__yuzu/build_id`（ビルド時刻）が dist に出る**~~ → v0.18.1 で修正済み

### B. 黙って誤った結果を出す不具合

- ~~**B1 サイト通し番号の本文キャッシュが、他ページの順序の変更で無効化されない**~~ → v0.18.1 で修正済み
- ~~**B2 プロジェクトの祖先に隠しディレクトリがあると、dev が黙って再ビルドしない**~~ → v0.18.1 で修正済み
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
  - 検索処理は `BuildCache::search` のヒットを先に判定し、全ページがヒットすると
    Tokenizer を呼ばない（`yuzu-index/src/builder.rs:186-197`）。原稿を変えずに辞書だけを
    差し替えると、旧辞書で数えた節がそのまま再利用される
  - 対処案: 辞書の指紋が変わったら、検索キャッシュを無効にして全ページを
    トークナイズし直すことを必須にする。再ビルドのたびに辞書の指紋を比べ、違えば
    セッションを作り直せば満たせる（envKey が辞書の指紋を含む = `commands/build.rs:318` の
    `env_key`。envKey が変わるとキャッシュ全体が無効になる）。Tokenizer をモデルの
    ハッシュをキーにして保持するだけでは、キャッシュに当たったページが残るので足りない
    （PR #29 のレビュー指摘）
- ~~**B6 `yuzu fmt` / `lint --fix` が原稿を直接上書きする**~~ → v0.19 Phase 85
- ~~**B7 外部リンク検査の curl が `~/.curlrc` を読む**~~ → v0.18.1 で修正済み
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
    テストを足した）。ci.yml の e2e に「編集すると再ビルドが起きる」・dev / preview の
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
  → v0.19 の方針に追加済み（リリース後は「完了済み: v0.19」に要約）
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

## 10-12 レビュー

**概要**: v0.19.0 のリリース後（10-12）に、Rust の実装をメンテナンスのしやすさの観点で
見直した結果。上級プログラマーの 3 つの人格（関数の長さを見る「分割派」・1 つの変更で触る
場所を数える「変更コスト派」・不正な状態を型で書けなくしたい「型で語る派」）が並行して読み、
2 巡目で互いの指摘に同意・反論した。既知の「急がないコード整理」と 10-07 見直しの E は除いた。

関数の長さは大きな問題ではなかった（yuzu 本体の関数は平均 20 行台。100 行を超えるのは既知の
4 本のほかに 6 本）。3 者が一致したのは、**同じ判断が 2〜3 か所に別々に書かれていて、手で
揃えている**箇所で、そこから実際の不具合が出ていた。最優先の 3 件（R1〜R3）はどれも手元で
再現した。

確認の仕方は 10-07 節と同じ 3 段階（**10-12 に確認** / **実機未確認** / **未確認**）。
ファイルの行番号は 10-12 時点。

### A. 不具合を再現したもの（3 者が一致した最優先）

- ⬜ **R1 dist に何を出すかの判断が 3 か所にあり、食い違っている**（工数: 中。**10-12 に確認**）
  - 書き出し（`yuzu-render/src/pipeline.rs` の syntect.css・sitemap・build_id）・その補集合の
    `unpublished_outputs`（同 36-54）・リンクの有効ターゲットの `GENERATED`
    （`yuzu-core/src/linkcheck.rs:30-31`）が別々の一覧を持つ
  - 再現した食い違い: `output.clean = false` で `markdown.highlight.enabled = false` にして
    `--force` すると `_assets/css/syntect.css` が残り、その後の build でも消えない /
    `build --watch` → `--force` → 通常の build の後も `__yuzu/build_id`（ビルド時刻）が残る /
    check は `/404.html` へのリンクを broken-link にする（404.html は必ず出力している。Phase 85 の
    build の件数にも入る）/ `llms.enabled = false` でも `/llms-full.txt` へのリンクが check を通る
  - 同じ種類の食い違いを 63c361e・f67e2ab（PR #42）・1e1a69b（Phase 85）で直してきた。D8
    （favicon・robots.txt）で生成物を足すと、また 3 か所を触る
  - 対処案: 出力物を enum にし、パスと「今回出すか」を網羅 match で決める（ワイルドカードを
    書かない）。書き出し・掃除・check の有効ターゲットはすべてこれを通す。全 variant の配列への
    登録漏れはテストで確認する。「表」にすると利用側がパス文字列を解釈し直すので、行を足しても
    利用側の漏れを検出できない（型で語る派の反論で enum に決まった）
- ⬜ **R2 build と check で、検査の一覧と抑制の解釈が別々**（工数: 中。**10-12 に確認**）
  - check は `commands/check.rs:26-81` で 11 系統、build は `commands/build.rs` の
    report_broken（3 系統）と route-case-conflict の独自の判定（`lint.rules` を直接読む）
  - 再現: `Guide.md` と `guide/index.md` に `lintDisable: [route-case-conflict]` を書くと、check は
    抑制するが build は警告を出す（docs `reference/rules.md` の「lintDisable で抑制できます」と
    食い違う。Phase 85 の実装の不具合）
  - `Diagnostic` はルール ID（`&str`）と深刻度を別々に持ち、組み立ての約 20 か所で
    `rules::X.id` と `rules::X.severity` を対にして手書きしている。`commands/diag.rs:24` は
    設定由来の診断の深刻度を `Warning` と直書きしている。同じことをする局所ヘルパが 6 つあり、
    引数の順序がすべて違う
  - 対処案: 診断を集める関数を cli に 1 つ置き（check は全部を報告、build はルールで絞って件数を
    出す）、抑制は core の `apply_suppressions` を通した後の型でしか読めないようにする。
    `Diagnostic` は `Rule` を持ち、深刻度はそこから決める（`Diagnostic::new(rule, rel, span, message)`）
  - 補足: 壊れた openapi は 2 回目以降の build（本文キャッシュに当たる）で何も出ない。仕様の中身の
    検証は Phase 85 の判断で build の件数に入れていないので不具合ではないが、診断を 1 つにまとめる
    ときに入れるかを決め直す
- ⬜ **R3 設定値と CLI の引数を入口で検証していない**（工数: 小〜中。**10-12 に確認**）
  - 再現: `search.page = "../search"` で検索結果ページが出ないのに build も check も exit 0
    （`yuzu-core/src/urlpath.rs:129` が不正値を黙って None にする。`markdown.glossary.page` も同じ
    関数）/ `yuzu dev --host localhost` は `invalid IP address syntax` で exit 2（エラー文言は
    指定していない `dev.host` を指す。parse は `dev.rs:68` と `preview.rs:44` の 2 か所）/
    `yuzu build --port 9999 --host nonsense` は `--watch` が無いと黙って無視して exit 0
  - `theme.toc.levels` の不正値もビルド時の位置なしの警告だけで、check を通る（**実機未確認**）
  - 起動時に固定する設定（`pin_restart_only`。`build.rs:258-297`）と、配信の設定の組み立て
    （dev.rs・preview.rs）が別々で、テストは `dev.host` と `dev.allowed_hosts` を見ていない
  - 対処案: 設定の読み込み直後に core の検証を呼んで、位置付きの設定エラーにする（yuzu-config に
    型を置くと、config は core の規則を使えないので実装が 2 つになる。変更コスト派の反論で決まった）。
    clap で `--host` を IP アドレスの型にし、`--port` / `--host` は `--watch` を必須にする。起動時の
    設定を 1 つの構造体にまとめ、dev / preview / `pin_restart_only` が同じものを使う

### B. 同じ判断が 2 か所にあるもの（実害の報告は無い）

- ⬜ **R4 page と 404 のテンプレート文脈を 2 回書いている**（工数: 小。3 者が指摘）
  - `pipeline.rs:305-334`（page / search）と `:377-402`（404）に、同じ式のキーが 11〜16 個ある。
    minijinja は未定義の変数を空・偽として通す（`templates.rs` が undefined の扱いを設定していない）
    ので、片方に足し忘れても 404 ページだけ機能が黙って消える。この 2 か所は 7 つのコミットで
    両方を触っている
  - 対処案: ページによらない値を 1 つの値にまとめてループの前に作り、`context!` の `..` で合成する。
    利用者が theme/ で上書きしたテンプレートを壊さないよう、変数の名前は変えない
- ⬜ **R5 URL の分類の手順が linkcheck と render に 1 つずつある**（工数: 小〜中。2 者が指摘）
  - `linkcheck.rs:119` の check_one と `yuzu-render/src/urls.rs:83` の rewrite が、同じ 6 分岐
    （空・`#`・外部・`/` 始まり・`.md`・拡張子）を同じ順に書き、一致はコメントで縛っている。
    ddfd6aa はこの順序のずれを直したもの
  - 対処案: urlpath に分類の enum を 1 つ置き、両側は網羅 match で扱う

### C. 構造の整理（防げる不具合は少ない）

- ⬜ **R6 render_body_html（292 行）のノード生成と、記法ごとの組み替え**（工数: 中）
  - `NodeHtmlBlock` の生成が 5 か所・`arena.alloc(AstNode::new(..))` が 4 か所、同じ 11 行の
    クロージャを 2 回定義している（`markdown/mod.rs`）。タブ・折りたたみ・用語集は解析側が
    tabs.rs / collapse.rs / glossary.rs にあるのに、組み替えだけが mod.rs にあり、記法の変更が
    2 ファイルにまたがる
  - 対処案（分割派と変更コスト派が合意）: ノード生成を 1 つの関数にまとめ、組み替えを各記法の
    モジュールへ移す。**適用の順序は本体の 1 関数に残す**（タブの判定は展開前・分量は展開後に
    数える・用語集は他の適用の後、の制約が記法をまたぐため）。本体は約 150 行になる見込み
  - あわせて CLAUDE.md の「`markdown/mod.rs` が comrak を触る唯一の場所」を「markdown/ 配下だけ」に
    直す（実際は 6 ファイル。**10-12 に確認**）
- ⬜ **R7 合成ページの `Page.src` は実在しないパスで、除外は否定のフィルタに頼っている**
  （工数: 中。**10-12 に確認**（件数））
  - `!p.is_generated()` / `!page.is_generated()` が 21 か所。書き込みや診断の経路を足すときに
    フィルタを書き忘れてもコンパイルが通る
  - 対処案: `PageOrigin { File(PathBuf), Generated(GeneratedKind) }` にし、ファイルのパスは
    `source_file() -> Option<&Path>` からしか取れなくする
- ⬜ **R8 出力先と原稿への書き込み口を clippy で縛る**（工数: 小。**10-12 に確認**（件数））
  - 「書き込みは `write_under` / `rewrite_source` だけ」はコメントで縛っている。今は守られている
    （テスト以外の `fs::write` / `remove_dir_all` は実装そのものと雛形の 4 か所だけ）
  - 対処案: `clippy.toml` の `disallowed-methods` で禁じ、4 か所だけ許す（前例は `main.rs` の
    `print_stdout`）
- ⬜ **R9 CACHE_FORMAT_VERSION の上げ忘れをテストで検出する**（工数: 小）
  - 上げるのはコメントとスキルの手順だけで、v13 は上げ忘れて後から計上した（fb56ef5）。
    Frontmatter は `serde(default)` なので、フィールドを足すと旧キャッシュが既定値のまま黙って読める
  - 対処案: 本文のスナップショットのハッシュを `CACHE_FORMAT_VERSION` の隣の定数と照合する
    テスト、またはキャッシュする型のフィールド名の指紋をヘッダーに入れる
- ⬜ **R10 小さい整理**（工数: 各小）
  - `build_once` から、テストの無い同義語の合成と丸め（IndexParams の組み立て）を純関数で出す
  - cli の run 関数（check・lint・search・dev・llms）にテストが 0 件。`lint --fix` の収束ループを
    `fmt.rs` のように純関数に出してテストする
  - `yuzu_config::load` の保護対象の配列（`resolve.rs:84-89`）とエラー文言の match が別で、
    `_ =>` が `.yuzu` の文言になっている。文言を配列に入れる
  - tankan: 前置き行（`%%{..}%%`・frontmatter・コメント）を飛ばす処理が 11 か所、SVG のルート要素が
    9 か所にコピーされ、frontmatter の `title:` の扱いが 4 通りに分かれた（引用符を外すのは packet
    だけ）。図種を足すたびにコピーが増える
  - mikan の `make_excerpt`（135 行）の窓を選ぶ処理を出し、総当たりの参照実装と比べるテストを書く
  - `BuildCache::meta(rel, source_hash)` は 2 つの `&str` を取り違えても通る（`SourceHash` 型。
    既知の「sha256 の 4 回計算」と一緒に）。`collect_sections` の `sections.last_mut().unwrap()`
    7 か所は状態の構造体にすれば消える。drafts・force の bool が 7 段そのまま渡る

### 判断の記録

- **分けなくてよい**: `build_once`（処理の順序がそのまま仕様。分けると状態を引数で運ぶだけ）・
  `yuzu_config::load`（順序に意味のある検証が並ぶ）・`check::run`（並びが「check が何を見るか」の
  一覧として読める）・表を引くだけの関数（`syntax_message`・`is_css_color_name`）
- **型にしなくてよい**: `RenderCtx` / `IndexCtx` の Option 3 つ（組み合わせはどれも有効）・
  mikan の検索の呼び出し順の typestate（JS と wasm の境界で型が消え、未ロードでも結果が減るだけ）・
  `base_url` の型（読む場所が 10 か所ほどあり、得られるのは R3 の構造体で足りる）
- **良くできている点**: `markdown_options` と分割代入テスト・設定の `table_codec!`（フィールドを
  足すとコンパイルエラーかテストで止まる）/ `GeneratedKind` の網羅 match と rules.rs の const fn /
  `Cx`（生成を 1 か所に限定）と `PageCodeRenderer`（並列ループで共有するとコンパイルエラー）/
  `apispec/render.rs` は 1900 行のファイルでも最長 59 行、`fmt.rs` は純関数とテスト 8 本

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

- **client 描画の Mermaid の書き間違いを check で報告する**（v0.19 Phase 85 から回した）— 今は
  既定の client では build も check も何も言わない。tankan のパースだけ通し、未対応
  （`is_unsupported`）以外の構文エラーを抑制できる warning で報告する案。tankan が未対応の
  書き方を誤りとして報告しないよう、切り分け方を決めてから扱う
- **mermaid 12 系への更新** — v0.18.1 で 11 系に留めた（12.x は tankan の互換対象の版と
  あわせて決める）。Phase 82 の照合で、11 系の最新の 11.17.2 が束ねる DOMPurify・js-yaml・
  KaTeX に勧告が 5 件見つかり、影響しない理由を書いて `scripts/vendor-advisories.sh` で
  除外している（10-10）。12 系へ上げるときに除外を外して照合し直す
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
  - **v0.18.1**: 同梱 mermaid（11.17.2）・KaTeX（0.18.11）の脆弱性勧告への対応と、
    黙って誤った結果を出す不具合 4 件の修正 — サイト通し番号の本文キャッシュ・
    隠しディレクトリ配下での監視・通常 build の `build_id`・外部リンク検査の `.curlrc`
  - `CACHE_FORMAT_VERSION` 22 → 23（Phase 78 の 1 回）
- **v0.19**（Phase 81〜86）配布物と第一印象
  - 第三者のライセンス表記 — リリースのアーカイブ（cargo-about）・利用者のサイトの dist
    （mermaid・KaTeX と検索の wasm・分かち書きのモデル）・crates.io の 3 crate
  - 脆弱性の窓口（SECURITY.md・private vulnerability reporting）と依存の監視（dependabot・
    cargo-deny・同梱の vendor 資産の勧告の定期照合）
  - 公開範囲の制御 — 原稿の `.md` の配信を止める設定（`llms.page_md` / frontmatter `pageMd`）・
    dev / preview の Host / Origin の検査（`dev.allowed_hosts`）・docs「社内で公開する」
  - docs と雛形を実際の動作に合わせる — 設定例の TOML 化・雛形 deploy.yml はリリースの
    アーカイブを SHA256SUMS で照合して使う・用語集は同じ階層の末尾・CLI のヘルプの日本語化・
    FAQ と目次の見直し・docs サイトはタグで公開
  - build が壊れたページを黙って出さない — リンク切れ・取り込みの失敗の件数を最後に 1 行で・
    原稿 0 件の警告・ログの時刻と色・大文字小文字を区別しない FS の孤児掃除・
    `route-case-conflict`・`check --no-fmt`・原稿の書き込みを一時ファイル経由に
  - dogfooding — 利用者のサイトに出るライセンス表記の docs・dev / preview がテキストファイルに
    `charset=utf-8` を付ける
  - **非互換**: `base_url` / `--base-url` の `.` で始まる値は設定エラー / `yuzu check` の画像の
    判定が build のコピー対象に揃って厳しくなる（隠しファイル・`input.ignore` の対象・
    シンボリックリンク・大文字小文字だけが違う参照）/ 用語集が同じ階層の末尾に並ぶ /
    ログの時刻は `dev`・`build --watch` だけ・パイプ先に色を付けない・キャッシュの内訳は `-v`
    だけ / docs サイトはリリースのタグで公開
  - `CACHE_FORMAT_VERSION` 23 → 24（Phase 83 の 1 回）

検索エンジン本体 **mikan**（旧 yuzu-index-format）と wasm ラッパ **mikan-wasm**
（旧 yuzu-search-wasm）は v0.7 リリース後に yuzu- プレフィックスを外して改名し、
mikan は crates.io で単独公開している（tankan と同じく独立バージョン）。

各版の Phase 内訳:

<details>
<summary>完了済み: v0.19（Phase 81〜86）の内訳</summary>

テーマは「**配布物と第一印象**」。10-04 の見直しで 3 人が合意した優先順位の先頭
「配布物として必要なもの」に、10-07 の見直しで見つかった「公開してはいけないものが外へ
出る」3 件と、初めて使う人が最初に踏む食い違いを加えて 6 つの Phase で消化した。先に
v0.18.1（同梱 mermaid・KaTeX の勧告対応と不具合 4 件）を出している。判断点は各 Phase の
着手時に決めた（Phase 84 は質問に応答が無く、推奨案を計画に書いて計画の承認で確定。
Phase 85 の 1 つは 2 回尋ねて応答が無く、実装の後に改めて尋ねて確定した）。
`CACHE_FORMAT_VERSION` は Phase 83 で frontmatter に `pageMd` を足したので 23 → 24 の 1 回。
ブラウザ・macOS・GitHub の実リポジトリが要る確認はユーザが行った。

- **81 第三者のライセンス表記** — アーカイブ用は cargo-about（0.9.2）で release.yml の
  `licenses` ジョブがリリースのたびに作る（コミットしない）。dist 用は資産の隣にテキスト 2 枚
  （`_assets/vendor/`・`_search/` の `THIRD-PARTY-LICENSES.txt`）で、バイナリに埋め込むので
  生成してコミットする。mermaid は npm の tarball に切り替え、同梱の source map から束ねた
  60 パッケージの LICENSE を集める。vendor の README は配信から外し、3 crate に LICENSE。
  レビュー指摘で、cargo-about が本文の無い crate で SPDX の雛形に戻る（`--fail` でも止まらない）
  ことへの補い（`about.toml` の clarify・`licenses/supplements`）を入れた
- **82 脆弱性の窓口と依存の監視** — SECURITY.md（受領 7 日・修正 90 日が目安）。dependabot は
  月次で、本文 HTML が変わりうる組とそれ以外に分ける。cargo-deny は licenses・bans・sources を
  PR ごと、advisories を deps.yml（週次）に分けた。初回の勧告は syntect の `default-fancy` を
  やめて 10 crate を依存から外して解消し、bincode だけ理由付きで除外。vendor 資産の勧告も
  定期照合し、mermaid 11 系で直せない 5 件は「GHSA とパッケージ@版」の組で除外（mermaid 12 で
  見直す）。検索 wasm に入る crate の版の照合もここで足した
- **83 公開範囲の制御** — 原稿の `.md` の配信をサイト全体（`llms.page_md`）とページ単位
  （`pageMd`）で止められる（止めたページはコピーボタンも出さず、llms.txt は HTML を指す）。
  dev / preview は localhost・IP 以外のホスト名を `dev.allowed_hosts` に無ければ 403
  （DNS リバインディング対策。WebSocket は Origin も）。docs「社内で公開する」。レビュー指摘で、
  止めた生成物を出力マニフェストの有無に関係なく消すようにした
- **84 docs と雛形を実際の動作に合わせる** — 設定例を TOML に揃え（否定 grep で CI が縛る）、
  既定値・監視範囲・README の説明を直した。雛形に `content/guide/index.md`、用語集は同じ階層の
  末尾（レビュー指摘で index として置いた用語集も）。雛形 deploy.yml はリリースのアーカイブを
  SHA256SUMS で照合して使う。`-h`・`-V`・`help` を日本語に、`base_url` の `.` 始まりは設定エラー。
  docs に「インストールと最初のサイト」「見た目の調整」「よくある質問とトラブル」と図の選び方。
  docs サイトはリリースのタグで公開する
- **85 build が壊れたページを黙って出さない** — check と同じ関数を build のページ集合で回し、
  リンク切れ・取り込みの失敗を最後に 1 行で知らせる（終了コードは 0 のまま。2 回目以降の
  build でも出る）。原稿 0 件の警告・Mermaid の警告にページ名・ログの時刻は dev / watch だけ・
  色は端末のときだけ。大文字小文字を区別しない FS では、旧出力が今回と同じファイルなら消さずに
  大文字小文字を付け直す（same-file で判定。macOS の実機で確認）。`route-case-conflict`・
  check の画像の判定を build のコピー対象に・`check --no-fmt`・原稿の書き込みを一時ファイル経由に。
  レビュー指摘で、一時ファイルを固定名にしない・作成時から 600 にする。client 描画の Mermaid の
  構文チェックは v0.20 へ回した
- **86 dogfooding** — 多くは 81〜85 で前倒しで済んでいた。利用者のサイトに出るライセンス表記を
  docs に書き、docs サイト自身でも照合。ブラウザでの確認で、dev / preview がテキストファイルを
  文字コード無しで返して日本語が化けるのを見つけ、`text/plain`・`text/markdown` に
  `charset=utf-8` を付けた（レビュー指摘で HTML には付けない。nginx の例も同じ）。雛形
  deploy.yml を実際の GitHub リポジトリで動かし、macOS で孤児掃除を確かめた

</details>

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

リリース後の **v0.18.1**（10-10）で、10-07 の見直しで 3 者が一致した 2 つを v0.19 より先に出した。

- **同梱資産の更新** — mermaid 11.16.0 → 11.17.2（08-06 公開の勧告 5 件。11.16.1 で修正）・
  KaTeX 0.17.0 → 0.18.11（GHSA-238p-pmpm-9mq7。0.18.2 で修正）。12.x / 0.19 は v0.20 以降で
  tankan の互換対象の版とあわせて決めるので、それぞれ 11 系・0.18 系の最新に留めた。
  npm の integrity と照合した tarball から取り、mermaid は CDN のファイルが tarball 内の
  ものと同一であることを確かめた。KaTeX 0.18.0 の CSS クラス接頭辞の変更は、テーマが参照する
  `.katex-display` に影響しない。mermaid 11.17.0 でクラス図の既定のレンダラが変わった
  （client 描画の見た目）
- **黙って誤った結果を出す不具合 4 件**（10-07 見直しの B1・B2・A4・B7）
  - サイト通し番号の本文キャッシュが、他ページの `order` の変更や図⇄表の差し替え（個数が
    同じ）で無効化されず古い図番号が残った。routesKey の組み立てを core の
    `SiteModel::routes_key` にまとめ（cli とテストが同じ関数を通る）、各ページの採番開始位置
    （`crossref_offset`）を入れた
  - `WatchIgnore` が絶対パスの全構成要素で隠しディレクトリを判定していたため、ルートの祖先
    （`~/.config/notes/`・`.claude/worktrees/…`）に `.` 始まりがあると dev / `build --watch` が
    黙って再ビルドしなかった。監視ルートからの相対パスで判定する。テスト専用の回避策
    `visible_tempdir` は削除
  - 通常の build でも `__yuzu/build_id`（ナノ秒の時刻）を書いていた。`build --watch` の
    ときだけ書く
  - 外部リンク検査の curl が `~/.curlrc` を読み、`--fail` があると 4xx が skipped に化けた。
    先頭に `-q`、`--proto` / `--proto-redir =http,https` を足した
  - 4 件とも、修正前の挙動で失敗するテストを足してから直した。ユーザが雛形でクラス図・
    状態遷移図・数式などをライト / ダークで確認した（問題なし）

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
