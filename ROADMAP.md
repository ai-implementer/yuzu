# ロードマップ

yuzu の開発計画（今の版の Phase・候補）と、これまでの版の 1 行の要約。**このファイルが Phase 状態の正**
（README には現在の版と概要だけを置く）。リリース済みの版の内訳と、片付いたレビュー・見直しの記録は
[ROADMAP-history.md](ROADMAP-history.md)。

## 現在: v0.20（Phase 87〜93）

**v0.19.1 まで公開済み**。kabosu 0.2.0 / tankan 0.2.1 / mikan 0.2.2 は crates.io で公開済み
（yuzu のリリースとは非同期。kabosu の publish 前に fuzz を実行する規律は CLAUDE.md にある）。

次は **v0.20**（テーマ「**最初の利用者が当たる所を直し、検証の穴を埋める**」）を 7 つの
Phase で進める。10-14 に未着手の 40 件と候補を一覧にして現状を確かめ直し、テーマの案
（A 最初の利用者 / B 外部への露出 / C 保守の仕組み）からユーザが「A を中心に、C から CI の
穴と fuzz だけを取り込む」案を選んだ。B（外部への露出）は v0.21 の予定にする。

### v0.20 の方針

利用者は今も 0 人なので、最初に来た人が自分の docs サイトと原稿で当たる所（frontmatter の
誤りの表示・検索の順位・読者側の UI・他ツールとの違い）を先に直す。あわせて、配布している
macOS・Windows のバイナリが CI でテストされていない穴と、fuzz が手動の kabosu だけの穴を埋める。

以下は策定の詳細。

- 動機
  - 10-04・10-07 の見直しで 3 者が合意した順序は「品質を先に片付けてから露出」。v0.19 で
    配布物と第一印象を扱ったが、利用者に見える未着手が残っている（下の「現状の確認」）
  - 10-12 に frontmatter を読む serde_yaml_ng の依存（unsafe-libyaml）にメンテナンス終了の
    勧告が出て、`deny.toml` で除外した（f37cf4a）。置き換えは frontmatter のエラー文言の
    改善と同じ箇所を触るので、一緒に行えば除外を外せる
  - 配布しているのは 4 ターゲットなのに、テストは Linux だけ。fuzz は v0.18 Phase 77 で
    tankan の不具合 2 件を 90 秒以内に見つけたが、定期実行が無く対象も kabosu だけ
- ゴール: frontmatter の書き間違いが日本語・ファイル上の行・全件で出て、勧告の除外が外れて
  いる。docs で「設定」と検索すると、読者のブラウザでも設定のページが上位に出る。favicon・
  フッター・ヘッダーのリンクを設定で足せて、読者側の UI の穴（検索欄・文言）が埋まっている。
  既存の Markdown を持ち込んだ人が黙って壊れたサイトを出さず、他ツールから来た人が違いを
  1 ページで読める。Windows・macOS で `cargo test` が CI で通り、fuzz が定期的に実行されて
  mikan・tankan がハング・メモリ超過で止まらない
- 変わるもの: frontmatter の読み込み（YAML パーサ）・検索のスコア（wasm 側）・テーマ
  （テンプレート・CSS・JS。スナップショットが変わる）・設定キー（見た目・`search: false`）・
  docs・CI。`CACHE_FORMAT_VERSION` は 88（frontmatter の読み方）・89（frontmatter の
  `search`）・91 の PR (1)（描画される href が変わるなら）で上がる見込み。検索インデックスの `FORMAT_VERSION` は据え置く（スコアの計算を
  wasm 側に置けば、新しい manifest と古い wasm の組でも壊れない）。MSRV は 88 の判断点
- Phase は「87 Windows・macOS のテスト → 88 frontmatter の読み込み → 89 検索の順位 →
  90 読者側の UI と見た目の設定 → 91 既存の原稿から始める人・他のツールから来た人へ → 92 fuzz →
  93 dogfooding と整理」の順
  - 87 を先頭にするのは、以後の Phase の PR を Windows・macOS でもテストするため（落ちる件数が
    分からないので、早く知るほうがよい）
  - 88 を 89 より前に置くのは、勧告の除外を早く外すため（どちらも `CACHE_FORMAT_VERSION` を
    上げる見込み）
  - 91 の docs は 90 で足す設定キーを書くので 90 の後。92 は利用者に見えないので後ろ
  - 着手時に判断点を決めてから実装する。応答が無いまま推奨案で進めた判断点は「仮決め」と
    PR と ROADMAP に書く（10-07 見直しの F2）
- 10-14 に PR #53 のレビュー指摘（延期候補とリリース判定の食い違い・favicon の判定の循環）と
  追加機能の提案を、3 つの人格（計画の一貫性と検証の手順を見る「整合派」・初めての利用者の
  導入経路を見る「導入派」・1 人で保守する範囲と戻せない決定を見る「範囲派」）で 2 巡協議し、
  リリース判定・延期候補・Phase 89・91・92 を直した。提案された機能（既存 Markdown 向けの
  `init`・公開物の詳細な JSON・PR 検証 CI の生成）は判定に入れず、最初の外部利用者を観察して
  から判断する候補にした（「v0.21 以降の候補」）。同じ日の再レビュー（検索資産の照合に同梱の
  JS が入っていない）も同じ 3 人格で 2 巡協議し、照合の対象・トップページの警告の条件・公開物の
  要約・fuzz の受け入れ条件を具体的にした
- **リリース判定**（すべて満たしたらリリースする。どれもタグの前に確かめられる）
  - `deny.toml` から RUSTSEC-2024-0449 の除外が外れ、deps.yml が成功している
  - タグを打つコミットで、ci.yml の Windows と macOS の `cargo test` が通っている
  - docs で「設定」と検索して、URL が `/reference/config/` で始まる結果が上位 3 件に入る
    （ci.yml の docs ゲートで照合する。照合はネイティブの `yuzu search` なので、コミットした
    検索 wasm と同梱の JS が今のソースから作られていることも 89 で足す照合で確かめる）
  - `--base-url` 付きで作った docs の dist で、`rel="icon"` の href が base の下を指し、その
    ファイルが dist にある（ci.yml で照合する。公開サイトはタグの後に更新されるので、公開 URL
    の確認はリリース後の作業にして判定には入れない）
  - fuzz.yml に定期実行（schedule）があり、定期実行と同じ秒数の手動実行で、mikan の対象が
    成功し、tankan の対象が指定した秒数を最後まで実行して、ハング（`timeout-*`）・メモリ超過
    （`oom-*`）の artifact が 0 件。tankan の panic（`crash-*`）が残るなら件数をリリースノートに
    書く（panic は yuzu が回収してクライアント描画に切り替える）。panic を受け流して最後まで
    実行できない（92 の判断点）なら、tankan の対象を定期実行から外してリリースノートに書く
  - 仮決めのままの判断点が無い（残すならリリースノートに書く）
- **ユーザの作業**

  | 作業 | Phase | 時期 |
  | --- | --- | --- |
  | 各 Phase の判断点を決める | 87〜93 | 着手時 |
  | Windows・macOS のランナーの料金を確かめる（PR ごとに実行してよいか） | 87 | 87 の着手時 |
  | 検索 wasm を作り直した後に、ブラウザで「設定」の順位を確かめる | 89 | 89 のマージ前 |
  | 手元の preview での確認（favicon・フッター・キーボード操作・検索欄。iOS は可能なら） | 90 | 90 のマージ前 |
  | tankan / mikan のパッチ版を crates.io へ publish（92 で直した場合。`cargo login` が要る） | 92 | 92 のマージ後 |
  | 公開サイトの favicon を確かめる | 90 | リリース後 |
  | 名前「yuzu」の調査（商標・区別のための副名を付けるか。10-07 見直しの F7） | v0.21 の準備 | v0.20 の期間中 |

- **時間が足りなければ v0.21 へ移すもの**: 87 の Windows での CLI の e2e / 90 の Admonition の
  ラベルとテーマ切替の 3 状態 / 91 の雛形の原稿の作り直し / 92 で tankan に見つかった panic の
  修正（判定のとおりリリースノートに書く）/ 93 の R5
- **v0.21 の予定**: 外部への露出（テーマの案 B）。F7 の結論を受けて、英語の README・他ツールとの
  比較・cargo-binstall のメタデータ・互換性の方針と CHANGELOG・CONTRIBUTING・リリースごとの
  利用数の記録を扱う（10-04 見直しの「外部への露出」「外部からの貢献の受け口」「0.x の互換性方針」、
  10-07 見直しの F1）

### 現状の確認（10-14）

- 利用の状況: Release の各アセットのダウンロードは v0.18.0・v0.19.0・v0.19.1 とも 1 回ずつで、
  release.yml 自身が SHA256SUMS を作るために取得した分（v0.18.1 の Linux 版だけ 3 回）。
  直近 14 日の閲覧は 327 回・2 人。star・fork・issue は 0。clone の数（14 日で 1,045 回）は
  Actions の checkout を含むので、利用の指標には使えない
- 未着手のうち、ファイルや公開サイトで確かめられるものを見直した
  - 解消していた: 10-04「依存とビルド設定」の yuzu-index の build.rs（Phase 81 で付いた）と
    sha2 の 2 版（0.11.0 だけになった）。10-04「運用文書の古い記述」の release スキルの記述
  - 再現した: D2（「設定」の 1 位は同義語の設定例でスコア 7.29、設定のページは上位 3 件に
    入らない）/ D8（docs サイトの `/yuzu/favicon.ico` が 404、テンプレートに `rel="icon"` が
    無い）/ D10（ヘッダーの検索欄に `aria-label` が無い・docs に「Cmd+K」の表記）/ D11
    （「（コンソール参照）」）/ B8（`markdown/glossary.rs:168` が `## {term}` を生のまま埋める）/
    雛形のページ数が build 5・検索 4・check 3 / R7 の否定のフィルタは 42 か所 / E4 の手書きの
    `/` 区切りは 11 か所
  - 再現しなかった: 2 文字の語の誤字補正（「単語」で「英語」が混ざる）は今の docs では出ない
    （ほかの語は試していない）
- PR #53 の協議で見つかった、既存の Markdown を持ち込んだときに黙って壊れる所（導入派が
  仮のリポジトリで見つけ、司会が手元で再現した）: `input.dir = "docs"` で `docs/README.md` は
  `/README/` になり、`dist/index.html` が無いのでサイトの `/` が 404。`input.dir` の外を指す
  `../../README.md` は `resolve_relative`（`yuzu-core/src/urlpath.rs:98-113`）がルートより上の
  `..` を捨てるので `/README/`（= `docs/README.md`）に解決され、`check --no-fmt` は
  「問題ありません」と出る。原稿 0 件の警告はルート直下の `.md` しか数えない

