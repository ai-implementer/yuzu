//! timeline の行指向パーサ。
//!
//! 対応構文（mermaid 互換）:
//! - ヘッダ: `timeline`
//! - `title <テキスト>` 行
//! - `section <名前>` 行（以降の期間をそのセクションに属させる）
//! - 期間行: `期間 : イベント1 : イベント2`（コロン無しはイベント無し期間）
//! - 継続行: `: イベント`（直前の期間へイベントを追加する縦書きスタイル）
//! - `%%` コメント・`%%{init}%%` ディレクティブ・YAML frontmatter（title を拾う）
//! - `accTitle:` / `accDescr:` は受理して無視する

use crate::error::Error;
use crate::kind::trim_line;
use crate::timeline::model::{Period, Section, TimelineDiagram};

pub(crate) fn parse(source: &str) -> Result<TimelineDiagram, Error> {
    let mut diagram = TimelineDiagram::default();

    let mut in_directive = false;
    let mut in_frontmatter = false;
    let mut seen_header = false;
    let mut first_content = true;

    for (idx, raw) in source.lines().enumerate() {
        let line_no = idx + 1;
        let line = trim_line(raw);
        if line.is_empty() {
            continue;
        }
        if in_directive {
            if line.ends_with("}%%") {
                in_directive = false;
            }
            continue;
        }
        if in_frontmatter {
            if line == "---" {
                in_frontmatter = false;
            } else if let Some(t) = line.strip_prefix("title:") {
                diagram.title = Some(t.trim().to_string());
            }
            continue;
        }
        if first_content && line == "---" {
            in_frontmatter = true;
            first_content = false;
            continue;
        }
        first_content = false;
        if line.starts_with("%%{") {
            if !line.ends_with("}%%") {
                in_directive = true;
            }
            continue;
        }
        if line.starts_with("%%") {
            continue;
        }

        if !seen_header {
            if line != "timeline" {
                return Err(Error::Parse {
                    line: line_no,
                    message: "timeline ヘッダがありません".to_string(),
                });
            }
            seen_header = true;
            continue;
        }

        if let Some(t) = line.strip_prefix("title ") {
            diagram.title = Some(t.trim().to_string());
            continue;
        }
        if let Some(name) = line.strip_prefix("section ") {
            diagram.sections.push(Section {
                name: Some(name.trim().to_string()),
                periods: Vec::new(),
            });
            continue;
        }
        if line.starts_with("accTitle") || line.starts_with("accDescr") {
            continue; // アクセシビリティ行は受理して無視（他図種と同じ）
        }

        // 継続行 `: イベント` — 直前の期間へ追加（縦書きスタイル）
        if let Some(rest) = line.strip_prefix(':') {
            let Some(period) = diagram
                .sections
                .last_mut()
                .and_then(|s| s.periods.last_mut())
            else {
                return Err(Error::Parse {
                    line: line_no,
                    message: "イベント行（`: ...`）の前に期間がありません".to_string(),
                });
            };
            period.events.extend(split_events(rest));
            continue;
        }

        // 期間行 `期間 : e1 : e2`（コロン無しはイベント無し期間として受理）
        let (label, events) = match line.split_once(':') {
            Some((label, rest)) => (trim_line(label).to_string(), split_events(rest)),
            None => (line.to_string(), Vec::new()),
        };
        // section 未出現の期間は暗黙セクションへ（gantt と同じ手口）
        if diagram.sections.is_empty() {
            diagram.sections.push(Section {
                name: None,
                periods: Vec::new(),
            });
        }
        diagram
            .sections
            .last_mut()
            .expect("直前で必ず 1 つ作られる")
            .periods
            .push(Period { label, events });
    }

    if !seen_header {
        return Err(Error::Parse {
            line: 1,
            message: "timeline ヘッダがありません".to_string(),
        });
    }
    if diagram.sections.iter().all(|s| s.periods.is_empty()) {
        return Err(Error::Parse {
            line: 1,
            message: "期間行がありません".to_string(),
        });
    }
    Ok(diagram)
}

/// `e1 : e2 : e3` をイベント列に分割する（空要素は捨てる）
fn split_events(rest: &str) -> Vec<String> {
    rest.split(':')
        .map(trim_line)
        .filter(|e| !e.is_empty())
        .map(str::to_string)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::parse;

    #[test]
    fn 基本形をパースできる() {
        let d =
            parse("timeline\n  title SNS の歴史\n  2002 : LinkedIn\n  2004 : Facebook : Google\n")
                .unwrap();
        assert_eq!(d.title.as_deref(), Some("SNS の歴史"));
        assert_eq!(d.sections.len(), 1);
        assert_eq!(d.sections[0].name, None, "暗黙セクション");
        let p = &d.sections[0].periods;
        assert_eq!(p.len(), 2);
        assert_eq!(p[1].label, "2004");
        assert_eq!(p[1].events, ["Facebook", "Google"]);
    }

    #[test]
    fn セクションで期間が分かれる() {
        let d = parse(
            "timeline\nsection 17世紀\n  1665 : 微積分\nsection 18世紀\n  1712 : 蒸気機関\n  1783 : 気球\n",
        )
        .unwrap();
        assert_eq!(d.sections.len(), 2);
        assert_eq!(d.sections[0].name.as_deref(), Some("17世紀"));
        assert_eq!(d.sections[0].periods.len(), 1);
        assert_eq!(d.sections[1].periods.len(), 2);
    }

    #[test]
    fn 継続行が直前の期間に追加される() {
        let d = parse("timeline\n2023 : リリース\n     : 機能追加\n     : 改善\n").unwrap();
        let p = &d.sections[0].periods[0];
        assert_eq!(p.events, ["リリース", "機能追加", "改善"]);
    }

    #[test]
    fn イベント無し期間も受理する() {
        let d = parse("timeline\n2020\n2021 : 出来事\n").unwrap();
        let p = &d.sections[0].periods;
        assert_eq!(p[0].label, "2020");
        assert!(p[0].events.is_empty());
    }

    #[test]
    fn frontmatter_の_title_を拾う() {
        let d = parse("---\ntitle: FM タイトル\n---\ntimeline\n2020 : a\n").unwrap();
        assert_eq!(d.title.as_deref(), Some("FM タイトル"));
    }

    #[test]
    fn 期間より前の継続行はエラー() {
        assert!(parse("timeline\n: イベント\n").is_err());
        // セクション直後（期間なし）もエラー
        assert!(parse("timeline\nsection A\n: イベント\n").is_err());
    }

    #[test]
    fn ヘッダ無しと期間ゼロはエラー() {
        assert!(parse("2020 : a\n").is_err());
        assert!(parse("timeline\ntitle だけ\n").is_err());
    }
}
