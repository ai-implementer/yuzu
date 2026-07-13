//! pie チャートの行指向パーサ。
//!
//! 対応構文（mermaid 互換）:
//! - ヘッダ: `pie` / `pie showData` / `pie title 好きな果物` / `pie showData title ...`
//! - `title <テキスト>` 行
//! - データ行: `"ラベル" : 42.5`（値は 0 以上の数値）
//! - `%%` コメント・`%%{init}%%` ディレクティブ・YAML frontmatter（title を拾う）
//! - `accTitle:` / `accDescr:` は受理して無視する

use crate::error::Error;
use crate::kind::trim_line;
use crate::pie::model::{PieChart, Slice};

pub(crate) fn parse(source: &str) -> Result<PieChart, Error> {
    let mut chart = PieChart::default();

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
                chart.title = Some(t.trim().to_string());
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
            let Some(rest) = line.strip_prefix("pie") else {
                return Err(Error::Parse {
                    line: line_no,
                    message: "pie ヘッダがありません".to_string(),
                });
            };
            seen_header = true;
            parse_header_rest(rest.trim(), line_no, &mut chart)?;
            continue;
        }

        if let Some(t) = line.strip_prefix("title ") {
            chart.title = Some(t.trim().to_string());
            continue;
        }
        if line == "showData" {
            chart.show_data = true;
            continue;
        }
        if line.starts_with("accTitle") || line.starts_with("accDescr") {
            continue; // アクセシビリティ行は受理して無視（他図種と同じ）
        }

        chart.slices.push(parse_data_line(line, line_no)?);
    }

    if !seen_header {
        return Err(Error::Parse {
            line: 1,
            message: "pie ヘッダがありません".to_string(),
        });
    }
    if chart.slices.is_empty() {
        return Err(Error::Parse {
            line: 1,
            message: "データ行（\"ラベル\" : 値）がありません".to_string(),
        });
    }
    if chart.total() <= 0.0 {
        return Err(Error::Parse {
            line: 1,
            message: "値の合計が 0 です".to_string(),
        });
    }
    Ok(chart)
}

/// ヘッダ行の `pie` に続く部分（`showData` / `title ...`）を解釈する
fn parse_header_rest(rest: &str, line_no: usize, chart: &mut PieChart) -> Result<(), Error> {
    let mut rest = rest;
    if let Some(r) = rest.strip_prefix("showData") {
        chart.show_data = true;
        rest = r.trim();
    }
    if let Some(t) = rest.strip_prefix("title") {
        chart.title = Some(t.trim().to_string());
        rest = "";
    }
    if !rest.is_empty() {
        return Err(Error::Parse {
            line: line_no,
            message: format!("pie ヘッダに解釈できない指定があります: `{rest}`"),
        });
    }
    Ok(())
}

/// `"ラベル" : 42.5` をパースする
fn parse_data_line(line: &str, line_no: usize) -> Result<Slice, Error> {
    let parse_err = |message: String| Error::Parse {
        line: line_no,
        message,
    };

    let Some(rest) = line.strip_prefix('"') else {
        return Err(parse_err(format!(
            "データ行はラベルを引用符で囲みます（`\"ラベル\" : 値`）: `{line}`"
        )));
    };
    let Some(quote_end) = rest.find('"') else {
        return Err(parse_err("ラベルの閉じ引用符がありません".to_string()));
    };
    let label = rest[..quote_end].to_string();
    let after = rest[quote_end + 1..].trim_start();
    let Some(value_part) = after.strip_prefix(':') else {
        return Err(parse_err("ラベルの後に `:` が必要です".to_string()));
    };
    let raw_value = value_part.trim().to_string();
    let value: f32 = raw_value
        .parse()
        .map_err(|_| parse_err(format!("値が数値ではありません: `{raw_value}`")))?;
    if !value.is_finite() || value < 0.0 {
        return Err(parse_err(format!(
            "値は 0 以上の数値にしてください: `{raw_value}`"
        )));
    }
    Ok(Slice {
        label,
        value,
        raw_value,
    })
}

#[cfg(test)]
mod tests {
    use super::parse;

    #[test]
    fn 基本形をパースできる() {
        let chart =
            parse("pie\n  title 好きな果物\n  \"りんご\" : 42\n  \"みかん\" : 28\n").unwrap();
        assert_eq!(chart.title.as_deref(), Some("好きな果物"));
        assert!(!chart.show_data);
        assert_eq!(chart.slices.len(), 2);
        assert_eq!(chart.slices[0].label, "りんご");
        assert_eq!(chart.slices[0].value, 42.0);
    }

    #[test]
    fn ヘッダの_show_data_と_title() {
        let chart = parse("pie showData title 割合\n\"a\" : 1\n").unwrap();
        assert!(chart.show_data);
        assert_eq!(chart.title.as_deref(), Some("割合"));
    }

    #[test]
    fn frontmatter_の_title_を拾う() {
        let chart = parse("---\ntitle: FM タイトル\n---\npie\n\"a\" : 1\n").unwrap();
        assert_eq!(chart.title.as_deref(), Some("FM タイトル"));
    }

    #[test]
    fn 小数と_raw_文字列を保持する() {
        let chart = parse("pie\n\"Ca\" : 42.96\n").unwrap();
        assert_eq!(chart.slices[0].raw_value, "42.96");
    }

    #[test]
    fn 引用符なしラベルは構文エラー() {
        let err = parse("pie\nりんご : 42\n").unwrap_err();
        assert!(!err.is_unsupported(), "Parse エラーであること: {err}");
    }

    #[test]
    fn 負の値は構文エラー() {
        assert!(parse("pie\n\"a\" : -1\n").is_err());
    }

    #[test]
    fn データ行ゼロは構文エラー() {
        assert!(parse("pie\ntitle t\n").is_err());
        assert!(parse("pie\n\"a\" : 0\n").is_err(), "合計 0 もエラー");
    }

    #[test]
    fn コメントとディレクティブを読み飛ばす() {
        let chart = parse("%% メモ\n%%{init: {}}%%\npie\n%% 行コメント\n\"a\" : 1\n").unwrap();
        assert_eq!(chart.slices.len(), 1);
    }
}
