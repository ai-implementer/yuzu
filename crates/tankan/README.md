# tankan 🍊

Mermaid 互換のダイアグラムテキストを **SVG** に変換する純 Rust ライブラリ。
名前は柑橘のタンカン（桶柑）から。

- **純 Rust・I/O なし・時刻/乱数非依存** — `wasm32-unknown-unknown` でもそのまま動く
- **特定ツール非依存の汎用設計**（開発は [yuzu](https://github.com/ai-implementer/yuzu)
  monorepo で行っているが `yuzu-*` に依存しない独立ライブラリ）
- **フォールバック前提**: 未対応の図種・構文は `Error` で明示し、呼び出し側が
  mermaid.js クライアント描画等へ落とせる（全図種対応を待たずに実戦投入できる）

## インストール

```bash
cargo add tankan
```

## 互換性の定義

1. **構文互換**: 対応図種について mermaid.js 公式ドキュメントの例文をエラーなく受理する
2. **意味互換**: 出力 SVG は「同じ図として読める」。ピクセル一致は目標にしない
   （mermaid.js 自体、バージョン間で描画が変わる）
3. **フォールバック**: 未対応は `Error::UnsupportedDiagram` / `UnsupportedSyntax` で検出可能

## 対応状況

| 図種 | 状態 |
|---|---|
| sequenceDiagram | ✅（participant/actor・矢印 10 種・activation・Note・loop/alt/opt/par/critical/break/rect・autonumber・box・title） |
| flowchart / graph | ✅（ノード形状 15 種・エッジ全種（実線/点線/太線/不可視・長さ・端点・ラベル 2 形）・チェーン/`&`・TB/BT/LR/RL・subgraph（ネスト・内部 direction）・ノードのスタイル（`style`/`classDef`（`default` 含む）/`class`/`:::`。fill/stroke/stroke-width/stroke-dasharray/color をインライン適用）。linkStyle/click と `@{}` 新記法はフォールバック） |
| stateDiagram / stateDiagram-v2 | ✅（`[*]`・ラベル付き遷移・`state "説明" as s`・composite（ネスト）・direction・`<<choice/fork/join>>`・note・concurrency `--`・状態ボックスのスタイル（`classDef`（`default` 含む）/`class` 文/`:::`/`style`）。レイアウトは flowchart エンジンを共用） |
| erDiagram | ✅（全基数×識別/非識別・属性ブロック（PK/FK/UK・引用符コメント）・エイリアス `E[表示名]`・引用符名・単独エンティティ宣言・エンティティのスタイル（`classDef`（`default` 含む）/`class` 文/`:::`/`style`）・direction は受理して無視） |
| classDiagram / classDiagram-v2 | ✅（クラス定義（波括弧ブロック・`X : member`）・可視性 `+ - # ~`・末尾 `* $`・関係 8 種（継承/コンポジション/集約/関連/リンク/依存/実現/点線）・ラベル・多重度・ジェネリクス `~T~`→`<T>`・`<<interface>>` 等アノテーション・クラスボックスのスタイル（`classDef`（`default` 含む）/`cssClass` 文/`:::`/`style`。宣言の `class` と衝突しないよう一括適用は `cssClass`）。note/click/namespace はフォールバック） |
| pie | ✅（`showData`・`title`（ヘッダ/単独行/frontmatter）・扇形＋凡例。塗りは CSS 変数 `--tankan-pie-1`〜`8` で上書き可） |
| gantt | ✅（`dateFormat YYYY-MM-DD`・section・done/active/crit/milestone・after 依存・開始省略・excludes（weekends/曜日/日付 = 働き日消化＋網掛け）・weekend・axisFormat・tickInterval。時分単位・until 等はフォールバック。**today 線は描かない**＝時刻非依存。`todayMarker off` のみ受理） |
| mindmap | ✅（インデント階層・中央ルートから左右へ振り分ける tidy tree・ノード形状 7 種（四角/角丸/円/バン/雲/六角形/既定）・ブランチごとのパレット色・幅ベースの自動折返し。`:::class` / `::icon` 行は受理） |
| timeline | ✅（`title`・section 帯・時期ごとのイベント縦積み（`: 継続行` の複数イベント可）・等間隔カラム・自動折返し） |
| その他 | フォールバック |

レイアウトは自作の **Sugiyama 法サブセット**（閉路除去 → longest-path 層割当 →
ダミーノード → barycenter 交差削減 → median 座標決定）。全ステップ決定的で、
同一入力からは常にバイト単位で同一の SVG が出る。

## 使い方

```rust
let options = tankan::Options::default();
match tankan::render_svg(source, &options) {
    Ok(svg) => { /* インライン SVG として埋め込む */ }
    Err(e) if e.is_unsupported() => { /* クライアント描画等へフォールバック */ }
    Err(e) => { /* 構文エラー（書き間違いの可能性）。警告してフォールバック */ }
}
```

- 色は `Theme` の CSS 色文字列で指定する。`"var(--fg, #333)"` のような CSS 変数参照を
  渡せば、HTML にインライン埋め込みした SVG がページのテーマ（ダークモード等）に追従する
- 同一ページに複数の SVG を埋め込む場合は `Options.id_prefix` を SVG ごとに変えること
  （`<marker>` の id は文書グローバルのため）

## テキスト計測について

ブラウザなしで文字幅を測るため、ASCII はメトリクステーブル、非 ASCII は
East Asian Width（全角 = 1em）による近似を使う。閲覧側はシステムフォントで
描画されるため厳密一致は原理的に不可能で、余白側に倒した安全な近似としている。

## ライセンス

MIT または Apache-2.0 のデュアルライセンス。
