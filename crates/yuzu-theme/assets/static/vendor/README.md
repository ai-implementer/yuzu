# vendor 物の記録

このファイルは開発用の記録で、テーマの埋め込みから外している（`crates/yuzu-theme/src/lib.rs`
の `#[exclude]`。dist には出ない）。利用者のサイトへ配るライセンス表記は
`THIRD-PARTY-LICENSES.txt`（`scripts/third-party-licenses.sh vendor` が mermaid と KaTeX の
材料 `crates/yuzu-theme/licenses/*.txt` から組む）。

## mermaid.min.js

- 取得元: <https://registry.npmjs.org/mermaid/-/mermaid-11.17.2.tgz>（npm tarball の
  `dist/mermaid.min.js`。2026-10-10 に CDN 直取りから切り替え。中身は同一）
- ライセンス: MIT（mermaid-js/mermaid）。mermaid.min.js は 60 の npm パッケージ（55 種。
  d3・dagre・cytoscape・DOMPurify 等。ISC・MIT・BSD-3-Clause・MPL-2.0 OR Apache-2.0）を
  束ねており、そのライセンス文は `crates/yuzu-theme/licenses/mermaid.txt`
- 更新手順: `scripts/vendor-mermaid.sh` の `MERMAID_VERSION` と `MERMAID_ARCHIVE_SHA256` を
  セットで書き換えてリポジトリルートで実行し、このファイルの記録も更新する
  （スクリプトがアーカイブの sha256 を展開前に検証して不一致なら失敗する。束ねたパッケージは
  tarball 同梱の source map の `sources` から取り出し、各パッケージの tarball を npm の
  integrity と照合してから LICENSE を読む。jq が要る）
- 取得バージョン: 11.17.2（2026-10-08 取得。11.16.0 から更新し、11.16.1 で修正された
  勧告 5 件に対応）
- アーカイブ sha256: `6ad2f42c3fc26bbf9e45cbb6d11898972573ea52b33a5f4ff51952899f950ffd`
- mermaid.min.js の sha256: `581ed7d74bd9048d0e3a91363927d72ef22942d7722546b27f7cc29e35390eb8`

> mermaid.min.js が未取得（プレースホルダ）の場合でも、ビルド・テストは
> 通る設計にしてある。` ```mermaid ` ブロックはコードのまま表示されるだけ。

## katex/

- 取得元: <https://registry.npmjs.org/katex/-/katex-0.18.11.tgz>（npm tarball の dist/）
- ライセンス: MIT（KaTeX/KaTeX）
- 更新手順: `scripts/vendor-katex.sh` の `KATEX_VERSION` と `KATEX_ARCHIVE_SHA256` を
  セットで書き換えてリポジトリルートで実行し、このファイルの記録も更新する
- 取得バージョン: 0.18.11（2026-10-08 取得。0.17.0 から更新し、0.18.2 で修正された
  勧告 GHSA-238p-pmpm-9mq7 に対応。katex.min.js / katex.min.css / fonts 592KB）
  - 0.18.0 で KaTeX 内部の CSS クラスに接頭辞が付いた（破壊的変更）。テーマが参照する
    `.katex-display` は 0.18 でも同じ名前・同じ規則で残っている（theme.css の数式の節）
- アーカイブ sha256: `a11d6ab44180c6e25a09108c6c391866d0600b49a61ef725368d567f4918ffc3`
  （スクリプトは**展開する前**にこれを照合する。アーカイブが一致すれば中身は一意なので
  fonts 20 ファイルもこれで覆える。完成形は `katex.new` へ組んでから差し替えるため、
  失敗しても既存の同梱物は壊れない）
- 同梱物の sha256（記録用）: katex.min.js
  `607cb26db8e23fba98ba8e1c0b764ff4c67af35106e59316ec5449a94083fa68` / katex.min.css
  `b8511ee880e4981ac28bbc3d675a90cb9fa080e275f06d648147ed316cb421ba`
- fonts は **woff2 のみ**同梱（css は woff2 → woff → ttf の順で参照するが、
  モダンブラウザは woff2 しか取得しない）。css が `url(fonts/...)` を相対参照する
  ため `katex/` のディレクトリ構造を崩さないこと
- ライセンス文は `crates/yuzu-theme/licenses/katex.txt`（tarball の LICENSE と、フォントの
  生成元 katex-fonts の LICENSE。katex-fonts も MIT・Copyright (c) 2018 Khan Academy。
  コミット `feee984b` で固定して取得する）

> katex/ が未取得の場合でもビルド・テストは通り、数式は原文（TeX ソース）
> 表示になるだけ。
