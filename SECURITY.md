# セキュリティポリシー

To report a vulnerability, please use GitHub's private vulnerability reporting
(the "Security" tab → "Report a vulnerability"). Reports in English are welcome.

## 脆弱性の報告

GitHub の private vulnerability reporting で報告してください（リポジトリの
「Security」タブ →「Report a vulnerability」）。報告の内容は、修正版を公開するまで
報告者とメンテナだけが見られます。

公開の issue・pull request には書かないでください。

報告には、次のものがあると調査が早く進みます。

- 影響を受ける版（`yuzu --version`、または crate の版）と OS
- 再現の手順（原稿・`yuzu.toml`・実行したコマンド）
- 想定される影響（例: 読者のブラウザでスクリプトが動く、プロジェクトの外のファイルが読める）

## 対象

| 対象 | 版 |
| --- | --- |
| yuzu（[GitHub Releases](https://github.com/ai-implementer/yuzu/releases) のバイナリと `cargo install` で入れる CLI） | 最新のリリースのみ |
| crates.io の [tankan](https://crates.io/crates/tankan)・[mikan](https://crates.io/crates/mikan)・[kabosu](https://crates.io/crates/kabosu) | 最新版のみ |

古い版へ修正を戻すことはしません。修正は新しいパッチ版として出します。

yuzu では次のものが対象です。

- CLI（`build` / `dev` / `preview` / `check` / `fmt` などのサブコマンド）と、
  `yuzu dev` / `yuzu preview` が立てる手元のサーバ
- 既定のテーマと、yuzu が生成する HTML・JavaScript・検索の wasm
- 生成するサイトに同梱して配る第三者の資産（mermaid・KaTeX・分かち書きモデル）。
  上流の脆弱性は上流へ報告してください。yuzu が影響を受ける版を同梱している場合は、
  こちらにも知らせてもらえると助かります

## 脆弱性として扱わないもの

yuzu は、原稿を書く人を信頼する前提で作っています。次のものは仕様です。

- 原稿・`yuzu.toml`・テーマ（`theme/` の上書き）に書いた内容がそのまま出力されること。
  原稿に書いた生の HTML・スクリプトは、そのままページに出ます
- `yuzu dev` / `yuzu preview` をインターネットに公開して使った場合の問題。どちらも
  手元での確認用のサーバです

## 対応の目安

1 人で運用しているため、次は目安です。

- 受領の返信: 報告から 7 日以内
- 修正: パッチ版で出します。報告から 90 日以内を目安にします
- 公表: 修正版を公開した後に GitHub Security Advisory で公表します。報告者の名前を
  載せるかどうかは、報告者の希望に従います
