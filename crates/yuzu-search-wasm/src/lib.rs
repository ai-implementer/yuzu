//! TODO(Phase 3): クライアント検索クエリエンジン（wasm32-unknown-unknown）。
//!
//! 初回に manifest ＋ wasm 本体をロードし、クエリをトークナイズ → fst 上の
//! 有界編集距離でタイポ吸収 → 必要シャードだけ fetch → BM25 ランキング →
//! 上位 N 件の fragment を 2 段目フェッチ（Pagefind 型）。
//! `yuzu-index-format` の型のみに依存し、`yuzu-*` の他 crate へは依存しない。
//!
//! ビルドは wasm-bindgen-cli + wasm-opt を直接叩く方針
//! （rustwasm org サンセットのため wasm-pack には寄せない）。

// 依存方向（search-wasm ↔ index-format）の配線だけ v0.1 で確定させておく
pub use yuzu_index_format as _format;
