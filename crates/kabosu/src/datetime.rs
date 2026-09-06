//! TOML の日付・時刻（RFC 3339 の 4 種）。
//!
//! 依存ゼロを維持するため独自型を持つ。**時刻演算・タイムゾーン変換・他の日時
//! crate への変換は持たない**（利用側がアクセサーの数値から組み立てる）。
//! 参照実装 `toml::value::Datetime` と同じく、date / time / offset の組み合わせ
//! 1 型で offset date-time / local date-time / local date / local time を表す。
//!
//! フィールドは非公開で、構築はコンストラクタだけを通る。暦として存在しない
//! 日付や範囲外の時刻は作れないため、`Encode` が不正な TOML を出すことはない。

use core::fmt;

/// TOML の日付・時刻。date と time の有無で 4 種を表す
/// （どちらか一方は必ず存在する）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Datetime {
    date: Option<Date>,
    time: Option<Time>,
    offset: Option<Offset>,
}

/// [`Datetime`] が表している 4 種のどれか
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DatetimeKind {
    /// `1979-05-27T07:32:00Z`
    OffsetDatetime,
    /// `1979-05-27T07:32:00`
    LocalDatetime,
    /// `1979-05-27`
    LocalDate,
    /// `07:32:00`
    LocalTime,
}

impl DatetimeKind {
    /// 英語の種別名（診断文言用）
    pub fn as_str(self) -> &'static str {
        match self {
            Self::OffsetDatetime => "offset date-time",
            Self::LocalDatetime => "local date-time",
            Self::LocalDate => "local date",
            Self::LocalTime => "local time",
        }
    }
}

impl Datetime {
    /// offset date-time（`1979-05-27T07:32:00Z`）
    pub fn offset_datetime(date: Date, time: Time, offset: Offset) -> Self {
        Self {
            date: Some(date),
            time: Some(time),
            offset: Some(offset),
        }
    }

    /// local date-time（`1979-05-27T07:32:00`）
    pub fn local_datetime(date: Date, time: Time) -> Self {
        Self {
            date: Some(date),
            time: Some(time),
            offset: None,
        }
    }

    /// local date（`1979-05-27`）
    pub fn local_date(date: Date) -> Self {
        Self {
            date: Some(date),
            time: None,
            offset: None,
        }
    }

    /// local time（`07:32:00`）
    pub fn local_time(time: Time) -> Self {
        Self {
            date: None,
            time: Some(time),
            offset: None,
        }
    }

    pub fn date(&self) -> Option<Date> {
        self.date
    }

    pub fn time(&self) -> Option<Time> {
        self.time
    }

    /// offset date-time のときだけ `Some`
    pub fn offset(&self) -> Option<Offset> {
        self.offset
    }

    pub fn kind(&self) -> DatetimeKind {
        match (self.date, self.time, self.offset) {
            (Some(_), Some(_), Some(_)) => DatetimeKind::OffsetDatetime,
            (Some(_), Some(_), None) => DatetimeKind::LocalDatetime,
            (Some(_), None, _) => DatetimeKind::LocalDate,
            (None, _, _) => DatetimeKind::LocalTime,
        }
    }
}

/// 正規形（RFC 3339。区切りは大文字 `T`、オフセット 0 は `Z`）
impl fmt::Display for Datetime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(date) = self.date {
            write!(f, "{date}")?;
            if self.time.is_some() {
                f.write_str("T")?;
            }
        }
        if let Some(time) = self.time {
            write!(f, "{time}")?;
        }
        if let Some(offset) = self.offset {
            write!(f, "{offset}")?;
        }
        Ok(())
    }
}

/// 暦日（proleptic Gregorian）
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Date {
    year: u16,
    month: u8,
    day: u8,
}

impl Date {
    /// 暦として存在する日付だけを返す（`1979-02-29` は `None`）。
    /// 年は 4 桁（0000〜9999）、月は 1〜12、日は月ごとの日数まで
    pub fn new(year: u16, month: u8, day: u8) -> Option<Self> {
        let in_range = year <= 9999
            && (1..=12).contains(&month)
            && (1..=days_in_month(year, month)).contains(&day);
        if !in_range {
            return None;
        }
        Some(Self { year, month, day })
    }

    pub fn year(&self) -> u16 {
        self.year
    }

    pub fn month(&self) -> u8 {
        self.month
    }

    pub fn day(&self) -> u8 {
        self.day
    }
}

impl fmt::Display for Date {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }
}

/// 月の日数。閏年は Gregorian 規則（4 の倍数。ただし 100 の倍数は 400 の倍数のときだけ）。
/// RFC 3339 は proleptic Gregorian なので 0000 年も 400 の倍数として閏年
fn days_in_month(year: u16, month: u8) -> u8 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
            if leap { 29 } else { 28 }
        }
        _ => 0,
    }
}

/// 時刻。秒は 60（うるう秒）まで、小数秒はナノ秒精度
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Time {
    hour: u8,
    minute: u8,
    second: u8,
    nanosecond: u32,
}

impl Time {
    /// 時 0〜23・分 0〜59・秒 0〜60（うるう秒）・ナノ秒 0〜999_999_999。
    /// 範囲外は `None`
    pub fn new(hour: u8, minute: u8, second: u8, nanosecond: u32) -> Option<Self> {
        if hour > 23 || minute > 59 || second > 60 || nanosecond > 999_999_999 {
            return None;
        }
        Some(Self {
            hour,
            minute,
            second,
            nanosecond,
        })
    }

    pub fn hour(&self) -> u8 {
        self.hour
    }

    pub fn minute(&self) -> u8 {
        self.minute
    }

    pub fn second(&self) -> u8 {
        self.second
    }

