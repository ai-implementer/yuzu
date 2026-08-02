---
name: add-theme-asset
description: デフォルトテーマへ JS / CSS / テンプレートを追加する配線レシピ（base.jinja・rust-embed の必須アセットテスト・insta スナップショット 3 件・CI ゲート）。外部 JS とインライン script の使い分けの判断基準を含む。テーマに新しいアセットを足すときに使う。
---

# テーマ資産の追加レシピ

デフォルトテーマは `crates/yuzu-theme/assets/` を rust-embed でバイナリ埋め込み。
プロジェクト側の `theme/` に同じ相対パスを置くとファイル単位で上書きされる。

**配線が 5 箇所に散っていて、抜けても静かに動いてしまう**のが厄介
（実際に必須アセットテストへの登録が 2 件漏れていた）。

## 判断: 外部 JS か、インライン script か

**既定は外部 JS**（`assets/static/js/<name>.js`）。次のときだけ base.jinja へインラインで書く。

> **最初のペイントより前に実行される必要がある**場合。
> 外部 JS は body 末尾で読むため、初回ロード（キャッシュ無し）では
> 「既定状態で描画 → JS が効いて飛ぶ」のちらつきが見える。

前例は 2 つだけ:

| インライン script | なぜ外部にできないか |
|---|---|
| head のテーマ初期適用 | CSS 適用前に `data-theme` を決めないと FOUC が出る |
| サイドバーのスクロール位置復元 | 復元と active 項目の可視化がペイント後だと飛んで見える |

どちらも「なぜインラインなのか」をコメントで残してある。**新規に足すときも理由を書く。**

## 手順（外部 JS の場合）

### 1. JS を書く

`crates/yuzu-theme/assets/static/js/<name>.js`。**プログレッシブエンハンスメント必須** —
JS 無効でも表示が壊れないこと。既存の書き方（IIFE・`var`・`try/catch` で storage を握り潰す）に合わせる。

storage を使うならキーは `yuzu-*`。同一オリジンに複数サイトが載る場合に備え、
サイト単位の状態は `data-base="{{ base_url | safe }}"` で baseUrl を渡して名前空間を切る。

### 2. `base.jinja` へ 1 行

```jinja
<script src="{{ asset_url | safe }}js/<name>.js"></script>
```

- **`| safe` は必須**（minijinja がデフォルトで属性中の `/` をエスケープするため）
- 条件付きにするなら `{% if %}` で囲み、Rust 側のフラグを `yuzu-render/src/pipeline.rs` で渡す。
  base.jinja は `{% endif %}` を次行頭に詰める書き方で改行を制御しているので、その流儀に合わせる

### 3. 必須アセットテストへ登録（**最も忘れやすい**）

`crates/yuzu-theme/src/lib.rs` の「必須アセットが同梱されている」テストの配列へ追加する。

> ここが漏れると、埋め込み欠落を検出するガードが空振りする。
> rust-embed は **debug ではファイルシステムから読む**ため、
> 「debug では動くのに release が古い埋め込みを使い回して template not found」になる。
> （`build.rs` の `rerun-if-changed=assets` はディレクトリ監視なので、ファイル追加自体は追随する）

### 4. insta スナップショット 3 件

body 末尾の script タグ列はスナップショットに逐語で入っているので必ず動く。

```bash
INSTA_UPDATE=always cargo test -p yuzu-render
```

対象は `render_snapshot__index_html.snap` / `__guide_html.snap` / `__not_found_html.snap`
（llms 系は HTML を含まないので無関係）。フィクスチャの baseUrl は `/docs/` なので、
`data-base` を渡している場合は `data-base="/docs/"` が焼き付き、**baseUrl 追随のテストを兼ねる**。

### 5. ci.yml へ配信ゲートを 1 行

`.github/workflows/ci.yml` の docs ステップへ:

```yaml
          # 折りたたみ内へのアンカージャンプを開く JS が配信される
          grep -q 'js/details-target.js' dist/guide/writing/index.html
```

インライン script なら中身の特徴的な文字列で照合する（`grep -q 'yuzu-sidebar-scroll' dist/index.html`）。

### 6. 実機確認

`run` スキル。**`file://` では fetch も sessionStorage も期待どおりに動かない**ので
必ず `yuzu preview` / `yuzu dev` 経由。JS 無効時に壊れないことも見る。

## CSS を足す場合

`assets/static/css/theme.css` に追記。テーマ追従は CSS 変数（`--bg` / `--fg` / `--accent` 等）を使い、
色を直書きしない。新しい CSS 変数をユーザへ公開するなら `docs/content/reference/config.md` の
`theme.cssVars` の説明も更新する。

画面専用の定義（ダーク配色等）は `@media screen` に置く。印刷対応は**ファイル末尾**の
`@media print` ブロックへ足す（レスポンシブ MQ はメディアタイプ無指定 = print でも成立するため、
末尾 = 後勝ちの位置が上書きの前提。Phase 55 のブロックコメント参照）。
CSS の配信ゲートも JS と同様に ci.yml へ 1 行（例: `grep -q '@media print' dist/_assets/css/theme.css`）。

## テンプレート（partial）を足す場合

`assets/templates/partials/<name>.jinja` を作って `base.jinja` から `{% include %}`。
手順 3〜5 は JS と同じ（必須アセットテストの配列には `templates/...` のパスで登録）。
