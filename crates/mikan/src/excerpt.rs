//! クエリ一致箇所周辺の動的抜粋（native の `yuzu search` と wasm の両方が使う 1 実装）。
//!
//! tokens は [`crate::Tokenizer::tokenize`] 済み（NFKC + lowercase）だが text は
//! 生テキスト。大文字・全角の不一致は「元テキストを 1 文字ずつ NFKC + lowercase した
//! 影文字列＋位置マップ」で吸収する。文字を跨ぐ合成（半角カナ＋濁点の結合等）は
//! per-char では再現できないが、その token がハイライトされないだけで
//! 先頭フォールバック表示に安全に劣化する。
//!
//! fragment.text をビルド時に正規化して持つ案は不採用: 表示される抜粋まで
//! 小文字・半角化されて品質が落ちるため、生テキスト保存＋クエリ時マッチングとする。
//!
//! 返り値はオフセットではなく**セグメント列**。JS（UTF-16）と Rust（char/byte）の
//! 位置換算問題を持ち込まず、表示側は text/mark を並べるだけでよい。

use serde::Serialize;
use unicode_normalization::UnicodeNormalization;

/// 抜粋の 1 断片。`mark == true` はクエリ token の一致箇所（`<mark>` / 強調表示）
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ExcerptSegment {
    pub text: String,
    pub mark: bool,
}

/// 影文字列と同じ正規化（char ごとに NFKC → lowercase）。
/// フレーズをまるごと 1 needle として渡すときに使う
/// （token は Tokenizer 側で正規化済みなので不要）
pub(crate) fn normalize_for_match(s: &str) -> String {
    s.chars()
        .flat_map(|c| c.nfkc())
        .flat_map(|n| n.to_lowercase())
        .collect()
}

