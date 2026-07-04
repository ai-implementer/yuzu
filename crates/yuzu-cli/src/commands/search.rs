//! `yuzu search <クエリ>`: ビルド済みインデックス（dist/_search）のネイティブ検索。
//! ブラウザの wasm と同一のエンジン・同一のモデルを通るため、
//! トークナイザ整合のドッグフードと CI の E2E を兼ねる

use anyhow::Context;

pub fn run(query: &str, limit: usize, json: bool) -> anyhow::Result<()> {
    let cwd = std::env::current_dir().context("カレントディレクトリを取得できません")?;
    let root = yuzu_config::find_project_root(&cwd)?;
    let rc = yuzu_config::load(&root)?;

    let results = yuzu_index::search_dist(&rc.output_dir, query, limit)?;

    if json {
        println!("{}", serde_json::to_string_pretty(&results)?);
        return Ok(());
    }

    if results.is_empty() {
        println!("「{query}」に一致するページはありませんでした");
        return Ok(());
    }
    for (rank, result) in results.iter().enumerate() {
        println!(
            "{:>2}. {:<7.3} {}  /{}",
            rank + 1,
            result.score,
            result.title,
            result.url
        );
        println!("      {}", result.excerpt);
    }
    Ok(())
}
