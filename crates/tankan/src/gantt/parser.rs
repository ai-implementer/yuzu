//! gantt の行指向パーサ。
//!
//! タスク行 `名前 : [タグ,]* [id,] [開始,] [長さ|終了日]` は
//! カンマ分割後、前からタグを剥がし、**残りを後ろから**
//! 「長さ/終了 → 開始 → id」と解釈する（mermaid の挙動に合わせる）。
//! excludes は解析時に展開し、期間計算は「働き日消化」で行う。

use std::collections::HashMap;

use crate::common::date::{is_supported_axis_format, parse_ymd, weekday};
use crate::error::Error;
use crate::gantt::model::{GanttDiagram, Section, Task, TaskTags, TickInterval};
use crate::kind::trim_line;

/// excludes の適用範囲を制限する安全弁（無限ループ防止）
const MAX_SPAN_DAYS: i64 = 3660;

pub(crate) fn parse(source: &str) -> Result<GanttDiagram, Error> {
    let mut diagram = GanttDiagram::default();
    let mut sections: Vec<Section> = Vec::new();
    let mut task_ids: HashMap<String, (usize, usize)> = HashMap::new(); // id → (section, task)
    let mut date_format_ok = false;
    let mut excluded_weekdays: Vec<u32> = Vec::new(); // 0=日 .. 6=土
    let mut excluded_dates: Vec<i64> = Vec::new();
    let mut weekend_days: [u32; 2] = [6, 0]; // 既定: 土日
    let mut excludes_weekends = false;

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
                diagram.title = Some(trim_line(t).to_string());
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
            if line == "gantt" {
                seen_header = true;
                continue;
            }
            return Err(Error::Parse {
                line: line_no,
                message: "gantt ヘッダがありません".to_string(),
            });
        }

        let keyword = line.split_whitespace().next().unwrap_or("");
        let rest = trim_line(&line[keyword.len().min(line.len())..]);
        match keyword {
            "dateFormat" => {
                if rest != "YYYY-MM-DD" {
                    return Err(Error::UnsupportedSyntax {
                        line: line_no,
                        construct: format!("dateFormat {rest}（YYYY-MM-DD のみ対応）"),
                    });
                }
                date_format_ok = true;
            }
            "title" => diagram.title = Some(rest.to_string()),
            "axisFormat" => {
                if !is_supported_axis_format(rest) {
                    return Err(Error::UnsupportedSyntax {
                        line: line_no,
                        construct: format!("axisFormat {rest}"),
                    });
                }
                diagram.axis_format = Some(rest.to_string());
            }
            "tickInterval" => {
                diagram.tick = Some(parse_tick(rest, line_no)?);
            }
            "excludes" => {
                for item in rest
                    .split([',', ' '])
                    .map(trim_line)
                    .filter(|s| !s.is_empty())
                {
                    if item.eq_ignore_ascii_case("weekends") {
                        excludes_weekends = true;
                    } else if let Some(wd) = weekday_of(item) {
                        excluded_weekdays.push(wd);
                    } else if let Some(z) = parse_ymd(item) {
                        excluded_dates.push(z);
                    } else {
                        return Err(Error::UnsupportedSyntax {
                            line: line_no,
                            construct: format!("excludes {item}"),
                        });
                    }
                }
            }
            "weekend" => {
                match rest.to_ascii_lowercase().as_str() {
                    "friday" => weekend_days = [5, 6],
                    "saturday" => weekend_days = [6, 0],
                    other => {
                        return Err(Error::Parse {
                            line: line_no,
                            message: format!("不明な weekend 指定: {other}"),
                        });
                    }
                };
            }
            "todayMarker" => {
                // tankan は時刻を読まないため today 線は描かない。off のみ受理
                if rest != "off" {
                    return Err(Error::UnsupportedSyntax {
                        line: line_no,
                        construct: "todayMarker（off 以外）".to_string(),
                    });
                }
            }
            "section" => sections.push(Section {
                name: rest.to_string(),
                tasks: Vec::new(),
            }),
            "displayMode" | "includes" | "inclusiveEndDates" | "topAxis" => {
                return Err(Error::UnsupportedSyntax {
                    line: line_no,
                    construct: keyword.to_string(),
                });
            }
            _ if line.contains(':') => {
                if line.contains("click ") || line.contains("href ") {
                    return Err(Error::UnsupportedSyntax {
                        line: line_no,
                        construct: "click/href".to_string(),
                    });
                }
                if !date_format_ok {
                    return Err(Error::Parse {
                        line: line_no,
                        message: "タスクの前に `dateFormat YYYY-MM-DD` が必要です".to_string(),
                    });
                }
                if sections.is_empty() {
                    sections.push(Section {
                        name: String::new(),
                        tasks: Vec::new(),
                    });
                }
                let is_excluded = |z: i64| -> bool {
                    (excludes_weekends && weekend_days.contains(&weekday(z)))
                        || excluded_weekdays.contains(&weekday(z))
                        || excluded_dates.contains(&z)
                };
                let section_idx = sections.len() - 1;
                // 開始省略はセクションを跨いでも直前タスクの終了に続く（mermaid 互換）
                let prev_end = sections
                    .iter()
                    .flat_map(|s| s.tasks.iter())
                    .next_back()
                    .map(|t: &Task| t.end);
                let (task, id) =
                    parse_task(line, line_no, prev_end, &task_ids, &sections, &is_excluded)?;
                sections[section_idx].tasks.push(task);
                if let Some(id) = id {
                    task_ids.insert(id, (section_idx, sections[section_idx].tasks.len() - 1));
                }
            }
            _ => {
                return Err(Error::Parse {
                    line: line_no,
                    message: "文として解釈できません".to_string(),
                });
            }
        }
    }

    if sections.iter().all(|s| s.tasks.is_empty()) {
        return Err(Error::Parse {
            line: source.lines().count(),
            message: "タスクがありません".to_string(),
        });
    }

    // 除外日の実体化（描画範囲内のみ）
    let min = sections
        .iter()
        .flat_map(|s| s.tasks.iter().map(|t| t.start))
        .min()
        .unwrap_or(0);
    let max = sections
        .iter()
        .flat_map(|s| s.tasks.iter().map(|t| t.end))
        .max()
        .unwrap_or(0);
    for z in min..max.min(min + MAX_SPAN_DAYS) {
        if (excludes_weekends && weekend_days.contains(&weekday(z)))
            || excluded_weekdays.contains(&weekday(z))
            || excluded_dates.contains(&z)
        {
            diagram.excluded_days.push(z);
        }
    }

    diagram.sections = sections;
    Ok(diagram)
}

