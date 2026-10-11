# theme/ — テーマの上書き

このディレクトリにファイルを置くと、yuzu 同梱のデフォルトテーマを
**同じ相対パスのファイル単位で**上書きできます（無ければ同梱版が使われます）。

```
theme/
├─ templates/            # minijinja テンプレート
│  ├─ base.jinja         # ページ全体の骨格
│  ├─ page.jinja         # 本文レイアウト
│  ├─ 404.jinja          # 存在しない URL 用のページ
│  ├─ redirect.jinja     # aliases のリダイレクト HTML
│  └─ partials/          # header / sidebar / toc / toc-mobile / breadcrumb / pager
└─ static/               # dist/_assets/ にコピーされる静的物
   ├─ css/theme.css      # 色は CSS 変数（--bg / --fg / --accent …）経由で
   └─ js/                # theme / nav / scrollspy / copy-button / search-ui …
```

上書きのしかたと、yuzu を更新したときの注意は、ドキュメントの「見た目の調整」を
参照してください: https://ai.implementer.net/yuzu/guide/appearance/

同梱版のファイルは、この雛形を作った yuzu の版のソースにあります。上書きするときは
そこからコピーして編集するのが手軽です:
https://github.com/ai-implementer/yuzu/tree/v__YUZU_VERSION__/crates/yuzu-theme/assets