    pub fn nanosecond(&self) -> u32 {
        self.nanosecond
    }
}

/// `HH:MM:SS` ＋ 小数秒（0 なら書かない。末尾のゼロは落とす）
impl fmt::Display for Time {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:02}:{:02}:{:02}", self.hour, self.minute, self.second)?;
        if self.nanosecond == 0 {
            return Ok(());
        }
        // 末尾のゼロを落とす（`.500000000` → `.5`）。0 でないので必ず止まる
        let mut value = self.nanosecond;
        let mut width = 9usize;
        while value % 10 == 0 {
            value /= 10;
            width -= 1;
        }
        write!(f, ".{value:0width$}")
    }
}

/// UTC からのオフセット（分単位）。`Z` と `+00:00` は同じ値になり、
/// 正規化ではどちらも `Z` で出力する
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Offset {
    minutes: i16,
}

impl Offset {
    /// `Z`（= `+00:00`）
    pub const UTC: Self = Self { minutes: 0 };

    /// `-23:59`〜`+23:59`（分単位）。範囲外は `None`
    pub fn from_minutes(minutes: i16) -> Option<Self> {
        if minutes.unsigned_abs() > 23 * 60 + 59 {
            return None;
        }
        Some(Self { minutes })
    }

    pub fn minutes(&self) -> i16 {
        self.minutes
    }
}

impl fmt::Display for Offset {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.minutes == 0 {
            return f.write_str("Z");
        }
        let sign = if self.minutes < 0 { '-' } else { '+' };
        let abs = self.minutes.unsigned_abs();
        write!(f, "{sign}{:02}:{:02}", abs / 60, abs % 60)
    }
}

#[cfg(test)]
mod tests {
    use alloc::string::ToString;

    use super::*;

    fn date(y: u16, m: u8, d: u8) -> Date {
        Date::new(y, m, d).unwrap()
    }

    fn time(h: u8, mi: u8, s: u8, n: u32) -> Time {
        Time::new(h, mi, s, n).unwrap()
    }

    #[test]
    fn 暦として存在しない日付は構築できない() {
        assert!(Date::new(1979, 2, 29).is_none());
        assert!(Date::new(1900, 2, 29).is_none());
        assert!(Date::new(2000, 2, 29).is_some()); // 400 の倍数は閏年
        assert!(Date::new(2024, 2, 29).is_some());
        assert!(Date::new(0, 2, 29).is_some()); // 0000 年も 400 の倍数
        assert!(Date::new(1979, 13, 1).is_none());
        assert!(Date::new(1979, 4, 31).is_none());
        assert!(Date::new(1979, 1, 0).is_none());
        assert!(Date::new(10000, 1, 1).is_none());
    }

    #[test]
    fn 範囲外の時刻とオフセットは構築できない() {
        assert!(Time::new(23, 59, 60, 999_999_999).is_some()); // うるう秒
        assert!(Time::new(24, 0, 0, 0).is_none());
        assert!(Time::new(0, 60, 0, 0).is_none());
        assert!(Time::new(0, 0, 61, 0).is_none());
        assert!(Time::new(0, 0, 0, 1_000_000_000).is_none());
        assert!(Offset::from_minutes(23 * 60 + 59).is_some());
        assert!(Offset::from_minutes(-(23 * 60 + 59)).is_some());
        assert!(Offset::from_minutes(24 * 60).is_none());
    }

    #[test]
    fn 正規形の表示() {
        assert_eq!(
            Datetime::offset_datetime(date(1979, 5, 27), time(7, 32, 0, 0), Offset::UTC)
                .to_string(),
            "1979-05-27T07:32:00Z"
        );
        assert_eq!(
            Datetime::offset_datetime(
                date(1979, 5, 27),
                time(0, 32, 0, 999_999_000),
                Offset::from_minutes(-7 * 60).unwrap()
            )
            .to_string(),
            "1979-05-27T00:32:00.999999-07:00"
        );
        assert_eq!(
            Datetime::local_datetime(date(1979, 5, 27), time(7, 32, 0, 500_000_000)).to_string(),
            "1979-05-27T07:32:00.5"
        );
        assert_eq!(
            Datetime::local_date(date(1979, 5, 27)).to_string(),
            "1979-05-27"
        );
        assert_eq!(
            Datetime::local_time(time(7, 32, 0, 1)).to_string(),
            "07:32:00.000000001"
        );
        // オフセット 0 は書き方に依らず `Z`
        assert_eq!(
            Datetime::offset_datetime(
                date(2024, 1, 1),
                time(0, 0, 0, 0),
                Offset::from_minutes(0).unwrap()
            )
            .to_string(),
            "2024-01-01T00:00:00Z"
        );
        assert_eq!(
            Datetime::offset_datetime(
                date(2024, 1, 1),
                time(0, 0, 0, 0),
                Offset::from_minutes(9 * 60 + 30).unwrap()
            )
            .to_string(),
            "2024-01-01T00:00:00+09:30"
        );
    }

    #[test]
    fn 種別の判定() {
        assert_eq!(
            Datetime::offset_datetime(date(2024, 1, 1), time(0, 0, 0, 0), Offset::UTC).kind(),
            DatetimeKind::OffsetDatetime
        );
        assert_eq!(
            Datetime::local_datetime(date(2024, 1, 1), time(0, 0, 0, 0)).kind(),
            DatetimeKind::LocalDatetime
        );
        assert_eq!(
            Datetime::local_date(date(2024, 1, 1)).kind(),
            DatetimeKind::LocalDate
        );
        assert_eq!(
            Datetime::local_time(time(0, 0, 0, 0)).kind(),
            DatetimeKind::LocalTime
        );
    }
}
