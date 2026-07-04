# theme/ — テーマの上書き

このディレクトリにファイルを置くと、yuzu 同梱のデフォルトテーマを
**同じ相対パスのファイル単位で**上書きできます（無ければ同梱版が使われます）。

```
theme/
├─ templates/            # minijinja テンプレート
│  ├─ base.jinja         # ページ全体の骨格
│  ├─ page.jinja         # 本文レイアウト
│  └─ partials/          # sidebar.jinja / toc.jinja / header.jinja
└─ static/               # dist/_assets/ にコピーされる静的物
   ├─ css/theme.css
   └─ js/theme.js
```

デフォルトテーマの実体はリポジトリの `crates/yuzu-theme/assets/` にあります。
カスタマイズはそこからコピーして編集するのが手軽です。
