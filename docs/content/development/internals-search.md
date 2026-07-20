---
title: 検索の内部設計
order: 1
description: トークナイザ整合・セクション単位インデックス・位置情報・OPFS キャッシュ
---

# 検索の内部設計

## 最重要制約: トークナイザ整合

index 時（ネイティブ）と query 時（wasm）で**同一のトークナイザコード
（mikan）＋同一のモデルバイト**を使います。分かち書きが
1 トークンでもずれると検索は静かに壊れるため、抜粋生成・ハイライトの
ロジックも mikan に 1 実装だけ置き、native / wasm で共有します。

`yuzu search` はブラウザとまったく同じエンジンを通るので、この整合の
検証にも使えます。

## インデックスの構造

- **doc = セクション（h2 / h3 境界）**。検索結果が「ページ › 見出し」に
  なり、`#アンカー` へ直接ジャンプできます。ページタイトルの重みは
  リード doc（アンカーなし）だけに載せ、タイトル検索の重複ヒットを
  防ぎます
- **転置インデックス＋BM25**。postings は delta + varint の自前
  フォーマットで、term 辞書は fst、シャード分割（term\_id の連続範囲）で
  2 段フェッチにします（Pagefind 型。ブラウザは必要なシャードだけ取得）
- **出現位置つき（フォーマット v3）**: postings に term の出現位置
  （セクション内トークン位置の delta varint 列）を持ちます。
  フィールド間（タイトル / 見出し / 本文）に位置ギャップを挟み、
  偽の隣接を防ぎます。これが**フレーズ検索**（隣接照合の filter）と
  **近接ブースト**（クエリ順に隣接するペアへの soft な加点）の土台です
- 分かち書きは **vaporetto**（同梱モデル、圧縮 372KB）。形態素解析器への
  差し替えは実測の結果、転送量が 9〜35 倍になるため見送りました
  （静的ホスティングだけで動く方針を優先）

## クエリ処理

1. クエリを分かち書きし、`lint.terms` ＋ `search.synonyms` 由来の辞書で
   **クエリ拡張**（ゆれ表記 → 正表記）
2. 文字単位の編集距離 1 まで許容する**タイポトレランス**
   （levenshtein\_automata の文字単位 DFA）
3. `"..."` の引用部は**フレーズ照合**: トークナイズ → 位置の隣接照合で
   filter（引用部はタイポ・同義語展開なしの完全一致）
4. BM25 スコアに**近接ブースト**（引用符なしの複数語がクエリ順に隣接して
   出現するページを上位へ。ヒット集合は不変）

## ブラウザ側の配信と OPFS

`dist/_search/` は manifest・term 辞書（fst）・postings シャード・
fragment・分かち書きモデル・wasm の静的ファイル群です。ブラウザは
manifest を 1 fetch した後、クエリに必要なシャードだけを取得します。

manifest の `contentHash`（fst ＋ 全シャード ＋ モデルバイトの sha256）を
版として、対応環境では **OPFS** にブロブを保存します。再訪問時はハッシュが
一致すればローカルから読み、不一致・OPFS 非対応・非セキュアコンテキストでは
フェッチのみの経路へフォールバックします。オーケストレーションは
`search-client.js` ＋汎用ブロブキャッシュ `opfs-cache.js`（DOM 非依存）で、
テーマの `search-ui.js` は DOM / UX 層に純化しています。

## vendor 資産

| 資産 | 置き場所 | 更新スクリプト |
| --- | --- | --- |
| vaporetto モデル | `crates/mikan/assets/model/` | `scripts/vendor-vaporetto-model.sh` |
| 検索 wasm 成果物 | `crates/yuzu-index/assets/search/` | `scripts/build-search-wasm.sh` |
| mermaid.min.js | `crates/yuzu-theme/assets/static/vendor/` | `scripts/vendor-mermaid.sh` |
| KaTeX | `crates/yuzu-theme/assets/static/vendor/` | `scripts/vendor-katex.sh` |

wasm 成果物の再生成では、wasm-bindgen-cli を workspace の
`wasm-bindgen = "=x.y.z"` と完全同一バージョンにする必要があります。