### 87 Windows・macOS のテスト ✅

**概要**: 配布している 4 ターゲットのうち、テストしているのは Linux だけ。ci.yml に
Windows と macOS の `cargo test` を足し、落ちたテストを直す。

- 現状: ci.yml の check ジョブは ubuntu-latest だけ。release.yml は 4 ターゲットでビルドして
  `--version` を確かめるだけ。Windows でのパスの区切り（`\`）・大文字小文字・シンボリック
  リンク・権限（600）の扱いは**未確認**。テストには `/tmp`・Unix の権限・symlink を前提に
  したものがある（`#[cfg(unix)]` で外しているものと、外していないものが混ざっている見込み）
- やること: windows-latest・macos-latest で `cargo test --workspace`（静的検査・docs・e2e は
  ubuntu のジョブのまま）。落ちたテストを「コードの不具合」と「テストの前提」に分けて直す。
  コードの不具合が出たら、v0.20 の範囲で直すか切り出すかをその場で決める。テストの前提で
  外したテストはこの節に記録し、Windows で動作が違うもの（権限・symlink 等）はリリースノートに書く
- 判断点: 実行の頻度（協議の推奨は PR ごと = 先頭に置く理由と揃う。公開リポジトリの標準
  ランナーは無料の見込みだが**未確認**なので、着手時にユーザが料金を確かめ、問題があれば
  main だけへ下げる）/ Windows で通らないテストを `cfg` で外すか直すか / CLI の e2e を
  Windows でも実行するか
- 工数: 中（Windows で何件落ちるかが分からない）
- 決定（10-15。ユーザが推奨案で決めた）と実装
  - **実行の頻度** — PR ごと（料金は確かめないまま推奨どおりに始めた）。ci.yml の `test-cross`
    ジョブ（windows-2025・macos-15 = release.yml の build と同じランナー）で
    `cargo test --locked --workspace --no-fail-fast`。所要は Windows 約 5 分 40 秒・macOS 約
    2 分 50 秒（check ジョブは約 4 分 30 秒）。`--no-fail-fast` は、最初の実行で yuzu-cli の失敗で
    止まり、後の crate の結果が出なかったため
  - **改行コード** — `.gitattributes` で LF にそろえ、Windows でもテストの入力とスナップショットを
    同じバイトで取り出す。CRLF の解釈を試す kabosu の toml-test と、上流のライセンス文を写した
    もの（CRLF が混ざる）は `-text` でバイトのまま残した（既存のファイルは renormalize で変わらない）
  - **CRLF の原稿** — Windows の git は既定（`core.autocrlf=true`）で原稿を CRLF で取り出すので、
    `yuzu fmt --check` と `yuzu check` がすべての原稿を整形差分（error）にしていた（build はできる）。
    改行コードだけの違いは差分にせず、`yuzu fmt` は原稿の改行コードで書き戻す（`LineEnding`・
    `same_ignoring_line_endings`。CRLF と LF が混ざった原稿も差分にしない）。原稿の改行コードへ
    戻すのは frontmatter の後ろだけで、frontmatter は改行コードも原稿のバイトのまま残す（PR #54 の
    レビュー指摘。全文を戻していたため、LF と CRLF が混ざった属性行がそろっていた）
  - **通らないテスト** — 「コードの不具合」と「テストの前提」に分け、不具合は直し、前提は外して
    ここに記録する。CLI の e2e を Windows で実行するのは v0.21 へ移せる候補のまま（行っていない）
