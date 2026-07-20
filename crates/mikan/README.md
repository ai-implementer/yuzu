# mikan 🍊

Pagefind 型（ビルド時にインデックスを静的ファイルへ焼き込み、ブラウザは 2 段フェッチで
検索する方式）の日本語全文検索インデックスフォーマットと、その読み書きロジック
（BM25 クエリエンジン）。名前は柑橘のミカン（蜜柑）から。

ネイティブのインデクサ（`yuzu search`）とブラウザのクエリエンジン（wasm）が、この
crate の**同一コード**を共有する。開発は [yuzu](https://github.com/ai-implementer/yuzu)
monorepo で行っているが、mikan 自体は `yuzu-*` に依存しない独立ライブラリ。

## インストール

```bash
cargo add mikan
```

## 特徴

- **`yuzu-*` の上位層（テンプレート・設定・サイトモデル）に依存しない**独立ライブラリ
- ただし **フォーマット非依存ではない**: このクレートは自前のワイヤフォーマット
  （`manifest.json` / `terms.fst` / `index/NNNN.bin` シャード / `fragment/*.json`）の
  読み書き実装であり、Pagefind の JS クライアントが Pagefind 独自フォーマットに
  結合しているのと同型の設計。主張しているのは「アプリの上位層への非依存」であって
  「フォーマットへの非依存」ではない
- I/O なし（fetch・ファイル読み書きは呼び出し側の責務）。wasm バイナリを軽く保つため
  依存を必要最小限に保つ（`fst` / `levenshtein_automata` / `ruzstd` / `serde(_json)` /
  `thiserror` / `unicode-normalization` / `vaporetto(_rules)` のみ）

## 最重要の整合制約

index 時（ネイティブ）と query 時（wasm）で**同一トークナイザ・同一モデルバイト**を
使うこと。ズレると検索がヒットしない。[`Tokenizer`] はこのクレートに 1 実装だけ置き、
モデル（`.model.zst`）はインデクサが `dist/_search/model.zst` へそのままコピーして
両側で読む。

## 使い方（書き側: インデックス構築）

Markdown からのセクション抽出（tf・出現位置の計算）は呼び出し側の責務。
すでに計算済みの tf・位置を渡すと、doc_id 採番から postings/fst/シャード/manifest
バイト列までを構築する（I/O なし。書き出しは呼び出し側の責務）:

```rust
use mikan::{Bm25Params, BuildOptions, DocumentInput, SectionInput, TokenizerMeta, TypoParams, build};

let docs = vec![DocumentInput {
    title: "ホーム".to_string(),
    url: "".to_string(),
    sections: vec![SectionInput {
        anchor: None,
        heading: None,
        text: "ようこそ".to_string(),
        doc_len: 1,
        tf: vec![("ようこそ".to_string(), 1, vec![0])],
    }],
}];
let opts = BuildOptions {
    tokenizer: TokenizerMeta {
        kind: "vaporetto".into(),
        model_file: "model.zst".into(),
        model_sha256: "…".into(),
    },
    bm25: Bm25Params::default(),
    typo: TypoParams { enabled: true, max_edits: 1 },
    max_terms_per_shard: 16384,
    synonyms: vec![],
};

let built = build(&docs, &opts)?;
// built.manifest / built.terms_fst / built.shards / built.fragments を
// dist/_search/ 一式として書き出すのは呼び出し側（yuzu-index::builder）の責務
```

`built.manifest.content_hash` は空文字で返る。この値（ブラウザ側 OPFS キャッシュの
版管理に使う識別子）は `terms_fst` ＋ 全シャード ＋ モデルバイトを連結したハッシュで、
このクレートに `sha2` 依存を持ち込まないよう計算は呼び出し側が行う設計にしている。

## 使い方（読み側: クエリエンジン）

```rust
use mikan::SearchEngine;

let mut engine = SearchEngine::new(&manifest_json, terms_fst, &model_zst)?;
for shard_id in engine.needed_shards("検索") {
    engine.load_shard(shard_id, &fetch_shard(shard_id))?;
}
let hits = engine.search("検索", 10); // Vec<Hit>（doc_id と BM25 スコア）
```

ブラウザから使う場合の wasm-bindgen ラッパと、フェッチ・OPFS キャッシュ・wasm 起動を
まとめた手書き JS クライアント（`search-client.js` / `opfs-cache.js`）は、yuzu の
`mikan-wasm` crate（`crates/mikan-wasm/`）にある。

## ライセンス

MIT または Apache-2.0 のデュアルライセンス。
