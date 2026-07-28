---
name: add-markdown-feature
description: 新しい Markdown 記法・本文レンダリング機能を yuzu へ追加するレシピ（コンテンツインクルード・図表相互参照・折りたたみ等と同型）。キャッシュ無効化・fmt 温存・lint/check・docs ゲートまでの配線漏れを防ぐ。yuzu.jsonc への設定キー追加も含む。
---

# Markdown 記法・本文レンダリング機能の追加レシピ

Phase 39（コードブロック表示メタ）/ 40（エイリアス）/ 42（インクルード）/ 43（図表相互参照）/
44（折りたたみ）/ 45（サイト通し番号）が同じ形。触るファイルが 10 前後に散らばり、
**忘れると静かに壊れる手順**があるので順番に確認する。

## 手順

### 1. 解釈の実装（yuzu-core）

`crates/yuzu-core/src/markdown/<機能>.rs` を新設し、`markdown/mod.rs` の `mod` 宣言と
本文パイプラインへ接続する。**comrak を触ってよいのは `markdown/` の中だけ**（公開 API は comrak 非依存）。

フェンス情報文字列の拡張なら新モジュールではなく `markdown/fence.rs` を拡張する
（`parse_fence_info` が描画・検索・lint の単一実装。lint 用に `parse_fence_info_detailed` も更新）。

### 2. `CACHE_FORMAT_VERSION` を上げる（**忘れやすい**）

本文 HTML・検索 tf・llms 正規化 md のいずれかの**生成結果が変わるなら必須**。
`crates/yuzu-core/src/cache.rs` の定数を +1 し、doc コメントの履歴に 1 行足す。

> Phase 44（Admonition → `<details>`）はこれを忘れて後追いで v13 を計上した。
> envKey に `CARGO_PKG_VERSION` が入るのでリリースを跨げば救済されるが、
> **開発中は `.yuzu/cache/` が古い HTML を返し続ける**。

### 3. クロスページ依存があれば routesKey へ

先行ページの状態で後続ページの出力が変わる機能は、`crates/yuzu-cli/src/commands/build.rs` の
routesKey にキーを足して本文キャッシュを無効化する。前例は
`crossref.numbering: "site"` のときの「ラベル個数」。

### 4. `yuzu fmt` の温存を確認

不変条件は「format_commonmark の正規形・frontmatter はバイト温存・冪等・差分なしなら書かない」。
新記法が comrak のエスケープで壊れるなら `markdown/mod.rs` の `restore_yuzu_syntax` に
**対象を絞って**復元処理を足す（`{#fig:x}` の行末ラベルと Admonition マーカーが前例。
一般の `#` エスケープは触らない）。

**復元が効くのは fmt 経路だけ**で `normalize_markdown`（llms-full.txt）には効かない。これは仕様。

### 5. lint / check

- lint ルールを足すなら `lint.rs` に `check_*` を書いて `lint_page` / `lint_project` へ 1 行。
  **`lint.rs` 冒頭のモジュール doc のルール一覧**と `docs/content/guide/quality.md` の表も更新する
- リンク・アンカーに関わるなら `linkcheck.rs`（図表ラベルは有効アンカーに含める / エイリアスは含めない）
- 新しいエラー種別は `diagnostics.rs` の規約に沿って `<機能>-error` 形式で

### 6. 検索・llms への反映方針を決める

`markdown/mod.rs` の `extract_plain_sections`（検索）と `normalize_markdown`（llms）で、
**展開するのか原文のままか**を意識的に決める。前例: インクルードは検索では展開、
llms.txt では原文のまま（fmt 正規形との一致を保つため）。

### 7. スナップショット

```bash
INSTA_UPDATE=always cargo test -p yuzu-core    # body_snapshot.rs にケースを足してから
```

本文 HTML が変わるなら `crates/yuzu-render/tests/snapshots/` の 3 件も動く。
差分は必ず目視してから承認する（`cargo insta` が無い環境では上記で直接更新される）。

### 8. 配布物への反映

- `crates/yuzu-cli/scaffold/`（`index.md` / `getting-started.md`）に実例
- `docs/content/guide/*.md` に説明を書く（**dogfooding**。表記は長音符なし・`yuzu fmt` の正規形）
- `ROADMAP.md` の Phase 行（README は入口専用なので触らない）

### 9. ci.yml へ grep ゲートを 1 行（**毎回やる**）

`.github/workflows/ci.yml` の docs ステップに、生成物を照合する行を足す。
Phase 42〜45 すべてが実施している:

```yaml
          # 図表番号: キャプションの採番と参照リンクの自動補完
          grep -q 'id="fig:deps"' dist/development/index.html
```

## `yuzu.jsonc` に設定キーを足す場合（4 手）

1. `crates/yuzu-config/src/schema.rs` に camelCase のフィールド＋既定値
2. 消費側（yuzu-core / yuzu-render / yuzu-index）へ配線
3. `crates/yuzu-cli/scaffold/yuzu.jsonc` にコメント付きで追記
4. `docs/content/reference/config.md` の**全キー例と該当セクションの表の 2 箇所**

frontmatter のキーを足す場合は `crates/yuzu-core/src/frontmatter.rs` の `KNOWN_KEYS` にも足す
（忘れても構造体との乖離検知テストが落ちるので気づける）。

## 罠

- **comrak の AST 構造変更は「走査で集めて後段で適用」**。`descendants()` のイテレート中に
  木を変えると *tree modified during iteration* でパニック。段落 → `HtmlBlock` 化は
  **子を先に detach しないと** `InvalidChildType` でパニック（HtmlBlock は子を持てない）。
  値の変更（URL 書き換え等）だけは走査中で安全
- **doc コメント内でフェンスを入れ子にすると doctest がコンパイルされて失敗する**（インデント記法も同様）。散文で書く
- **アンカー採番は extract_meta / HTML 化 / extract_plain_sections の 3 経路とも全見出しを文書順に** Anchorizer へ通す。片方で飛ばすと id がずれる
- `comrak_options_keep_footnotes` は fmt / normalize / linkcheck 専用。HTML レンダと extract_meta に使うと壊れる
- **`docs/yuzu.jsonc` の 15-21 行目**はインクルードの `lines=` で引用されている。この範囲を動かすと docs の原稿と ci.yml のゲートが同時に壊れる

## 仕上げ

`verify` スキル（CI 相当 + e2e + docs サイト検証）→ 表示に関わるなら `run` スキルで実機確認。
