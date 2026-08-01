//! `yuzu search <クエリ>`: ビルド済みインデックス（dist/_search）のネイティブ検索。
//! ブラウザの wasm と同一のエンジン・同一のモデルを通るため、
//! トークナイザ整合のドッグフードと CI の E2E を兼ねる

use anyhow::Context;

use crate::out::outln;

pub fn run(query: &str, limit: usize, sections: &[String], json: bool) -> anyhow::Result<()> {
    let cwd = std::env::current_dir().context("カレントディレクトリを取得できません")?;
    let root = yuzu_config::find_project_root(&cwd)?;
    let rc = yuzu_config::load(&root)?;

    let out = yuzu_index::search_dist_with_options(&rc.output_dir, query, limit, sections)?;
    let (results, total) = (out.results, out.total);

    if json {
        // 出力契約は配列のまま（section を各要素へ足す加算的変更）
        outln!("{}", serde_json::to_string_pretty(&results)?);
        return Ok(());
    }

    if results.is_empty() {
        outln!("「{query}」に一致するページはありませんでした");
        return Ok(());
    }
    let scope = match sections.is_empty() {
        true => String::new(),
        false => format!("・{}", sections.join(" / ")),
    };
    let overall = match sections.is_empty() {
        true => String::new(),
        false => format!("・全体 {} 件", out.total_unfiltered),
    };
    if total > results.len() {
        outln!(
            "全 {total} 件（上位 {} 件を表示{overall}{scope}）",
            results.len()
        );
    } else if sections.is_empty() {
        outln!("全 {total} 件");
    } else {
        outln!("全 {total} 件（全体 {} 件{scope}）", out.total_unfiltered);
    }
    // 絞り込み無指定のときはセクション別の件数を出す（ファセットの発見性）
    if sections.is_empty() {
        let facets: Vec<String> = out
            .group_counts
            .iter()
            .filter(|(_, n)| *n > 0)
            .map(|(name, n)| format!("{name} {n}"))
            .collect();
        if facets.len() > 1 {
            outln!("セクション: {}", facets.join(" / "));
        }
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
        outln!(
            "{:>2}. {:<7.3} {}  /{}{}",
            rank + 1,
            result.score,
            title,
            result.url,
            anchor
        );
        outln!("      {}", result.excerpt);
    }
    Ok(())
}
