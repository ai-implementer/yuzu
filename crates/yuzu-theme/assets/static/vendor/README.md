# vendor 物の記録

## mermaid.min.js

- 取得元: <https://cdn.jsdelivr.net/npm/mermaid@11.17.2/dist/mermaid.min.js>
- ライセンス: MIT（mermaid-js/mermaid）
- 更新手順: `scripts/vendor-mermaid.sh` の `MERMAID_VERSION` と `MERMAID_SHA256` を
  セットで書き換えてリポジトリルートで実行し、このファイルの記録も更新する
  （スクリプトが sha256 を検証して不一致なら失敗する）
- 取得バージョン: 11.17.2（2026-10-08 取得。11.16.0 から更新し、11.16.1 で修正された
  勧告 5 件に対応。CDN のファイルが、npm の integrity を照合した tarball 内の
  `dist/mermaid.min.js` と同一であることを確認）
- sha256: `581ed7d74bd9048d0e3a91363927d72ef22942d7722546b27f7cc29e35390eb8`

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

> katex/ が未取得の場合でもビルド・テストは通り、数式は原文（TeX ソース）
> 表示になるだけ。