/// タスク行を解決する。戻り値: (タスク, id)
fn parse_task(
    line: &str,
    line_no: usize,
    prev_end: Option<i64>,
    task_ids: &HashMap<String, (usize, usize)>,
    sections: &[Section],
    is_excluded: &dyn Fn(i64) -> bool,
) -> Result<(Task, Option<String>), Error> {
    let (name, meta) = line.split_once(':').expect("contains(':') 確認済み");
    let name = trim_line(name).to_string();
    let mut segs: Vec<&str> = meta.split(',').map(trim_line).collect();
    segs.retain(|s| !s.is_empty());

    // 前からタグ
    let mut tags = TaskTags::default();
    while let Some(&seg) = segs.first() {
        match seg {
            "done" => tags.done = true,
            "active" => tags.active = true,
            "crit" => tags.crit = true,
            "milestone" => tags.milestone = true,
            _ => break,
        }
        segs.remove(0);
    }

    // 後ろから: 長さ/終了 → 開始 → id
    let parse_err = |message: String| Error::Parse {
        line: line_no,
        message,
    };
    let resolve_start = |seg: &str| -> Result<Option<i64>, Error> {
        if let Some(rest) = seg.strip_prefix("after ") {
            let mut latest: Option<i64> = None;
            for id in rest.split_whitespace() {
                let Some(&(si, ti)) = task_ids.get(id) else {
                    return Err(parse_err(format!("after の参照先が見つかりません: {id}")));
                };
                let end = sections[si].tasks[ti].end;
                latest = Some(latest.map_or(end, |l: i64| l.max(end)));
            }
            return Ok(latest);
        }
        Ok(parse_ymd(seg))
    };

    let (id, start, len_seg) = match segs.len() {
        1 => (None, None, segs[0]),
        2 => {
            let maybe_start = resolve_start(segs[0])?;
            if maybe_start.is_some() {
                (None, maybe_start, segs[1])
            } else {
                (Some(segs[0].to_string()), None, segs[1])
            }
        }
        3 => {
            let start = resolve_start(segs[1])?;
            if start.is_none() {
                return Err(parse_err(format!("開始として解釈できません: {}", segs[1])));
            }
            (Some(segs[0].to_string()), start, segs[2])
        }
        n => {
            return Err(parse_err(format!(
                "タスクのメタ情報が解釈できません（{n} 項目）"
            )));
        }
    };

    let start = match start.or(prev_end) {
        Some(s) => s,
        None => {
            return Err(parse_err(
                "開始日がありません（最初のタスクには日付が必要です）".to_string(),
            ));
        }
    };

    // 長さ or 終了日
    let end = if let Some(z) = parse_ymd(len_seg) {
        // 終了日は排他（mermaid は end 日の 0:00 まで）
        if z <= start {
            return Err(parse_err(format!("終了日が開始日以前です: {len_seg}")));
        }
        z
    } else if let Some(days) = parse_duration_days(len_seg) {
        // 働き日を days 日ぶん消化するまで進める（excludes 対応）
        let mut end = start;
        let mut remaining = days;
        while remaining > 0 {
            if !is_excluded(end) {
                remaining -= 1;
            }
            end += 1;
            if end - start > MAX_SPAN_DAYS {
                return Err(parse_err("期間が長すぎます".to_string()));
            }
        }
        end
    } else if len_seg.ends_with('h') || len_seg.ends_with("min") || len_seg.ends_with('m') {
        return Err(Error::UnsupportedSyntax {
            line: line_no,
            construct: format!("時分単位の期間: {len_seg}"),
        });
    } else if len_seg.starts_with("until ") {
        return Err(Error::UnsupportedSyntax {
            line: line_no,
            construct: "until".to_string(),
        });
    } else {
        return Err(parse_err(format!("期間として解釈できません: {len_seg}")));
    };

    Ok((
        Task {
            name,
            start,
            end,
            tags,
        },
        id,
    ))
}