/// text から tokens の一致箇所周辺 `max_chars` 文字（char 単位）の抜粋を作る。
/// 一致が無ければ先頭 `max_chars` ＋（切れたら）`…` の非 mark 1 断片を返す
pub fn make_excerpt(text: &str, tokens: &[String], max_chars: usize) -> Vec<ExcerptSegment> {
    let chars: Vec<char> = text.chars().collect();
    if chars.is_empty() || max_chars == 0 {
        return Vec::new();
    }

    // 影文字列: 元 char ごとに NFKC → lowercase した文字列を norm に連結し、
    // norm の各 char がどの元 char 由来かを map に記録する（1:N 展開も同一元添字）
    let mut norm: Vec<char> = Vec::with_capacity(chars.len());
    let mut map: Vec<usize> = Vec::with_capacity(chars.len());
    for (i, c) in chars.iter().enumerate() {
        for n in c.nfkc() {
            for l in n.to_lowercase() {
                norm.push(l);
                map.push(i);
            }
        }
    }

    // 各 token の一致範囲（元 char 添字の [start, end)）を収集
    let mut ranges: Vec<(usize, usize)> = Vec::new();
    let mut seen: Vec<&str> = Vec::new();
    for token in tokens {
        if token.is_empty() || seen.contains(&token.as_str()) {
            continue;
        }
        seen.push(token);
        let needle: Vec<char> = token.chars().collect();
        if needle.len() > norm.len() {
            continue;
        }
        for start in 0..=(norm.len() - needle.len()) {
            if norm[start..start + needle.len()] == needle[..] {
                ranges.push((map[start], map[start + needle.len() - 1] + 1));
            }
        }
    }

    if ranges.is_empty() {
        // 一致なし: 先頭 max_chars のフォールバック
        let cut = max_chars.min(chars.len());
        let mut out: String = chars[..cut].iter().collect();
        if cut < chars.len() {
            out.push('…');
        }
        return vec![ExcerptSegment {
            text: out,
            mark: false,
        }];
    }

    // 範囲をソートし、重なり・隣接をマージ
    ranges.sort_unstable();
    let mut merged: Vec<(usize, usize)> = Vec::with_capacity(ranges.len());
    for (s, e) in ranges {
        match merged.last_mut() {
            Some(last) if s <= last.1 => last.1 = last.1.max(e),
            _ => merged.push((s, e)),
        }
    }

    // 窓決定: 各一致の少し手前を開始候補とし、最も多くの一致範囲を覆う窓を選ぶ
    // （複数語クエリで両語入りの窓が優先される）。merged はソート済み・非重複なので
    // start / end とも単調増加し、窓内の範囲 [lo, hi) は two-pointer で O(R) に数えられる
    // （コードブロック索引時は短い token が数千の一致を生むため O(R²) は不可）
    let mut best_start = 0usize;
    let mut best_covered = 0usize;
    let mut lo = 0usize; // rs >= start を満たす最初の添字
    let mut hi = 0usize; // re <= end を満たす範囲の個数（= 最初に re > end となる添字）
    for &(s, _) in &merged {
        let start = s.saturating_sub(max_chars / 4);
        let start = start.min(chars.len().saturating_sub(max_chars));
        let end = (start + max_chars).min(chars.len());
        while lo < merged.len() && merged[lo].0 < start {
            lo += 1;
        }
        while hi < merged.len() && merged[hi].1 <= end {
            hi += 1;
        }
        let covered = hi.saturating_sub(lo);
        if covered > best_covered {
            best_covered = covered;
            best_start = start;
        }
    }
    let win_start = best_start;
    let win_end = (win_start + max_chars).min(chars.len());

    // 窓内の一致範囲で交互にセグメント化
    let mut segments: Vec<ExcerptSegment> = Vec::new();
    let push = |segments: &mut Vec<ExcerptSegment>, s: usize, e: usize, mark: bool| {
        if s < e {
            segments.push(ExcerptSegment {
                text: chars[s..e].iter().collect(),
                mark,
            });
        }
    };
    let mut cursor = win_start;
    for &(s, e) in merged
        .iter()
        .filter(|&&(rs, re)| re > win_start && rs < win_end)
    {
        let s = s.max(win_start);
        let e = e.min(win_end);
        push(&mut segments, cursor, s, false);
        push(&mut segments, s, e, true);
        cursor = e;
    }
    push(&mut segments, cursor, win_end, false);

    // 両端の省略記号
    if win_start > 0 {
        match segments.first_mut() {
            Some(first) if !first.mark => first.text.insert(0, '…'),
            _ => segments.insert(
                0,
                ExcerptSegment {
                    text: "…".to_string(),
                    mark: false,
                },
            ),
        }
    }
    if win_end < chars.len() {
        match segments.last_mut() {
            Some(last) if !last.mark => last.text.push('…'),
            _ => segments.push(ExcerptSegment {
                text: "…".to_string(),
                mark: false,
            }),
        }
    }
    segments
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tokens(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    fn joined(segments: &[ExcerptSegment]) -> String {
        segments.iter().map(|s| s.text.as_str()).collect()
    }

    fn marked(segments: &[ExcerptSegment]) -> Vec<&str> {
        segments
            .iter()
            .filter(|s| s.mark)
            .map(|s| s.text.as_str())
            .collect()
    }

    #[test]
    fn 一致箇所周辺の窓と省略記号() {
        let text = format!("{}検索エンジンの説明{}", "あ".repeat(200), "い".repeat(200));
        let segments = make_excerpt(&text, &tokens(&["検索"]), 40);
        let out = joined(&segments);
        assert!(out.starts_with('…') && out.ends_with('…'), "out: {out}");
        assert_eq!(marked(&segments), ["検索"]);
        // 非 … 部分は原文の連続部分文字列
        let core = out.trim_matches('…');
        assert!(text.contains(core), "core が原文にない: {core}");
        // 窓は max_chars 以内（… を除く）
        assert!(core.chars().count() <= 40);
    }

    #[test]
    fn 大文字一致は原文の表記を保持する() {
        let segments = make_excerpt("Yuzu Build Watch", &tokens(&["build"]), 160);
        assert_eq!(marked(&segments), ["Build"]);
        assert_eq!(joined(&segments), "Yuzu Build Watch");
    }

    #[test]
    fn 全角英数も一致する() {
        let segments = make_excerpt("ＡＰＩサーバーの構築", &tokens(&["api"]), 160);
        assert_eq!(marked(&segments), ["ＡＰＩ"]);
    }

    #[test]
    fn 一致なしは先頭フォールバック() {
        let text = "长长长".repeat(100);
        let segments = make_excerpt(&text, &tokens(&["みつからない"]), 20);
        assert_eq!(segments.len(), 1);
        assert!(!segments[0].mark);
        assert_eq!(segments[0].text.chars().count(), 21, "20 文字 + …");
        assert!(segments[0].text.ends_with('…'));
    }

    #[test]
    fn 重なる一致範囲はマージされる() {
        // "検索エンジン" に token "検索" と "検索エンジン" が両方一致 → 1 つの mark に
        let segments = make_excerpt(
            "全文検索エンジンの話",
            &tokens(&["検索", "検索エンジン"]),
            160,
        );
        assert_eq!(marked(&segments), ["検索エンジン"]);
    }

    #[test]
    fn 複数語では両方を含む窓が選ばれる() {
        let text = format!(
            "検索だけの話。{}ここで検索とエンジンが並ぶ。{}",
            "あ".repeat(100),
            "い".repeat(100)
        );
        let segments = make_excerpt(&text, &tokens(&["検索", "エンジン"]), 30);
        let marks = marked(&segments);
        assert!(
            marks.contains(&"検索") && marks.contains(&"エンジン"),
            "両語を含む窓が選ばれる: {marks:?}"
        );
    }

    #[test]
    fn 空テキストと空トークンでも壊れない() {
        assert!(make_excerpt("", &tokens(&["a"]), 10).is_empty());
        let segments = make_excerpt("本文", &[], 10);
        assert_eq!(joined(&segments), "本文");
    }
}
