//! 依存なしの日付演算（gantt 用）。
//!
//! Howard Hinnant の civil calendar アルゴリズムによる日数変換。
//! **時計は読まない**（tankan は時刻・乱数非依存 = 決定的出力が原則）。

/// (年, 月, 日) → 1970-01-01 を 0 とする通算日
pub(crate) fn days_from_civil(y: i32, m: u32, d: u32) -> i64 {
    let y = i64::from(y) - i64::from(m <= 2);
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = (y - era * 400) as u64; // [0, 399]
    let mp = u64::from((m + 9) % 12); // 3 月始まりの月番号 [0, 11]
    let doy = (153 * mp + 2) / 5 + u64::from(d) - 1; // [0, 365]
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy; // [0, 146096]
    era * 146097 + doe as i64 - 719468
}

/// 通算日 → (年, 月, 日)
pub(crate) fn civil_from_days(z: i64) -> (i32, u32, u32) {
    let z = z + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u64; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365; // [0, 399]
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32; // [1, 12]
    ((y + i64::from(m <= 2)) as i32, m, d)
}

/// 曜日（0 = 日曜, …, 6 = 土曜）
pub(crate) fn weekday(z: i64) -> u32 {
    (z + 4).rem_euclid(7) as u32
}

/// 厳密な `YYYY-MM-DD` のみを受理する
pub(crate) fn parse_ymd(s: &str) -> Option<i64> {
    let mut it = s.split('-');
    let (y, m, d) = (it.next()?, it.next()?, it.next()?);
    if it.next().is_some() || y.len() != 4 || m.len() != 2 || d.len() != 2 {
        return None;
    }
    let (y, m, d) = (
        y.parse::<i32>().ok()?,
        m.parse::<u32>().ok()?,
        d.parse::<u32>().ok()?,
    );
    if !(1..=12).contains(&m) || d < 1 || d > days_in_month(y, m) {
        return None;
    }
    Some(days_from_civil(y, m, d))
}

fn days_in_month(y: i32, m: u32) -> u32 {
    match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if y % 4 == 0 && (y % 100 != 0 || y % 400 == 0) => 29,
        2 => 28,
        _ => 0,
    }
}

const MONTH_ABBREV: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];
const WEEKDAY_ABBREV: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];

/// 軸ラベルの整形。対応トークン: `%Y %y %m %d %e %b %a`（それ以外は素通し）
pub(crate) fn format_axis(z: i64, pattern: &str) -> String {
    let (y, m, d) = civil_from_days(z);
    let mut out = String::new();
    let mut chars = pattern.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '%' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('Y') => out.push_str(&format!("{y:04}")),
            Some('y') => out.push_str(&format!("{:02}", y.rem_euclid(100))),
            Some('m') => out.push_str(&format!("{m:02}")),
            Some('d') => out.push_str(&format!("{d:02}")),
            Some('e') => out.push_str(&d.to_string()),
            Some('b') => out.push_str(MONTH_ABBREV[(m - 1) as usize]),
            Some('a') => out.push_str(WEEKDAY_ABBREV[weekday(z) as usize]),
            Some(other) => {
                out.push('%');
                out.push(other);
            }
            None => out.push('%'),
        }
    }
    out
}

/// `format_axis` が対応しているパターンか（未対応はフォールバック判定に使う）
pub(crate) fn is_supported_axis_format(pattern: &str) -> bool {
    let mut chars = pattern.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '%' {
            match chars.next() {
                Some('Y' | 'y' | 'm' | 'd' | 'e' | 'b' | 'a') => {}
                _ => return false,
            }
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 既知の日付の往復と通算日() {
        // 1970-01-01 = 0（木曜）
        assert_eq!(days_from_civil(1970, 1, 1), 0);
        assert_eq!(weekday(0), 4);
        // 2000-03-01（うるう年の 2/29 の翌日）
        let z = days_from_civil(2000, 2, 29);
        assert_eq!(civil_from_days(z + 1), (2000, 3, 1));
        // 2100 年はうるう年でない
        let z = days_from_civil(2100, 2, 28);
        assert_eq!(civil_from_days(z + 1), (2100, 3, 1));
        // 往復同値（広い範囲）
        for z in [-100_000i64, -1, 0, 1, 10_000, 20_000, 100_000] {
            assert_eq!(days_from_civil_tuple(civil_from_days(z)), z);
        }
    }

    fn days_from_civil_tuple((y, m, d): (i32, u32, u32)) -> i64 {
        days_from_civil(y, m, d)
    }

    #[test]
    fn parse_ymd_の検証() {
        assert_eq!(parse_ymd("2026-07-05"), Some(days_from_civil(2026, 7, 5)));
        assert_eq!(parse_ymd("2024-02-29"), Some(days_from_civil(2024, 2, 29)));
        assert!(parse_ymd("2023-02-29").is_none(), "非うるう年");
        assert!(parse_ymd("2024-13-01").is_none());
        assert!(parse_ymd("2024-00-01").is_none());
        assert!(parse_ymd("2024-1-1").is_none(), "ゼロ埋め必須");
        assert!(parse_ymd("24-01-01").is_none());
        assert!(parse_ymd("2024/01/01").is_none());
    }

    #[test]
    fn 軸ラベルの整形() {
        let z = days_from_civil(2026, 7, 5); // 日曜
        assert_eq!(format_axis(z, "%Y-%m-%d"), "2026-07-05");
        assert_eq!(format_axis(z, "%m/%e (%a)"), "07/5 (Sun)");
        assert_eq!(format_axis(z, "%b %y"), "Jul 26");
        assert!(is_supported_axis_format("%Y-%m-%d"));
        assert!(!is_supported_axis_format("%H:%M"), "時刻は未対応");
    }
}