- 結果: macOS は最初の実行ですべて通った。Windows は 2 回の修正で通った
  - **Windows で動作が違った不具合（直した。リリースノートに書く）**: 人向けの診断のパスが
    `content\guide/x.md` のように `\` と `/` の混ざった表記になっていた（json・github 形式と同じ
    `/` 区切りにした）/ 合成ページの route の設定値に `\` を書くと、Windows では区切りになって
    別のページ（`a/b`）になり、ほかの OS では使えない文字のエラーになっていた（入口の検証で
    すべての OS の設定エラーにした）/ 上の CRLF の原稿
  - **テストの前提で外したもの**: `\` を含むファイル名の入力 1 件（Windows では `\` がパスの
    区切りでファイル名にできない。`routes.rs` のテスト）。外部リンクのテストは rel を `/` 区切りで
    比べるよう直した（外してはいない）。もとから Unix だけのテストは 20 件（シンボリックリンクと
    権限 600 を作るもの。Windows ではリンクの作成に権限が要る）で、preview のテストのモジュール
    もこれに合わせて Unix だけにした
  - **確かめていないこと**: Windows の実機での CLI の動作（e2e）。ログの `page=` は Windows では
    `\` 区切りで出る見込み（**実機未確認**。人向けの診断と違い、表記はそろえていない）

### 88 frontmatter の読み込み ✅

**概要**: frontmatter（と openapi / jsonschema の YAML）を読むパーサを、メンテナンスの
終わった unsafe-libyaml を使わないものへ置き換える。あわせて、frontmatter の型エラーを
日本語・ファイル上の行・全件で出す（10-04 見直しの持ち越し）。

- 現状
  - serde_yaml_ng 0.10.0（最新）を 3 か所で使う: frontmatter の読み込み
    （`yuzu-core/src/frontmatter.rs`）・`frontmatter-unknown-key` の lint（`lint.rs:555`）・
    openapi / jsonschema（`yuzu-render/src/apispec/render.rs`。`serde_json::Value` へ読む）
  - 依存の unsafe-libyaml に RUSTSEC-2024-0449（unmaintained）。`deny.toml` で理由付きで除外中
  - `order: "最初"` で `invalid type: string "最初", expected i64 at line 2 column 8`。英語・
    1 件ずつ・行は frontmatter の中での行（yuzu.toml のエラーは位置付き・日本語・全件）
  - 候補（10-14 時点）: serde-saphyr 2.0.0（serde でそのまま読める。MSRV 1.89 で、ワークスペースの
    1.87 を上げることになる）/ saphyr 0.2.1（MSRV 1.85。値の木を読むだけで型の検査は自前 =
    kabosu の decode と同じ考え方で全件・位置付きにしやすい）。どちらも MIT OR Apache-2.0
- やること: パーサを置き換え、3 か所を同じ読み込み口に通す。frontmatter はキーごとに型を
  検査して全件を日本語で出し、行はファイル上の行に足し直す。`deny.toml` の除外を外す。
  YAML 1.1 由来の書き方（`yes` / `no`・`on` / `off` の真偽値など）の解釈の差を確かめ、
  変わる書き方は docs に書く。読み方が変わるなら `CACHE_FORMAT_VERSION` を上げる
- 判断点: どのパーサにするか / MSRV を上げるか（README・release.yml・docs と ci.yml の
  msrv ジョブ（`ci.yml:876-882`）も直す）/
  解釈が変わる書き方を設定エラーにするか、警告にするか、黙って新しい解釈にするか
- 確かめること: docs・雛形・テストの原稿の frontmatter が置き換えの前後で同じ値に読めること。
  apispec のスナップショットが変わらないこと
- 工数: 中
- 決定（10-17 に着手時に確認したが応答が無く、推奨案で仮決めした。10-18 にユーザが推奨案のまま確定）と実装
  - **パーサ** — saphyr 0.2.2。serde-saphyr 2.0.0 は serde 経由なので最初の 1 件で
    止まり（全件にできない）、`draft: y` / `yes` / `on` を黙って true、`1_000` を整数として読む
    （10-17 に手元で確かめた）。saphyr は位置付きの値の木を読むだけで、型の検査は自前
    （yuzu-core の `yaml.rs` と `frontmatter.rs`）。frontmatter・未知キーの lint・openapi /
    jsonschema の 3 か所を `yaml.rs` の同じ読み込み口に通した
  - **MSRV** — 上げない（saphyr は 1.85。ロックされた依存はワークスペースの 1.87 で選ばれる）
  - **YAML 1.1 の書き方** — エラーにする。真偽値のキーの `yes` / `no`・`on` / `off`・
    `y` / `n` と、整数のキーの先頭 0（`010`）・`_` 区切り（`1_000`）は、置き換え前もエラーだった。
    文言を日本語にして書き直しを案内する（`true` か `false` で書く・先頭の 0 を外す）
  - **最後のキーのブロックスカラー（`|` / `>`）の末尾の改行** — YAML の仕様どおり残す。
    以前は切り出しで落としていたので、最後のキーのときだけ改行が無かった。docs に `|-` / `>-` を書いた
  - **全件の範囲** — 全ページ分を集めて出す。yuzu.toml と同じ「（N 件）:」＋
    `パス:行:列: 文言` の形で、行・列はファイル上の位置（列はバイト基準）。終了コードは従来どおり 2
  - **スカラーの解釈** — 置き換え前（serde_yaml_ng）と同じ境界にした（先頭 0 の 10 進数・`_` 区切りは
    文字列、`0b`・符号付きの 16 進 / 8 進は整数、u64 まで、`.inf` / `.nan` は JSON では null）。
    文字列のキーはスカラーの原文のまま（`title: 0x10` は「0x10」）
  - **別名の展開の上限** — saphyr は別名（`*x`）の展開を数えないので、入れ子の別名で指数的に
    膨らむ（10 個ずつ 7 段で 3.4 秒）。展開するノードを合計 20 万までに制限した（serde_yaml_ng にも
    同じ種類の上限があった）
  - `deny.toml` から RUSTSEC-2024-0449 の除外を外した。saphyr・saphyr-parser は crate の LICENSE が
    本文を持たない（上流の `.licenses/` を参照する）ので、公開コミットの原文を
    `licenses/supplements/` に置いた。`CACHE_FORMAT_VERSION` を 24 → 25 に上げた
- 結果
  - **同じ値に読めることの確認**: docs・雛形・テストの frontmatter 28 件と、openapi / jsonschema の
    ブロック・ファイルと kabosu の toml-test の JSON（計 212 件）を、置き換えの前後で読み比べた
    （一時的な比較用のテストで行い、コミットしていない）。食い違いは toml-test の JSON 3 件だけで、
    `"\u0000"` のようなエスケープした制御文字を以前は読めず、今は読める。apispec のスナップショットは
    変わらない
  - **置き換えで変わる書き方**（どれも黙って値が変わるものではない。最後の 1 つは上の決定のとおり）:
    `title:<タブ>x` は構文エラーになる（以前は通った。saphyr が `:` の後のタブを受け付けない）/
    配列の要素の値なし（`aliases: [a, ~]`）は型のエラーになる（以前は「~」という文字列だった）/
    全体を字下げした frontmatter は読める（以前は構文エラー）/ 最後のキーのブロックスカラーに
    末尾の改行が付く
  - **PR #55 のレビュー指摘（2 件。再現して直した）**: (1) saphyr はスカラーを原文のまま受け取る
    設定だとキーを引用符のスタイルとタグ込みで比べるので、`draft: true` と `"draft": false` が別の
    キーとして通り、後の false が勝って下書きのページが公開されていた。同名のキー（スカラーの原文が
    同じもの）を重複のエラーにした（frontmatter・未知キーの lint・API 仕様の 3 か所とも）/
    (2) 引用符を先に見て、明示のコアタグ付きの値（`!!bool "true"`・`!!int "42"`）を文字列にしていた
    （OpenAPI の `nullable: !!bool "true"` の「null 許容」が消えた）。タグを引用符より先に見て、
    タグに合わない値（`!!int "x"`・`!!bool yes`）はエラーにした（どちらも serde_yaml_ng と同じ）
  - **確かめていないこと**: YAML 1.2 の仕様の網羅（yaml-test-suite）。saphyr 側の試験に任せる
- 別件（Phase 88 の外）: 10-17 に ruzstd 0.8.2（mikan が分かち書きモデルの伸長に使う）へ
  RUSTSEC-2026-0344（小さな入力でメモリを使い果たす）が出て、deps.yml の advisories が失敗した。
  ユーザの判断で別の PR（#56。10-18 にマージ）にし、0.9.1 へ上げて検索 wasm と通知を
  作り直した。crates.io の mikan 0.2.1 は ruzstd 0.8 のままだったので、ruzstd の要件だけを上げた
  mikan 0.2.2 を公開した（#58。10-18。ソースは 0.2.1 と同一）
- 別件（Phase の外）: 10-18 にユーザの判断で MSRV を当時の最新の stable 1.99 へ上げた（ワークスペース
  1.87・kabosu と tankan 1.85 から。3 つとも同じ版）。cargo の依存の選び方はワークスペースでいちばん
  低い MSRV に合わせるので、1.85 のままだと globset 0.4.20・sha2 0.11.1（1.88）・comrak 0.56（1.89）・
  ordered-float 5.5（1.90）が上がらなかった。clippy の指摘 53 件（let チェーンへのまとめ直し 48 件・
  `is_multiple_of` 4 件・`as_chunks` 1 件）を直した。依存の更新は別の PR。kabosu・tankan・mikan の
  crates.io の MSRV は次に公開するときに変わる。リリースノートに書く

### 89 検索の順位 ✅

**概要**: docs で「設定」と検索して設定のページが上位に出ない（10-07 見直しの D2）のを直す。
あわせて、検索の対象から外す frontmatter と、watch 中に辞書を差し替えたときのずれ（B5）を扱う。

- 現状
  - 同義語の変形（`mikan/src/engine.rs` の `expand_queries`）は語ごとに重み 1.0 で足すので、
    同義語の組をそのまま書いた節が 2 語分の点を得る。docs の
    `synonyms = [["図", "ダイアグラム"], ["設定", "コンフィグ"]]` の例を書いた節が「設定」の
    1 位（7.29）になり、2 位以下は 2.2 前後。「シーケンス図」「ER図」でも同じ節が 1 位になる
    （10-07 に確認）
  - ページタイトルとの一致を優遇しているかは**未確認**。ページや節を検索から外す手段は無い
  - B5（10-07 見直し。**実機未確認**）: watch 中に辞書ファイルの中身だけを変えると、索引と
    ブラウザのトークナイザがずれる。モデルのバイト列は build のたびに dist へコピーするが、
    Tokenizer はセッションの `OnceLock`（`yuzu-index/src/builder.rs:94`）に残って作り直されない。
    辞書の指紋を含む envKey を計算する `BuildSession::new` は、watch では `yuzu.toml` の文字列が
    変わったときしか呼ばれない。検索のキャッシュは全ページが当たると Tokenizer を呼ばないので、
    旧辞書で数えた節がそのまま残る。Tokenizer をモデルのハッシュで持ち直すだけでは、キャッシュに
    当たったページが残るので足りない（PR #29 のレビュー指摘）
- やること: 同義語のグループの中では最大の点だけを数える。タイトルとの一致を優遇する。
  frontmatter `search: false` でページを索引から外す。B5 は辞書の指紋を再ビルドのたびに比べ、
  違えばセッションを作り直す。2 文字の語の誤字補正はほかの語でも確かめ、再現すれば語の長さで切る
- 検索 wasm を作り直す（vendor-update スキル）。あわせて、作り直し忘れを CI で検出する。
  CI の検索の照合はネイティブの `yuzu search` を通り、`third-party-licenses.sh check` は crate と
  版の集合しか比べないので、版を上げない mikan の変更では wasm が古いままでも CI が通る
  （PR #53 の協議で整合派が指摘。92 の `Shard::parse` の修正も同じ）
  - wasm と `search.js`: `scripts/build-search-wasm.sh` が次の入力のハッシュを wasm の横に記録し、
    ci.yml が今の入力と照合する（記録を書くのはこのスクリプトだけ）。対象は mikan・mikan-wasm の
    `src/` と Cargo.toml / wasm32 向けの依存の木（`cargo tree -p mikan-wasm --target
    wasm32-unknown-unknown -e normal,features`。`third-party-licenses.sh:113-115` と同じく
    接頭辞と path 依存の行（絶対パスと workspace の版を含む）を除く。wasm-bindgen の版と
    features の違いもここに出る）/ ルート Cargo.toml の `[profile.release]` と
    `[profile.wasm-release]` / スクリプト自身
  - 入れないもの: Cargo.lock の全体とルート Cargo.toml の全体（wasm に関係しない依存の更新と
    リリースの版上げでも落ちる。08-01 以降、この 2 ファイルを変えたコミットは 31 件、mikan の
    `src/` を変えたコミットは 0 件）/ `mikan/assets`（分かち書きのモデルは `builtin-model` の
    ときだけ埋め込まれ、wasm では無効 = `mikan/src/lib.rs:53-56`）/ rustc と wasm-opt の版
    （stable で固定していないので記録だけにする）。テストだけの変更でも照合は落ちるが、mikan を
    変えることは少ないので受け入れる（記録だけを更新する手段は作らない = 作り直し忘れの抜け道になる）
  - 同梱の手書きの JS（`crates/mikan-wasm/js/*.js`。スクリプトがコピーするだけ
    = `build-search-wasm.sh:44`）は、コピー先とのバイト一致で照合する（PR #53 の再レビューの指摘）
  - wasm をバイト比較で照合しないのは、コミットした wasm にビルドした人の
    `~/.cargo/registry` の絶対パスが入っていて（10-14 に 45 か所を確認）、CI で作ると必ず違う
    バイトになるため。この絶対パスはすべての利用者のサイトの `_search/` に配られるので、
    作り直すときに `--remap-path-prefix` で取り除く
- 判断点: スコアの変更を wasm 側だけに置いて `FORMAT_VERSION` を据え置けるか / タイトルの
  優遇の重み / `search: false` はページ単位に限るか（協議の推奨は限る。frontmatter の形は
  後から戻せない）
- 確かめること: `yuzu search` とブラウザで同じ順位になること（wasm を作り直した後にユーザが
  ブラウザで確かめる）。docs ゲートで「設定」の上位 3 件（URL が `/reference/config/` で
  始まる結果）を照合する
- 工数: 小〜中（mikan の変更。native と wasm は同じ実装を通る）
- 着手時の確認（10-18）: 「設定」の 1 位は同義語の例を書いた節（7.33 = 「設定」1.91 ＋
  「コンフィグ」5.42）、設定のページの先頭の節は 14 位（1.893。本文が 470 で平均 137 の 3.4 倍あり、
  BM25 の長さの補正で下がる。タイトルの重み 2 では足りない）。「コンフィグ」は 1 節にしか無く
  idf が 4.79 と高いので、**ROADMAP の「同義語のグループの中では最大の点だけ」では max(1.91, 5.42)
  = 5.42 で 1 位のまま**だった。索引の側でタイトルの重みを上げる案は、W=10 で 2.281（次は 2.176）・
  W=20 で 2.457 と BM25 の飽和で差が広がらなかった
- 決定（10-18。ユーザが推奨案で決めた）と実装
  - **同義語** — グループをまとめて 1 語として数える（出現数はメンバーの合計、df はメンバーの最大 =
    Lucene の SynonymQuery。`mikan/src/engine.rs` の `SynonymSlot`）。上の理由で「最大の点だけ」から
    変えた。複数語のメンバーは語ごとの点の合計を 1 つの候補にし、グループの点は候補の最大。
    同義語のグループに入らない語（タイポ展開を含む）は従来どおり足し合わせる
  - **タイトルの優遇** — 検索時の倍率。節のタイトル（先頭の節はページタイトル、ほかは見出し）に
    クエリの語が入っていれば、一致した語の割合に比例して点を上げる（全語一致で 1.5 倍）。節ごとの
    タイトルの term_id を manifest の `docTitleTerms` に足した（mikan の `SectionInput` に
    `title_terms` を足したので、次に mikan を公開するときは 0.3.0）
  - **`FORMAT_VERSION`** — 据え置き（`docTitleTerms` は `serde(default)`。古い wasm は読まず、
    倍率が掛からないだけ。新しい wasm と古い manifest では空 = 倍率なし）
  - **`search: false`** — ページ単位だけ（Phase 88 の読み込み口に足し、`Page::in_search_index` で
    判定）。`CACHE_FORMAT_VERSION` 26 → 27（`CachedSection` の `title_terms` と合わせて 1 回）
  - **B5** — watch の再ビルドのたびに envKey（辞書ファイルの指紋を含む）を比べ、変わっていたら
    セッション（Tokenizer と検索のキャッシュ）を作り直す。zstd の skippable frame を足した辞書
    （伸長した中身は同じでバイト列だけが違う）で、作り直すこと・dist のモデルが新しい辞書になる
    ことをテストで確かめた（修正を外すとテストが落ちることも確かめた）
  - **検索 wasm の作り直し忘れ** — ROADMAP の仕様どおり `build-search-wasm.sh` が入力のハッシュを
    `wasm-inputs.sha256` に記録し、ci.yml の `search wasm inputs`（`--check`）が照合する。path 依存の
    行は絶対パスと版を外して名前と features だけ残した（mikan の公開の版上げは Cargo.toml の
    中身として検出される = wasm のバイトも通知も変わるので作り直しを求めるのは正しい）。
    `--remap-path-prefix` で wasm の絶対パスは 0 件になった（以前は 45 か所）
  - **2 文字の語の誤字補正** — 再現しなかったので変えない（語彙にある 8 語で、結果のすべてに元の
    語が入っていた。語彙に無い「見出」は「見出し」の節に当たり、妥当な補正だった）
- 結果（`yuzu search --root docs`）: 「設定」は設定のページが 1 位（2.824）。「コンフィグ」も
  同じ順位。「シーケンス図」「ER図」は該当の節が 1 位のまま、2 位にあった同義語の例の節が消えた。
  「デプロイ」は配信とデプロイのページが 1 位。docs ゲートで「設定」の上位 3 件を照合する
- ブラウザでの確認（ユーザ作業）: 10-18 にユーザが確かめた。`search_bg.wasm`（524,793 bytes）を読み込んだ
  状態で、「設定」「コンフィグ」の順位が `yuzu search` と同じ（設定のページが 1 位）
- PR #65 のレビュー指摘（3 件。どれも再現して直した）: (1) 複数語の同義語のメンバーに語彙に無い
  語があるとメンバーごと捨てていたのに、クエリの中の token はグループの語として外していたので、
  既知の語まで検索から消えていた（`[["browser", "web client"]]` で「web client」が web の doc に
  当たらない）。語彙にある token だけでメンバーの候補を作る / (2) 同じ語を含むグループが 2 つあると
  （lint.terms と search.synonyms の両方に書いた等）、その語の点を 2 回足していた。重なるグループを
  1 つにまとめる / (3) タイトル一致の判定を引用符の外の語だけで作っていたので、`"設定"` では設定の
  ページが 14 位だった。フレーズの構成語も判定に含める（完全一致の条件はフレーズ照合が別に見る）。
  検索 wasm を作り直した（527,448 bytes）。作り直す前の `build-search-wasm.sh --check` は作り直し
  忘れを検出して失敗した
- PR #65 の再レビューの指摘（1 件。再現して直した）: (2) の修正が構成 token が 1 つ共通なだけで
  グループをまとめていたので、`[["alpha", "client error"], ["beta", "server error"]]` が error だけで
  1 つになり、「alpha beta」でタイトルが alpha だけの doc にも全語一致の 1.5 倍が掛かっていた
  （一部一致なら 1.25 倍）。まとめる判定をメンバー全体（同じ表記のメンバー、または同じ term の
  1 語のメンバー）で行う。複数語のメンバーを term_id 列で比べないのは、語彙に無い token を除いた
  残り（どちらも error だけ）が同じになるため。検索 wasm を作り直した（527,031 bytes）

### 90 読者側の UI と見た目の設定 ✅

**概要**: favicon・フッター・ヘッダーのリンクを設定で足せるようにし（10-07 見直しの D8）、
読者側の UI の小さな穴（D10）と開発者向けの文言（D11）を直す。

- 現状
  - `base.jinja` に `<link rel="icon">` が無く、`public/favicon.ico` はサブパス配信では
    ブラウザが探すドメイン直下に出ない（docs サイトの `/yuzu/favicon.ico` は 404）
  - フッターはテンプレートに無く、ヘッダーはタイトル・検索・テーマ切替だけ。「社外秘」の
    表示や社内ポータルへのリンクにはテンプレートの上書きが要る
  - ヘッダーの検索欄は `font-size: 0.9rem`（`theme.css:269`。iOS Safari は 16px 未満の入力欄で
    画面を拡大する。iOS の挙動は**未確認**）・`aria-label` が無い。テーマ切替は押した状態を
    示さず、一度選ぶと「OS に従う」へ戻れない。ショートカットの表記が「Cmd+K」と
    「Cmd/Ctrl+K」で揃っていない
  - 検索の初期化に失敗すると「（コンソール参照）」（`search-ui.js:237`・`search-page.js:228`）。
    Admonition のラベルは英語（comrak の既定の Note / Tip 等）
- やること: favicon・フッターの文言・ヘッダーのリンクの設定キー。検索欄の 16px と
  `aria-label`、表記を「Ctrl/Cmd+K」に、失敗時の文言を読者向けに。テーマの追加は
  add-theme-asset スキルの手順で。ci.yml の docs のビルド（`ci.yml:130`）は `--base-url` を
  付けないので、サブパスで 404 になる今の不具合を検出できない。`--base-url` 付きのビルドで
  `rel="icon"` の href と実ファイルを照合する（リリース判定）。リリース判定は docs の dist を
  見るので、docs サイトへの favicon の設定もこの Phase で行う（93 から移した）
- 判断点: 設定キーの名前と型（`site.favicon`・フッター・ヘッダーのリンク。協議の推奨は
  この 3 つの最小限に絞る）/ Admonition のラベルを日本語にする設定を入れるか（i18n の候補
  `theme.strings` と重なる）/ テーマ切替をライト・ダーク・OS に従うの 3 状態にするか
- 確かめること: 手元の preview で favicon・フッター・キーボード操作・検索欄（ユーザ）。
  スナップショット。公開サイトの favicon はリリース後に確かめる
- 工数: 小〜中
- 決定（10-18 に着手時に確認したが応答が無く、推奨案で仮決めした。11-08 にユーザが推奨案のまま確定）と実装
  - **設定キー** — `site` に 3 つ: `site.favicon`（文字列。public/ のパスかフル URL）・
    `site.footer`（インラインの Markdown。`yuzu_core::render_inline_markdown` で描く）・
    `[[site.header_links]]`（`text` と `url` は必須。`javascript:` / `data:` / `vbscript:` は設定エラー）。
    リンクは `site.logo` と同じく base を前置し、外部 URL・`mailto:` はそのまま（`pipeline.rs` の
    `site_link_url`）。ヘッダーのリンクは狭い画面（≤50rem）ではナビの下に出す。フッターは印刷にも残す
  - **favicon の自動** — 自動で出す。`site.favicon` が無くても `public/favicon.ico` が
    あれば、base の下を指す `<link rel="icon">` を出す
  - **Admonition のラベル** — v0.21 へ移す（i18n の `theme.strings` でまとめて扱う）
  - **テーマ切替** — 3 状態にする。押すたびに OS の設定 → 今の見た目の反対 → もう一方 →
    OS の設定（最初の 1 回は必ず見た目が変わる）。今の状態と次の状態を aria-label と title に出し、
    アイコンを ◐ / ☀ / ☾ にする。OS の設定に戻すと localStorage の選択を消す
  - 検索欄を 16px（`1rem`）にし `aria-label="サイト内を検索"` を足した。ショートカットの表記を
    「Ctrl/Cmd+K」にそろえた（docs 3 か所・404 ページ）。検索の初期化に失敗したときの文言を
    「検索を読み込めませんでした。ページを再読み込みしてください」にした（原因はコンソールに出す）
  - docs サイトに favicon（ロゴの SVG）・フッター・ヘッダーのリンク（GitHub）を設定した。
    `docs/yuzu.toml` の `[site]` に 2 行足したので、インクルードの引用範囲を 25-45 → 27-47 にした
    （code-and-math の 3 か所・ci.yml・verify スキル・CLAUDE.md）
  - ci.yml の docs の手順の最後に、`--base-url /yuzu/` で作り直して `rel="icon"` の href が base の下を
    指し、そのファイルが dist にあることを照合する（リリース判定。修正前の docs では失敗することを
    手元で確かめた）
- ブラウザでの確認（ユーザ作業）: 10-18 にユーザが preview で確かめた。タブのアイコン・フッター
  （印刷プレビューにも残る）・ヘッダーのリンク（狭い画面ではナビの一番下）・検索欄・切替ボタンを
  4 回押して元に戻ること・`/` と Ctrl/Cmd+K
- PR #66 のレビュー指摘（1 件。再現して直した）: 外部 URL の判定（`urlpath::is_external_url`）が文字列の
  どこかに `://` があるかを見ていたので、`/search/?q=https://example.com/` のようにクエリに URL を含む
  サイト内リンクに base が付かなかった（ヘッダー・フッター・本文とも。`public_url` にも同じ判定があった）。
  リンク検査もこのリンクを外部として飛ばし、リンク切れを見逃していた。先頭の `scheme://` で判定する
  `has_url_scheme` を足し、`public_url` と `og:image` もこれを使う。本文 HTML が変わるので
  `CACHE_FORMAT_VERSION` 27 → 28。本文のリンクとリンク検査は Phase 90 より前からの不具合
- 確かめていないこと: iOS Safari の実機で検索欄が拡大しないこと

### 91 既存の原稿から始める人・他のツールから来た人へ ✅

**概要**: 既存の Markdown を持ち込んだ人が黙って壊れたサイトを出さないようにし、他の
ツールから来た人がつまずく違いを 1 ページにまとめる（10-07 見直しの D6）。PR #53 の協議で、
導入の手順で実際に止まる所（上の「現状の確認」の最後の項目）をこの Phase に加えた。
3 つの PR に分ける: (1) `input.dir` の外を指すリンクの扱い、(2) それ以外、(3) `search.page` /
`markdown.glossary.page` の `"/"` を設定エラーにする（v0.19.1 の入口検証の漏れ。小さいので先に出してよい）。

- 現状
  - 既存の Markdown を持ち込むと: `docs/README.md` がトップページにならず `/` が 404・警告も
    無い / ルートより上へ出る `..` のリンクが黙って別のページに解決される / 原稿 0 件の警告が
    ルート直下の `.md` しか数えない（`commands/build.rs:576-584`）/ `yuzu new .` の「空では
    ありません」（`commands/new.rs:59-61`）が「既存のフォルダで始める」の節を案内しない /
    持ち込んだ原稿は整形差分で check が失敗するのに、CI の例（`guide/quality.md:256`）に
    `--no-fmt` が無い（すべて 10-14 に確認）
  - 雛形 deploy.yml の中身が docs のどこにも無い（`guide/install.md`・`guide/deploy.md` は名前
    だけ）ので、既存のフォルダの人は別の場所で `yuzu new` してコピーするしかない。PR を検証する
    （公開しない）workflow の例も無い（`guide/quality.md:248-258` は check の 1 ステップだけ）
  - ナビは自動生成だけ（`nav.auto = false` は予約で効果なし）。ファイル名がそのまま URL に
    なる。frontmatter のキーは camelCase で yuzu.toml は snake_case。生 HTML・画像の大きさの
    指定が docs に無い
  - 雛形で、build のログは 5 ページ（用語集と検索結果ページを含む）・検索インデックスは
    4 ページ（検索結果ページを除く）・check は 3 ページ（原稿だけ）と出る。check は雛形の
    原稿の抑制の例を「抑制 3 件」と数える。何が公開物として出るかは `guide/internal.md` の表に
    しか無く、build のログは件数だけ
  - B8: 用語集ページは用語を `## {term}` で生のまま埋め込む（`markdown/glossary.rs:168`）ので、
    `__init__` は太字に、`Vec<T>` の `<T>` は生 HTML として消える（**実機未確認**）
- やること
  - PR (1): `resolve_relative` が `input.dir` の外へ出る `..` を捨てずに知らせる。リンク検査
    （`linkcheck.rs`）と描画（`urls.rs`）は同じ関数を通るので両側が揃う（93 の R5 とは独立）。
    描画される href が変わるなら `CACHE_FORMAT_VERSION` を上げる。既存のサイトで check が新たに
    失敗しうることをリリースノートに書く
  - PR (2): トップページが無いときの警告と、0 件の警告を `input.dir` の下も数えるように直す。
    `yuzu new` のエラーから手順の節へ案内する。build の最後に何を出したかの 1 行の要約
    （ページ・原稿の `.md`・llms.txt / llms-full.txt・検索・public の件数）を出し、ページ数の
    食い違いの答えにする。一覧は `SiteOutput::emitted` と `Page::emits_page_md` から作る（別の
    一覧を持つと v0.19.1 の R1 と同じずれが起きる）。用語の Markdown の記号をエスケープして
    から合成する（B8）
  - PR (2) の条件（PR #53 の再レビューと協議で決めた。導入派が 0.19.1 で 8 通りを試した）
    - トップページの警告は `index.md` の有無ではなく「今回の出力の記録（書いたファイル。ページ・
      同伴アセット・`public/` のコピーを含む）に `index.html` が無い」で判定し、理由（`/` の
      ページが draft・`input.ignore` で外れている・`README.md` がある等）を添える。`public/index.html`
      や同伴の `content/index.html` だけでトップページを作るサイトは警告しない（同伴の
      `index.html` だけの構成で正しいトップページが出ることを PR #53 の 3 回目のレビューで確認。
      ページと `public/` だけで判定すると誤って警告する）。draft を含む check では判定がずれる
      ので、警告は build と dev で出す。判定の関数を単体テストで確かめ（導入派の 8 通りと同伴の
      `index.html`）、e2e は配線の確認に 1 件
    - 利用者のファイル（同伴アセット・`public/`）が yuzu の生成物（トップページ・原稿の `.md`・
      llms 等）を上書きしたら警告する。利用者のファイル同士（`public/` が同伴アセットを上書き）は
      仕様なので除く。今は `index.md` と `content/index.html` が両方あると、同伴の HTML が生成した
      トップページを黙って上書きする（10-14 に確認）。`public/` はルート直下に固定なので、Web
      アプリのリポジトリの直下に yuzu.toml を置くと、アプリの `public/index.html` も同じく
      トップページを上書きする（導入派が再現）。上書きは出力の記録への 2 回目の書き込みで分かる
    - 要約は「生成した Markdown」と「実際に配信されるファイル」を混同しない: 件数は出力の
      記録（今回書いたもの）から数え、`public/` の `.md` は原稿の `.md` に数えない。`--force` と
      増分ビルド（書き込みを省いたファイルも数える）で同じ件数になることを確かめる
  - PR (3): `synth_page_problem`（`urlpath.rs:144-147`）は trim した後に空なら「作らない」と
    みなすので、`search.page = "/"` は検索結果ページを作らずに build も check も成功する（10-14 に
    確認）。元の値が空でないのに trim した後に空になるなら設定エラーにする（エイリアスの `"/"` は
    すでに設定エラー = `aliases.rs:57`）。空文字は今までどおり「作らない」
  - PR (2) の docs: 「他のツールとの違い・よくある書き方」のページ（90 で足す設定キーも
    含める）。deploy.md に雛形 deploy.yml の全文と `--root` での書き換えを載せる。PR を検証する
    （公開しない）workflow の例を yml ファイルにして `file=` で取り込み、ci.yml の SHA の照合
    （`ci.yml:77-86`。今は雛形 deploy.yml だけを読む）の対象に足す。雛形は docs のルートの外に
    あって `file=` で取り込めないので、docs の下に写しを置いて CI で差分を照合する。CI の例に
    `--no-fmt` を書く
- 判断点: `input.dir` の外を指すリンクを既存の `broken-link`（error）にするか、新しいルールの
  warning（`lint.rules` で外せる）にするか（導入派は「行き先が別のページになっていた不具合で、
  利用者が 0 人の今がいちばん安く直せる。docs と雛形にはルートの外へ出るリンクが 0 件」、範囲派は「既存のサイトの check が新たに失敗する
  非互換なので外せる warning に」）/ `README.md` を index として扱うか（route の規則は後から
  戻せないので、協議の推奨は v0.20 では警告だけ）/ 雛形の原稿を機能のカタログのままにするか
  （10-04 見直しの「細かな分かりにくさ」）/ 「他のツールとの違い」のページと v0.21 の「他ツール
  との比較」の分担（91 は書き方の違い、v0.21 は比較と選び方）
- 工数: 中（10-14 の協議で「小」から直した）
- 決定（10-18。ユーザが推奨案で決めた）
  - **`input.dir` の外を指すリンク** — 新しいルールの warning（`lint.rules`・`lintDisable`・行コメントで
    外せる）。既存の原稿は `docs/` から `../src/` を指すことが多く、すぐには直せない人が外して CI を
    通せるようにする。warning も外さなければ check は終了コード 1（着手時の確認では「check が失敗
    しない」と誤って説明したので、訂正して確かめ直した）。href は別のページへ書き換えずに残す
  - **`README.md`** — v0.20 では警告だけ（トップページが無いときの警告に理由として添える）
  - **雛形の原稿** — 今のまま。作り直しは v0.21 へ
  - **「他のツールとの違い」のページ** — 91 は書き方の違い、v0.21 は比較と選び方
- PR (3): `synth_page_problem` が、元の値が空でないのに `/` を外すと空になる値（`"/"`・`" / "`）を
  設定エラーにする（「サイトのルート（`/`）には置けません」）。空文字は今までどおり「作らない」。
  設定リファレンスの 2 行に追記（PR #67。10-18 マージ）
- PR (1)（PR #68。10-18 マージ）: `resolve_relative` は `..` が content より上へ出ると `None`。リンク検査は
  新しいルール `link-outside-content`（warning）、描画は href を書いたまま残す。build の警告の 1 行にも
  件数を出す。`CACHE_FORMAT_VERSION` 28 → 29
  - レビュー指摘（2 件。どちらも再現して直した）: (1) Windows で新しいテストが落ちた（診断のパスを
    文字列で比べていた。`rel_to_slash` で比べる）/ (2) lint は check だけのルールを評価しないのに、
    評価しなかったルールとして渡していたのは `external-link-broken` だけで、`link-outside-content` の
    抑制を未使用と報告して終了コード 1 にした。`route-case-conflict` も Phase 85 から同じで、発火する
    抑制まで lint が未使用と報告していた。`rules::CHECK_ONLY`（docs の「yuzu check が追加するルール」の
    節とテストで一致）を lint が評価しなかったルールとして渡す
- PR (2) は、コードの PR（2a）と docs の PR（2b）に分ける（docs のページが警告と要約の行を説明するので、
  コードを先に出す）
- PR (2a)（PR #69。11-06 マージ）: build の最後の報告と、持ち込んだ原稿で黙って壊れる所
  - **出力の記録に出どころ** — `OutputTracker::write` は書く側が `OutputKind`（ページ・リダイレクト・
    原稿の `.md`・`Site(SiteOutput)`・同伴アセット・`public/`）を申告する。記録をパスの文字列で分類しない
  - **要約の 1 行**（INFO）— 「出力: ページ 5（原稿 3・用語集・検索結果ページ）・リダイレクト 1・原稿の .md 4・
    llms.txt・llms-full.txt・検索 4 ページ・public 1」。件数は出力の記録から数える（`--force` と増分ビルドで
    同じ行・同じマニフェストになることを確かめた）。build 5・検索 4・check 3 の食い違いの答えにする。
    利用者のファイルで上書きされたページがあれば、内訳に「N 件は利用者のファイルで上書き」と添える
  - **トップページの警告** — 出力の記録に `index.html` が無ければ、理由（`index.md` が無い・draft・
    `input.ignore`）と、`README.md` があれば `/README/` になることを添える。`public/index.html`・同伴の
    `index.html` だけのサイトは警告しない。build と dev だけ。導入派の 8 通りは記録に残っていなかったので、
    協議の要約から組み直して単体テストにした
  - **上書きの警告**（10-18 に確認したが応答が無く推奨案で進め、10-19 にユーザが推奨案で確定）—
    利用者のファイルが生成したページ（トップページを含む）・リダイレクト・原稿の `.md` を上書きしたら
    警告する。docs が案内している `public/404.html`・`public/llms.txt`・`public/sitemap.xml` 等の
    置き換えは警告せず、要約の行の末尾に
    「（利用者のファイルを優先: 404.html）」と出す（ROADMAP の「llms 等も警告」と docs の案内が
    食い違っていたため。意図して置き換えた人に毎回警告が出て、消す設定も無い）
  - **0 件の警告** — `input.dir` の下の `.md` も数え、draft と `input.ignore` の件数を添える
  - **`yuzu new`** — 空でないフォルダのエラーに、docs の「既存のフォルダで始める」を添える
  - **B8** — 用語集の用語とタイトル・検索結果ページのタイトルの Markdown の記号をエスケープしてから
    合成する（修正前は `Vec<T>` が「Vec」、`__init__` が「init」になることを確かめた）。記号の集合は
    comrak の正規形と同じ（`$` は数式の記法のまま）。用語の見出しのアンカーが変わる（`#init` → `#__init__`）
  - レビュー指摘（1 件。再現して直した）: 出力の記録を rel の文字列だけで照合していたので、大文字小文字を
    区別しない FS で `content/index.md` と `public/INDEX.html` を置くと、`dist/index.html` は利用者の内容に
    なるのに上書きの警告が出ず、要約も「ページ 5」のままだった（`--force` と増分ビルドの両方）。
    記録は 1 ファイル 1 件にし、大文字小文字を畳んだ文字列で候補を探してファイルの同一性で判定する
    （孤児掃除と同じ規則。区別する FS では別のファイルとして記録する）。キーは先に書いたパスのまま
    出どころを付け替え、警告には利用者のファイルの実際の名前（`public/INDEX.html`）を出す
  - 2 回目のレビュー指摘（2 件。再現して直した）: (1) トップページの警告の理由をファイルシステムから
    別に調べていたので、ページの読み込みの走査とずれた。シンボリックリンクの `index.md`（走査はリンクを
    辿らない）を draft と、draft や `input.ignore` で外れた `README.md` を `/README/` になると案内し、
    原稿の `.md` がすべてシンボリックリンクだと 0 件の警告が「.md がありません」になった。理由は、
    ページの読み込みと同じ走査で分けた `yuzu_core::markdown_census` とビルドしたページから作る
    （大文字の `Index.md` も README と同じく、なったページを案内する）/ (2) 同伴アセットが上書きした
    ページを `public/` がさらに上書きすると、警告が配信されない同伴アセットを挙げた。上書きした側を
    `public/` へ付け替える
- PR (2b): docs
  - 決めたこと（10-20。ユーザが推奨案で決めた）: ページに他のツールの名前は出さず、書き慣れた書き方
    （`::: tip`・`sidebar_position`・`README.md` がトップページ 等）と yuzu の規則で書く（名前を出した
    比較と選び方は v0.21）/ ページはガイドの「インストールと最初のサイト」の直後に置き、後ろの
    11 ページの `order` を 1 つずつ振り直す
  - **他のツールとの違い**（`guide/migrate.md`）— ナビと URL・frontmatter と設定ファイル・記法・リンク・
    生 HTML と画像の大きさ・サイトの見た目（90 で足した設定キー）・移したあとに確かめること。書く前に
    試験用のプロジェクトで確かめた: `::: tip`・見出しの `{#id}`・`{{ }}`・`:smile:`・`[[…]]` は警告なしで
    文字のまま出る / `.md` を付けない相対リンクは書き換えも検査もしない / `.md` を付けた絶対パスは
    broken-link / 生 HTML の URL は `base_url` も付かず検査もしない / `.mdx` は同伴ファイルとして
    そのまま公開される（`input.ignore` で外せる）/ タイトルは `title`・最初の h1・ファイル名の順
  - **ワークフローの全文** — deploy.md に雛形 deploy.yml の全文（`docs/snippets/deploy.yml` の写しを
    `file=` で取り込む）と、`__YUZU_VERSION__`・`--root`・`branches` の書き換えを載せる。install.md の
    「既存のフォルダで始める」から案内する
  - **PR 検証の例**（`docs/snippets/pr-check.yml`）— `yuzu check --format github` と `yuzu build`。
    quality.md に取り込み、CI の例に `--no-fmt` を足す
  - ci.yml の `scaffold actions pins` が、docs の写しと雛形のバイト一致・PR 検証の例の actions の SHA・
    yuzu のインストール手順（コメントを除く）が雛形と同じかを照合する（3 つとも、写しを壊すと
    exit 1 になることを確かめた）。docs ゲートに 3 行
  - ブラウザでの確認（ユーザ作業）: 10-20 にユーザが preview で確かめた。他のツールとの違いのページ・
    deploy の「ワークフローの全文」・quality の「CI に組み込む」・install の「既存のフォルダで始める」の
    手順 5 と案内・サイドバーの並び（インストールの次）

### 92 fuzz ✅

**概要**: fuzz を定期実行にし、対象を kabosu から mikan（検索の索引の読み込み）と tankan
（図の描画）へ広げる。mikan の `Shard::parse` の panic と巨大確保（10-07 見直しの E2）を直す。

- 現状: fuzz.yml は手動起動だけで、対象は kabosu の 3 つ（decode・parse・roundtrip）。
  実行は 09-06 の 2 回だけ。`mikan/src/shard.rs:96` の `(term_count as usize + 1) * 4` は
  wasm32 で桁あふれし、長さの検査を通った後に範囲外の添字で panic する。`:137`・`:157` の
  `Vec::with_capacity(doc_freq as usize)` は入力の値をそのまま使う（**実機未確認**）
  - tankan の入口は `render_svg` の 1 つ（`tankan/src/lib.rs:43`）で、種には corpus の `.mmd`
    約 100 件を使える。yuzu は tankan の panic を回収してクライアント描画に切り替える
    （`yuzu-render/src/highlight.rs:301`）が、無限ループとメモリ超過は回収できず build ごと止まる
    （v0.18 Phase 77 で見つかった 2 件は gantt の panic と state の循環によるメモリ超過）
  - fuzz.yml の rust-cache は `crates/kabosu/fuzz` に固定（`fuzz.yml:28`）。定期実行は
    リポジトリに 60 日間動きが無いと GitHub が止める
  - `shard.rs:96` の桁あふれは usize が 32 bit のときだけ起きるので、64 bit で動く cargo-fuzz
    では見つからない。`:137`・`:157` は postings の読み込みの中で、`parse` だけを呼ぶ対象では
    届かない。64 bit では今のコードでも `term_count = u32::MAX` はエラーで返るので、素直に書いた
    単体テストは直す前から通る（PR #53 の協議で整合派が指摘）
  - postings には位置情報がある。`postings_with_positions`（`shard.rs:153`）の `:163` の
    `with_capacity(pos_count)` と `:166` の加算は、この関数からしか通らない（PR #53 の再レビューの
    協議で範囲派が指摘）。fuzz.yml は実行時間（`-max_total_time`）しか指定しておらず、
    `-timeout` の既定は 1200 秒なので、300 秒の実行ではハングを報告しない
- やること
  - `Shard::parse` を `checked_*` と確保量の上限で直す（`:137`・`:157`・`:163`）。長さの計算を
    usize の幅に依らない関数（u64 か checked の u32）に切り出し、その関数を単体テストで確かめる。
    直したら検索 wasm を作り直す（89 で足す照合が作り直し忘れを検出する）
  - fuzz の対象に mikan（位置情報付きの postings まで読む）と tankan の `render_svg`（corpus を
    種にする）を足す。対象ごとに別のジョブにして（`fail-fast: false`）、tankan の panic で他の
    対象が止まらないようにする。キャッシュの対象を広げる
  - 制限の値は fuzz.yml の対象ごとの設定（matrix）の 1 か所に書く: 実行時間
    （`-max_total_time`）・ハングの判定（`-timeout`。既定の 1200 秒より短く）・メモリの上限
    （`-rss_limit_mb`。既定と同じ 2048 でもリリース判定の基準なので明記する）。ジョブに
    `timeout-minutes` を付ける。publish-crate スキルの手元での実行は fuzz.yml の値を参照する
  - fuzz.yml に定期実行を足し、リリース判定のとおり手動で 1 回実行して確かめる。見つかった
    ハング・メモリ超過は直す。tankan / mikan を直したらパッチ版を出す（publish-crate スキル）
- 判断点: 定期実行の頻度と 1 回の時間 / tankan で見つかった panic をどこまで直すか（ハングと
  メモリ超過は必ず直す。panic は yuzu が回収するので延期できる）/ panic を延期するときの
  実行の仕方。libFuzzer は最初のクラッシュで止まるので、直していない panic があると定期実行が
  毎回同じ入力で失敗し、ハングを探せない。協議の案は `-fork` と `-ignore_crashes=1` で最後まで
  実行し、`-ignore_timeouts=0`・`-ignore_ooms=0` を明記する（fork のときはハングとメモリ超過も
  既定で受け流す見込みのため）。この動作は**未確認**で、使えなければリリース判定のとおり tankan
  の対象を定期実行から外す
- 決めたこと（11-08。ユーザが推奨案で決めた）: 定期実行は週 1 回・1 対象 600 秒・
  `-timeout=10`・`-rss_limit_mb=2048`（リリース判定の「同じ秒数の手動実行」は 600 秒）/ tankan の
  panic は、図種のパーサの中で直せる小さいものはこの Phase で直し、大きいもの・件数が多いものは延期して
  件数をリリースノートに書く（ハングとメモリ超過は必ず直す）/ PR は 3 本: (1) mikan の `Shard::parse`、
  (2) fuzz の対象と fuzz.yml・手動実行、(3) 手動実行で見つかった tankan の修正。`-fork` と
  `-ignore_crashes` の動作は (2) の最初の手動実行で確かめる（開発コンテナに C++ コンパイラが無い。
  tankan に panic が出なければ確かめる必要が無く、出て受け流せなければリリース判定のとおり tankan を
  定期実行から外す）。合否はプロセスの終了コードではなく、artifact の `timeout-*`・`oom-*` の有無で決める
- PR (1): mikan の `Shard::parse` と postings の読み込み
  - blob の開始位置を `blob_start`（u64）で計算する。32 bit の usize での桁あふれを、usize の幅に依らない
    値の単体テストで確かめる（64 bit では修正前も parse がエラーを返すので、parse だけのテストでは差が出ない）
  - 申告された件数（`doc_freq`・`pos_count`）での確保を、残りのバイト数で読める件数までに抑える
    （`bounded_capacity`。1 posting は 3 バイト以上・1 位置は 1 バイト以上）。64 bit の Linux は overcommit で
    32 GB の確保も通るので、修正前との差は通常のテストでは出ない（wasm32 では容量の桁あふれで panic、
    fuzz では malloc の上限で止まる）。上限の計算は単体テストで確かめる
  - doc_id と出現位置の delta の合計の桁あふれを `FormatError::PostingOverflow` にする（修正前は debug で
    panic・release で値が巻き戻っていた。テストは修正を外すと panic で失敗する）。variant を足すのは
    破壊的だが、mikan の次の公開は 89 の `title_terms` で既に 0.3.0
  - 検索 wasm を作り直した（528,750 bytes。`search.js` は同一）。壊れていないシャードの読み方は変わらない
    ので、`FORMAT_VERSION` と `CACHE_FORMAT_VERSION` は上げない
  - PR #80 でマージ（11-08）
- PR (2): fuzz の対象と fuzz.yml
  - 対象: mikan の `shard`（parse の後、全 term の postings を位置情報付きまで読む）と tankan の
    `render_svg`。kabosu と同じく `crates/<crate>/fuzz/` の独立した workspace（`Cargo.lock`・corpus・
    artifacts は .gitignore。入れ子のパッケージなので crates.io の配布物には入らない）
  - 種: mikan はエンコーダが書いたシャード 5 つを `fuzz/seeds/shard/` に置く（magic とバージョンを
    通らないと postings まで届かない）。今のエンコーダの出力と同じかを mikan のテストが照合し、
    `MIKAN_UPDATE_SEEDS=1` で書き直す。tankan は `tests/corpus/<図種>/*.mmd` を fuzz.yml が平らにして置く
  - fuzz.yml: 週 1 回（月曜 03:00 JST）の定期実行と手動実行。対象ごとに別のジョブ（`fail-fast: false`）。
    制限の値（600 秒・`-timeout=10`・`-rss_limit_mb=2048`）は `env` の 1 か所（対象で値を変えないので
    matrix ではなく env にした）。`timeout-minutes: 60`。合否は artifact と終了コードで決める:
    `timeout-*`・`oom-*` と 0 以外の終了は失敗、`crash-*`・`leak-*` は tankan だけ、最後まで実行できたら
    （終了コード 0）件数を Summary と警告に出して受け流す（`-fork=1 -ignore_crashes=1 -ignore_timeouts=0
    -ignore_ooms=0`）。artifact があれば upload する
  - publish-crate スキルの手元での実行を fuzz.yml の値にそろえ、CLAUDE.md に fuzz の運用を書いた
  - レビュー指摘（1 件。再現して直した）: judge は tankan に `crash-*` があると終了コードを見ずに成功に
    していたので、panic の後に OOM killer 等で artifact を残さず止まった実行（終了コード 1・137）も成功に
    なった。終了コード 0 をすべての対象で必須にし、tankan の panic は最後まで実行できたときだけ受け流す
    （手動実行で、panic を受け流して最後まで実行したときの終了コードが 0 であることを確かめていた）。
    judge の `run:` を取り出して 11 通り（件数・終了コード・対象の組み合わせ）を手元で試し、修正前は
    3 通りが誤って成功になることを確かめた
  - 手動実行（600 秒。run 42442784937・11-08）: kabosu 3 つと mikan は panic・ハング・メモリ超過とも 0。
    tankan はハング・メモリ超過 0・panic 16（受け流した）。fork モードは panic を受け流して 619 秒まで実行し、
    最後の集計は `oom/timeout/crash: 0/0/16`、終了コードは 0（判断点の「未確認」を解消）
  - PR #81 でマージ（11-08）
- PR (3): 手動実行で見つかった tankan の panic
  - 16 件はすべて state 図の `state/parser.rs:315` の `expect("事前スキャンで登録済み")`。事前スキャンは
    `"state "`（空白 1 つ）で `state X {` を見つけ、本処理は `split_whitespace` で行頭の語を取っていたので、
    両者が食い違うと未登録の名前を引いた。fuzz の入力は行頭の垂直タブだったが、`state<タブ>A {`・
    `state "説明" as A <<fork>> {`・`state {` でも当たることを確かめた
  - 行頭の語の取り方を `kind::keyword_of`（必ず行の先頭部分を返す）にまとめ、各図種のパーサ
    （flowchart・class・sequence・gantt・ER・state）から使う。`split_whitespace` で取った語の長さで残りを
    切り出すと、行頭に複数バイトの空白（U+2028 等）が並ぶ行で文字の境界の途中を切って panic する
    （class・flowchart・gantt・state で再現。fuzz の 600 秒では見つかっていなかった）
  - state 図の `state` の後ろの分解を `parse_state_decl` にまとめ、事前スキャンと本処理が同じ規則で
    composite の名前を決める。名前の無い `state {` はエラーにし、`expect` はエラーを返す形にした
  - `state<タブ>A {` のように panic してクライアント描画に切り替わっていた図が SSR で描けるようになり
    本文 HTML が変わるので、`CACHE_FORMAT_VERSION` を 33 に上げる。tankan の修正はパッチ版（0.2.2）で
    公開する（ユーザの作業。publish-crate スキル）
  - 手動実行（600 秒。run 42452688578・11-08。ブランチ phase-92-tankan-state）: 5 対象とも panic・ハング・
    メモリ超過が 0 件（tankan は 616 秒まで実行して `oom/timeout/crash: 0/0/0`・終了コード 0）。**リリース判定の
    fuzz の条件を満たした**（tankan の panic の件数をリリースノートに書く必要も無い）
- 工数: 小〜中

### 93 dogfooding と整理 ✅

**概要**: v0.20 の機能を docs サイトで使い、10-12 レビューの小さい整理（R4・R5・R9）と
運用文書の古い記述の残りを片付ける。ROADMAP の完了済みの内訳を別の場所へ移す。

- 現状
  - R4: `pipeline.rs` の page / search と 404 の `context!` に同じ式のキーが 11〜16 個ある。
    minijinja は未定義の変数を空・偽として通すので、片方に足し忘れても 404 ページだけ機能が
    黙って消える。対処案はページによらない値を 1 つにまとめて `..` で合成する（利用者が上書き
    したテンプレートを壊さないよう変数名は変えない）
  - R5: `linkcheck.rs` の `check_one` と `urls.rs` の `rewrite` が同じ 6 分岐（空・`#`・外部・
    `/` 始まり・`.md`・拡張子）を同じ順に書き、一致はコメントで縛っている（ddfd6aa はこの
    順序のずれの修正）。対処案は urlpath に分類の enum を置き、両側を網羅 match にする
  - R9: `CACHE_FORMAT_VERSION` を上げるのはコメントとスキルの手順だけで、v13 は上げ忘れて
    後から計上した（fb56ef5）。対処案は本文のスナップショットのハッシュを版の隣の定数と照合する
    テストか、キャッシュする型のフィールド名の指紋をヘッダーに入れる
  - 運用文書の古い記述（10-04 見直し）の残り: 7 crate の Cargo.toml の「README ロードマップ
    参照」（ロードマップは ROADMAP.md に分かれた）・tankan / mikan / kabosu の Cargo.toml・
    `fuzz.yml`・`ci.yml` が CLAUDE.md の「リリース手順」を参照する（今はスキルへの案内だけで
    1 段遠回り）・`crates/mikan/assets/model/README.md:23` が git 管理外の「設計ノート」に
    言及する。10-04 の対処案どおり、参照先が実在するかを verify で照合する検査も足す
  - axum を既定の features のまま使っていて、使っていない json / form / query が入る
  - ROADMAP は 1,700 行あまりで、約 7 割が完了済みの内訳。候補に「誰の何を解決するか」が
    書かれていない（10-04 見直しの「ROADMAP.md の肥大化」）
- やること: docs サイトにフッターを設定する（favicon は 90 で設定する）。R4・R5・R9。運用文書の古い記述の残り。
  axum の features。完了済みの内訳を移し、ROADMAP は「現在」と候補と各版の 1 行の要約にする
- 判断点: 完了済みの内訳の置き場所（docs サイトの開発者向けのページとして公開するか、
  リポジトリ内の別ファイルにするか）
- 決めたこと（11-08 に確認したが応答が無く、推奨案で仮決めした。同日にユーザが推奨案のまま確定）:
  完了済みの内訳はリポジトリ内の別ファイル（`ROADMAP-history.md`。公開サイトには出さない）へ移す /
  PR は 3 本: (1) R4・R5・axum の features、(2) R9、(3) 運用文書の古い記述と参照先の照合・ROADMAP の
  整理。docs サイトのフッターは 90 で設定済み（`site.footer`）だった。あわせて、90 で仮決めのままだった
  判断点 4 つもユーザが推奨案のまま確定した（リリース判定の「仮決めのままの判断点が無い」）
- PR (1): R4・R5・axum の features（内部の整理。出力は変えない）
  - R4: ページと 404 の `context!` に同じ式で並べていた 12 個のキー（site・nav_collapse・base_url・
    asset_url・live_reload の 2 つ・highlight・dark・theme_mode・search の 2 つ・theme_css_vars）を
    `common_ctx` の 1 つにまとめ、両方で `..common_ctx` として合成する（minijinja の `context!` の `..`）。
    テンプレートから見える変数名は変えない
  - R5: リンクの URL の分類を `urlpath::classify_link`（enum `LinkTarget`: 空・フラグメント・外部・
    ルート絶対・相対の `.md`・拡張子付き・その他）に置き、`linkcheck::check_one` と yuzu-render の
    `UrlResolver::rewrite` が網羅 match で使う。分類はデコード前・相対参照のパスだけデコードする規則も
    この 1 か所にした
  - axum: 既定の features をやめ、`http1`・`tokio`・`ws` だけにする（json・form・query の extractor は
    使っていない）。Cargo.lock から form_urlencoded・ryu・serde_path_to_error・serde_urlencoded が外れた
  - 変更の前後で、docs（295 ファイル。404.html を含む）と雛形（91 ファイル）の出力、docs の
    `check --format json` の出力がバイト同一
  - PR #83 でマージ（11-08）
- PR (2): R9（`CACHE_FORMAT_VERSION` の上げ忘れをテストで止める）
  - yuzu-cli のテスト `キャッシュの中身が変わったら版を上げる`: 記法を一通り使う試験用のプロジェクト
    （タブ・行ハイライトと行番号・表と表番号の参照・脚注・定義リスト・約物に隣接した強調・用語集の
    `<abbr>`・リンクの書き換えと base_url・画像・別名・図の SSR と図番号・`state<タブ>A {`・未対応の
    図種のクライアント描画への切り替え・数式・折りたたみ・`file=` と Markdown 断片の取り込み・
    frontmatter の lintDisable / readingTime / pageMd / search）を実際にビルドし、保存された
    キャッシュの中身（meta・body・search・llms。キーの source_sha256 等は外す）の SHA-256 を、
    `CACHE_FORMAT_VERSION` の隣の `CACHE_FINGERPRINT`（版, 指紋）と照合する。失敗時に書き直す値を出す
  - ROADMAP の 2 案のうち「本文のスナップショットのハッシュ」を、キャッシュに保存された中身そのもので
    行う形にした。生成のロジック・依存の更新（スナップショットに差分が出ないものも含む）・キャッシュする
    型の変更（JSON にフィールド名が入るので、もう 1 案の「フィールド名の指紋」も兼ねる）をまとめて検出する
  - 原稿は文字列から一時ディレクトリに書き（チェックアウトの改行の違いを持ち込まない）、2 回ビルドして
    指紋が同じことも確かめる（実行ごとに変わる値が入ると指紋に使えない）。取り込み（`file=`）のある
    ページは本文 HTML をキャッシュしない（v15）ので、取り込みは別のページにした
  - 本文 HTML を変えて版を上げない・版だけ上げる、のどちらでもテストが失敗することを確かめた。
    試験用のプロジェクトの原稿を書き換えたときも指紋が変わり版を上げることになる（利用者側でフル
    ビルドが 1 回走るだけ）。版は上げない（今の 33 の指紋として記録した）
  - add-markdown-feature・tankan-add-diagram・dependency-update スキルと CLAUDE.md の手順に、
    `CACHE_FINGERPRINT` の書き直しとこのテストを足した
  - PR #84 でマージ（11-10）。指紋のテストは Linux・Windows・macOS で同じ値で通った
- PR (3): 運用文書の古い記述・参照先の照合・ROADMAP の整理
  - 古い記述: 7 crate の Cargo.toml の「README ロードマップ参照」→ ROADMAP「v0.21 以降の候補」/
    tankan・mikan・kabosu の Cargo.toml と ci.yml の「CLAUDE.md「リリース手順」」→ `publish-crate`
    スキル（1 段遠回りだった）/ mikan のモデルの README の「設計ノートの方針どおり」→ 理由を直接書いた
  - 参照先の照合 `scripts/check-doc-refs.sh`（ci.yml の check ジョブ・verify スキル）: 運用文書と
    コードのコメント（Cargo.toml・workflow・スクリプト・CLAUDE.md・README・スキル・各 README・Rust の
    ソース。git 管理下の 216 ファイル）が指す、リポジトリのパス・`<名前>` スキル・ROADMAP「…」/
    CLAUDE.md「…」の節が実在するかを見る。ROADMAP.md・ROADMAP-history.md（過去の記録）と、利用者の
    プロジェクトのパス（`yuzu new` が作る `.github/workflows/deploy.yml`）は見ない。わざと壊した
    パス・スキル・節の 3 つを検出することを確かめた
  - ROADMAP の整理: 2,369 行を、ROADMAP.md（「現在」・候補・これまでの版の 1 行）905 行と
    `ROADMAP-history.md`（各版の要約と内訳 v0.1〜v0.19・v0.10.1 / 10-04 / 10-07 / 10-12 のレビューの
    記録）1,552 行に分けた。レビューの記録のうち未着手の 5 項目（10-12 の R6・R7・R8・R10 と、v0.10.1 の
    開発コンテナのインストーラ）は、候補の「レビューからの持ち越し（未着手）」に写した。分ける前の
    742 行目以降の行がすべて新しい 2 ファイルのどちらかにあること・「現在」の節が変わらないことを
    スクリプトで確かめた
  - release スキル: マイナー版のときは「現在」の節を ROADMAP-history.md の「各版の内訳」へ移し、
    ROADMAP.md に 1 行を足す手順にした。タグの本文の素材も ROADMAP-history.md から取る。README の表・
    tabs.rs のコメント（Phase 50 の理由）の参照先も ROADMAP-history.md にした
- 工数: 小〜中

## v0.21 以降の候補

### dogfooding 候補（v0.13 Phase 61 からの持ち越し）

- ~~**OS ダーク追従** / **見出しパーマリンクのキーボード到達性** / **`<head>` メタ** /
  **ページメタの拡充（読了時間・文字数）**~~ — 4 件とも v0.18 の Phase 76・78・79 で
  実装済み（内訳は「これまでのリリース」の「完了済み: v0.18」）

### 10-07 見直しからの候補

- **最初の外部利用者**（10-07 見直しの F1）— v0.21 の予定（v0.20 の方針）。外部の利用者を
  得て、その使い方を観察する
  - 現状（10-07 に確認）: Release の各アセットのダウンロード数は v0.16〜v0.18 で 1 回
    ずつで、これは release.yml の publish-release が SHA256SUMS を作るために
    `gh release download` した分。直近 14 日の閲覧は約 80 回・閲覧者 1 人、issue は
    累計 0 件（`gh api`）
  - 対処案: リリースごとに、自分の取得分を引いたダウンロード数と閲覧数をリリース行に
    記録する。FAQ と Issues への導線（v0.19 Phase 84）の後に、既知の「外部への露出」
    （英語の README・他ツールとの比較・cargo-binstall）とあわせて進める
- ~~**検索の順位**（10-07 見直しの D2）~~ → v0.20 Phase 89
- **名前「yuzu」の調査**（10-07 見直しの F7）
  - 現状: GitHub でリポジトリ名に yuzu を含むものは約 2,850 件で、上位は Switch
    エミュレータの関連。crates.io の名前が取られている件は既知。商標は**未調査**
  - 対処案: 改名するか、区別のための副名を付けるかを早めに決める（URL・雛形・crate 名が
    増えるほど改名の費用が上がる）。商標を簡単に調べる
  - 調査は v0.20 の期間中にユーザが進める（v0.20 のユーザの作業。v0.21 の外部への露出の前提）

### PR #53 のレビューからの候補（10-14）

PR #53 のレビュアー側の協議で提案され、こちらの 3 人格の協議（v0.20 の方針を参照）でも
「需要はまだ仮説で、今ある機能と docs でおおむね足りる」と見た。どれも、最初の外部利用者の
導入を観察してから（上の「最初の外部利用者」）着手するかを判断する。v0.20 の Phase 91 には、
導入で実際に止まる所の修正と docs だけを入れた。再レビューは「非破壊 init → PR 検証 CI の生成 →
詳細な JSON」の順を提案したが、順序は観察で多かった止まり方から決める（3 人格とも、順序を先に
決めずに前提だけを書く意見）。前提は、init は `input.dir = "."` の扱い、JSON は v0.21 の
互換性の方針。

- **既存 Markdown 向けの非破壊 `init`** — 原稿のディレクトリを指定し、最小の yuzu.toml だけを
  作る（既存の設定があれば止まる。原稿の移動・変換・公開はしない）
  - 今あるもの: 手順は `guide/install.md` の「既存のフォルダで始める」にあり、空の yuzu.toml に
    `input.dir` を書けば build できる（10-14 に確認）。自動にできるのは手順の最初の 2 つだけで、
    deploy.yml は渡らない。リポジトリ直下の原稿は `input.dir = "."` が設定エラーなので救えない
  - 戻せない決定: サブコマンドの名前と引数。直下の原稿を扱うなら、先に `input.dir = "."` の
    扱い（dist・.yuzu・public・theme・README 類を走査から外す設計）を決める必要がある
- **公開物の詳細な JSON**（出力ファイルと、どの原稿・設定から出たか）— 人向けの 1 行の要約は
  v0.20 Phase 91 に入れた。JSON は `check --format json` と並ぶ戻せない契約になり、出力ごとの
  由来は出力マニフェスト（`yuzu-core/src/output.rs`）に無いので新しく記録する必要がある。
  v0.21 の互換性の方針の後
- **PR 検証 CI の雛形の生成** — docs の例は v0.20 Phase 91 に入れた。`yuzu new` で生成するのは、
  既存の workflow を上書きしない作りと明示の選択が要るので、観察の後

### その他の候補

- **client 描画の Mermaid の書き間違いを check で報告する**（v0.19 Phase 85 から回した）— 今は
  既定の client では build も check も何も言わない。tankan のパースだけ通し、未対応
  （`is_unsupported`）以外の構文エラーを抑制できる warning で報告する案。tankan が未対応の
  書き方を誤りとして報告しないよう、切り分け方を決めてから扱う
- **mermaid 12 系への更新** — v0.18.1 で 11 系に留めた（12.x は tankan の互換対象の版と
  あわせて決める）。Phase 82 の照合で、11 系の最新の 11.17.2 が束ねる DOMPurify・js-yaml・
  KaTeX に勧告が 5 件見つかり、影響しない理由を書いて `scripts/vendor-advisories.sh` で
  除外している（10-10）。11-06 に DOMPurify の 3 件（修正は 3.4.17）を同じく除外に足した（計 8 件）。
  12 系へ上げるときに除外を外して照合し直す
- **vaporetto 0.7 系への更新** — 0.7.0（10-24）はモデルの形式が変わり、同梱のモデル
  （vaporetto-models v0.5.0 の `bccwj-suw_c1.0`。0.6 の形式）を `model version mismatch` で読めない。
  モデルの差し替えと検索 wasm の作り直しが要るので、自動では上げず手動で行う（11-06。dependabot の
  #74・#75 を閉じ、`dependabot.yml` の ignore に足した）
  - 0.7 の形式のモデルは vaporetto 本体の v0.7.0 のリリースに添付されている（`bccwj-suw_c1.0.tar.xz` も
    ある）。`scripts/vendor-vaporetto-model.sh` の取得元（今は vaporetto-models）と SHA-256 を差し替える
  - 上流のリリースノートでは、BCCWJ だけのモデルは 0.5.0 のものを `convert_bincode_model` で新しい
    形式へ変換したもの。分かち書きは変わらない見込みだが `yuzu search` で確かめ、変わるなら
    `CACHE_FORMAT_VERSION` を上げる
  - vaporetto と vaporetto_rules は同じ版にする（片方だけだと 0.6 と 0.7 が両方入る）
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

### レビューからの持ち越し（未着手）

10-12 レビュー（R6・R7・R8・R10）と v0.10.1 レビューで見送ったもの（v0.20 Phase 93 で
ROADMAP-history.md の「レビューと見直しの記録」から写した。判断の経緯はそちら）。

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
- ⬜ **`.devcontainer/post-create.sh` の Claude Code インストーラ** — 取得した
  `install.sh` を検証せず bash へ渡している
  - 緩和されている点: **インストーラ自身がダウンロードしたバイナリを SHA-256 検証する**
    （バージョンごとの `manifest.json` の値と照合し、不一致なら削除して終了）
  - 残るギャップ: スクリプトの TOFU のみ
  - 固定を見送った理由: `install.sh` に公開チェックサムが無く、ベンダ更新のたびに
    devcontainer のビルドが壊れる
  - 着手するなら: バージョン指定（`bash -s -- <version>`）は可能なのでそこから

## これまでのリリース

各版の要約と Phase ごとの内訳は [ROADMAP-history.md](ROADMAP-history.md)。

- **v0.1**（Phase 1〜6）build / dev サーバ / 日本語検索 / llms.txt / tankan SSR / fmt・lint・check
- **v0.2**（Phase 7〜12）執筆表現 / 数式 / ページナビ / 検索のセクション単位化 / インクリメンタルビルド
- **v0.3**（Phase 13〜18）執筆の改善 / ページ Markdown の配信とコピー / 用語統一 lint
- **v0.4**（Phase 19〜23）表記ゆれの lint / 検索の同義語・誤字 / OpenAPI・JSON Schema の SSR
- **v0.5**（Phase 24〜29）tankan のスタイル構文 / コードブロックの索引
- **v0.6**（Phase 30〜35）検索の位置情報（フォーマット v3）/ フレーズ検索 / 並列化
- **v0.7**（Phase 36〜38）公開・配布の整備
- **v0.8**（Phase 39〜41）執筆機能の拡充
- **v0.9**（Phase 42〜45）執筆機能の拡充 第 2 弾
- **v0.10**（Phase 46〜49）実運用の質を上げる
- **v0.11**（Phase 50〜53）執筆機能の拡充 第 3 弾
- **v0.12**（Phase 54〜57）読む体験の完成
- **v0.13**（Phase 58〜61）lint の制御性
- **v0.14**（Phase 62〜63）設定基盤の刷新 = TOML 化
- **v0.15**（Phase 64〜67）正しさ・堅牢性
- **v0.16**（Phase 68〜71）kabosu の TOML 1.0 完全対応
- **v0.17**（Phase 72〜75）CLI の使い心地
- **v0.18**（Phase 76〜80）公開サイトの仕上げ
- **v0.19**（Phase 81〜86）配布物と第一印象
