# vendor 物の記録

## mermaid.min.js

- 取得元: <https://cdn.jsdelivr.net/npm/mermaid@11/dist/mermaid.min.js>
- ライセンス: MIT（mermaid-js/mermaid）
- 更新手順: リポジトリルートで `scripts/vendor-mermaid.sh` を実行し、
  取得したバージョンをこのファイルに記録する
- 取得バージョン: 11.16.0（2026-07-04 取得）

> mermaid.min.js が未取得（プレースホルダ）の場合でも、ビルド・テストは
> 通る設計にしてある。` ```mermaid ` ブロックはコードのまま表示されるだけ。

## katex/

- 取得元: <https://registry.npmjs.org/katex/-/katex-0.17.0.tgz>（npm tarball の dist/）
- ライセンス: MIT（KaTeX/KaTeX）
- 更新手順: リポジトリルートで `scripts/vendor-katex.sh` を実行し、
  取得したバージョンをこのファイルに記録する
- 取得バージョン: 0.17.0（2026-07-11 取得。katex.min.js / katex.min.css / fonts 588KB）
- fonts は **woff2 のみ**同梱（css は woff2 → woff → ttf の順で参照するが、
  モダンブラウザは woff2 しか取得しない）。css が `url(fonts/...)` を相対参照する
  ため `katex/` のディレクトリ構造を崩さないこと

> katex/ が未取得の場合でもビルド・テストは通り、数式は原文（TeX ソース）
> 表示になるだけ。
