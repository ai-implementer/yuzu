//! TODO(Phase 3): 検索インデクサ。
//!
//! ビルド時に本文を抽出し、日本語トークナイズ（vaporetto）→ 転置インデックス＋
//! BM25 統計 → term 辞書の fst 化 → シャーディング → 静的成果物
//! （`dist/_search/`）を出力する。索引フォーマット型は `yuzu-index-format` に
//! 切り出し、クエリ側（`yuzu-search-wasm`）と共有する。
//! 全体像は README.md のロードマップ（Phase 3）を参照。
