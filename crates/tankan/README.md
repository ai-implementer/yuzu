# tankan 🍊

Mermaid 互換のダイアグラムテキストを **SVG** に変換する純 Rust ライブラリ。
名前は柑橘のタンカン（桶柑）から。

- **純 Rust・I/O なし・時刻/乱数非依存** — `wasm32-unknown-unknown` でもそのまま動く
- **特定ツール非依存の汎用設計**（yuzu ワークスペースに同居しているが `yuzu-*` に依存しない。
  必要になれば別リポジトリ / crates.io へ分離できる形を保つ）
- **フォールバック前提**: 未対応の図種・構文は `Error` で明示し、呼び出し側が
  mermaid.js クライアント描画等へ落とせる（全図種対応を待たずに実戦投入できる）

## 互換性の定義

1. **構文互換**: 対応図種について mermaid.js 公式ドキュメントの例文をエラーなく受理する
2. **意味互換**: 出力 SVG は「同じ図として読める」。ピクセル一致は目標にしない
   （mermaid.js 自体、バージョン間で描画が変わる）
3. **フォールバック**: 未対応は `Error::UnsupportedDiagram` / `UnsupportedSyntax` で検出可能

## 対応状況

| 図種 | 状態 |
|---|---|
| sequenceDiagram | ✅ M1（participant/actor・矢印 10 種・activation・Note・loop/alt/opt/par/critical/break/rect・autonumber・box・title） |
| flowchart / graph | ✅ M2（ノード形状 15 種・エッジ全種（実線/点線/太線/不可視・長さ・端点・ラベル 2 形）・チェーン/`&`・TB/BT/LR/RL・subgraph（ネスト・内部 direction）。スタイル系（style/classDef/class/linkStyle/click/`:::`）と `@{}` 新記法はフォールバック） |
| stateDiagram / stateDiagram-v2 | ✅ M3（`[*]`・ラベル付き遷移・`state "説明" as s`・composite（ネスト）・direction・`<<choice/fork/join>>`・note・concurrency `--`。レイアウトは flowchart エンジンを共用。classDef 等はフォールバック） |
| ER / gantt | 🔜 M4 |
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
