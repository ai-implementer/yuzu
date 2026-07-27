// content/index.md から ```rust file="snippets/greet.rs" で引用されるサンプル。
// 設計書に実ソースを埋め込むデモ（このファイルを編集すると本文も追随する）
pub fn greet(name: &str) -> String {
    format!("こんにちは、{name}!")
}
