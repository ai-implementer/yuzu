# vendor 物の記録

## mermaid.min.js

- 取得元: <https://cdn.jsdelivr.net/npm/mermaid@11.16.0/dist/mermaid.min.js>
- ライセンス: MIT（mermaid-js/mermaid）
- 更新手順: `scripts/vendor-mermaid.sh` の `MERMAID_VERSION` と `MERMAID_SHA256` を
  セットで書き換えてリポジトリルートで実行し、このファイルの記録も更新する
  （スクリプトが sha256 を検証して不一致なら失敗する）
- 取得バージョン: 11.16.0（2026-07-04 取得）
- sha256: `74d7c46dabca328c2294733910a8aa1ed0c37451776e8d5295da38a2b758fb9b`

> mermaid.min.js が未取得（プレースホルダ）の場合でも、ビルド・テストは
> 通る設計にしてある。` ```mermaid ` ブロックはコードのまま表示されるだけ。

## katex/

- 取得元: <https://registry.npmjs.org/katex/-/katex-0.17.0.tgz>（npm tarball の dist/）
- ライセンス: MIT（KaTeX/KaTeX）
- 更新手順: `scripts/vendor-katex.sh` の `KATEX_VERSION` と `KATEX_ARCHIVE_SHA256` を
  セットで書き換えてリポジトリルートで実行し、このファイルの記録も更新する
- 取得バージョン: 0.17.0（2026-07-11 取得。katex.min.js / katex.min.css / fonts 588KB）
- アーカイブ sha256: `252efd48f892d178136fe3ba3530d3718b2b087ea81c3a40a877227bc61d5256`
  （スクリプトは**展開する前**にこれを照合する。アーカイブが一致すれば中身は一意なので
  fonts 20 ファイルもこれで覆える。完成形は `katex.new` へ組んでから差し替えるため、
  失敗しても既存の同梱物は壊れない）
- 同梱物の sha256（記録用）: katex.min.js
  `45fbe318fea878fdc0a111913dc1f87894b2c439360d0228c086ef313f213efc` / katex.min.css
  `a34ad8fc188e8f5a3af7ceaa2a58d7210c6c9171335a15bff2b48ebcd6a6f5b0`
- fonts は **woff2 のみ**同梱（css は woff2 → woff → ttf の順で参照するが、
  モダンブラウザは woff2 しか取得しない）。css が `url(fonts/...)` を相対参照する
  ため `katex/` のディレクトリ構造を崩さないこと

> katex/ が未取得の場合でもビルド・テストは通り、数式は原文（TeX ソース）
> 表示になるだけ。