/// `Nd` / `Nw`（小数は切り上げ）
fn parse_duration_days(seg: &str) -> Option<i64> {
    let (num, unit) = seg.split_at(seg.len().checked_sub(1)?);
    let value: f64 = num.parse().ok()?;
    if value <= 0.0 {
        return None;
    }
    match unit {
        "d" => Some(value.ceil() as i64),
        "w" => Some((value * 7.0).ceil() as i64),
        _ => None,
    }
}

fn parse_tick(rest: &str, line_no: usize) -> Result<TickInterval, Error> {
    let split = rest
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(rest.len());
    let (num, unit) = rest.split_at(split);
    let n: u32 = num.parse().unwrap_or(1).max(1);
    match unit {
        "day" => Ok(TickInterval::Day(n)),
        "week" => Ok(TickInterval::Week(n)),
        "month" => Ok(TickInterval::Month(n)),
        other => Err(Error::UnsupportedSyntax {
            line: line_no,
            construct: format!("tickInterval {num}{other}"),
        }),
    }
}

fn weekday_of(name: &str) -> Option<u32> {
    match name.to_ascii_lowercase().as_str() {
        "sunday" => Some(0),
        "monday" => Some(1),
        "tuesday" => Some(2),
        "wednesday" => Some(3),
        "thursday" => Some(4),
        "friday" => Some(5),
        "saturday" => Some(6),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::parse;
    use crate::common::date::parse_ymd;

    fn gantt(body: &str) -> String {
        format!("gantt\ndateFormat YYYY-MM-DD\n{body}")
    }

    #[test]
    fn 基本のタスクと日付境界() {
        let d = parse(&gantt(
            "タスクA : 2024-01-01, 3d\nタスクB : 2024-01-10, 2024-01-15",
        ))
        .unwrap();
        let tasks = &d.sections[0].tasks;
        assert_eq!(tasks[0].start, parse_ymd("2024-01-01").unwrap());
        // 3d = 幅 3 日（排他 end）
        assert_eq!(tasks[0].end - tasks[0].start, 3);
        // 終了日指定も排他
        assert_eq!(tasks[1].end, parse_ymd("2024-01-15").unwrap());
    }

    #[test]
    fn 開始省略は直前タスクの終了() {
        let d = parse(&gantt("A : 2024-01-01, 2d\nB : 3d")).unwrap();
        let tasks = &d.sections[0].tasks;
        assert_eq!(tasks[1].start, tasks[0].end);
        // セクションを跨いでも直前タスクに続く
        let d = parse(&gantt("section S1\nA : 2024-01-01, 2d\nsection S2\nB : 3d")).unwrap();
        assert_eq!(d.sections[1].tasks[0].start, d.sections[0].tasks[0].end);
    }

    #[test]
    fn after_依存と_id() {
        let d = parse(&gantt(
            "A : a1, 2024-01-01, 2d\nB : b1, 2024-01-01, 5d\nC : after a1 b1, 1d",
        ))
        .unwrap();
        let tasks = &d.sections[0].tasks;
        assert_eq!(tasks[2].start, tasks[1].end, "最も遅い終了に合わせる");
        // 未知の id はエラー
        assert!(parse(&gantt("X : after nazo, 1d")).is_err());
    }

    #[test]
    fn タグとセクション() {
        let d = parse(&gantt(
            "section 設計\n完了タスク : done, 2024-01-01, 1d\nsection 実装\n重要 : crit, active, 2024-01-02, 2d\n節目 : milestone, 2024-01-05, 1d",
        ))
        .unwrap();
        assert_eq!(d.sections.len(), 2);
        assert!(d.sections[0].tasks[0].tags.done);
        let imp = &d.sections[1].tasks[0];
        assert!(imp.tags.crit && imp.tags.active);
        assert!(d.sections[1].tasks[1].tags.milestone);
    }

    #[test]
    fn excludes_weekends_で期間が伸びる() {
        // 2024-01-05 は金曜。3d で土日を跨ぐ
        let d = parse(&gantt("excludes weekends\nA : 2024-01-05, 3d")).unwrap();
        let t = &d.sections[0].tasks[0];
        // 金・(土日除外)・月・火 → end = 水曜 = 1/10
        assert_eq!(t.end, parse_ymd("2024-01-10").unwrap());
        assert!(!d.excluded_days.is_empty());
    }

    #[test]
    fn 週単位と小数切り上げ() {
        let d = parse(&gantt("A : 2024-01-01, 1w\nB : 2024-01-01, 1.5d")).unwrap();
        assert_eq!(d.sections[0].tasks[0].end - d.sections[0].tasks[0].start, 7);
        assert_eq!(d.sections[0].tasks[1].end - d.sections[0].tasks[1].start, 2);
    }

    #[test]
    fn 未対応構文はフォールバック() {
        for src in [
            "gantt\ndateFormat DD-MM-YYYY\nA : 01-01-2024, 1d",
            "gantt\ndateFormat YYYY-MM-DD\nA : 2024-01-01, 2h",
            "gantt\ndateFormat YYYY-MM-DD\ntickInterval 1hour\nA : 2024-01-01, 1d",
            "gantt\ndateFormat YYYY-MM-DD\naxisFormat %H:%M\nA : 2024-01-01, 1d",
            "gantt\ndateFormat YYYY-MM-DD\ntodayMarker stroke-width:5px\nA : 2024-01-01, 1d",
        ] {
            let err = parse(src).unwrap_err();
            assert!(err.is_unsupported(), "{src}: {err}");
        }
    }

    #[test]
    fn 構文エラーの検出() {
        assert!(
            parse("gantt\nA : 2024-01-01, 1d").is_err(),
            "dateFormat なし"
        );
        assert!(parse(&gantt("A : 1d")).is_err(), "最初のタスクに日付なし");
        assert!(parse(&gantt("A : 2024-01-01, 2024-01-01")).is_err(), "幅 0");
        assert!(
            parse("gantt\ndateFormat YYYY-MM-DD\ntitle x").is_err(),
            "タスクなし"
        );
    }
}
