//! `yuzu search <クエリ>`: ビルド済みインデックス（dist/_search）のネイティブ検索。
//! ブラウザの wasm と同一のエンジン・同一のモデルを通るため、
//! トークナイザ整合のドッグフードと CI の E2E を兼ねる

use anyhow::Context;

pub fn run(query: &str, limit: usize, json: bool) -> anyhow::Result<()> {
    let cwd = std::env::current_dir().context("カレントディレクトリを取得できません")?;
    let root = yuzu_config::find_project_root(&cwd)?;
    let rc = yuzu_config::load(&root)?;

    let (results, total) = yuzu_index::search_dist_with_total(&rc.output_dir, query, limit)?;

    if json {
        println!("{}", serde_json::to_string_pretty(&results)?);
        return Ok(());
    }

    if results.is_empty() {
        println!("「{query}」に一致するページはありませんでした");
        return Ok(());
    }
    if total > results.len() {
        println!("全 {total} 件（上位 {} 件を表示）", results.len());
    } else {
        println!("全 {total} 件");
    }
    for (rank, result) in results.iter().enumerate() {
        let title = match &result.heading {
            Some(heading) => format!("{} › {}", result.title, heading),
            None => result.title.clone(),
        };
        let anchor = result
            .anchor
            .as_deref()
            .map(|a| format!("#{a}"))
            .unwrap_or_default();
        println!(
            "{:>2}. {:<7.3} {}  /{}{}",
            rank + 1,
            result.score,
            title,
            result.url,
            anchor
        );
        println!("      {}", result.excerpt);
    }
    Ok(())
}
