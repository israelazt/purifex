//! A calendar day (the date of a driver, or today), without time zones: the
//! driver's date is the one its package declares.

use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Ticks of 100 ns between 1601-01-01 (Windows) and 1970-01-01 (Unix).
const UNIX_EPOCH_AS_TICKS: i64 = 116_444_736_000_000_000;
const TICKS_PER_DAY: i64 = 864_000_000_000;

/// Year, month and day. Ordered from oldest to newest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Date {
    pub year: u16,
    pub month: u8,
    pub day: u8,
}

impl Date {
    /// A real day of the calendar, or `None`.
    pub fn new(year: u16, month: u8, day: u8) -> Option<Self> {
        let valid = (1..=12).contains(&month) && day >= 1 && day <= days_in_month(year, month);
        valid.then_some(Self { year, month, day })
    }

    /// From a Windows `FILETIME` (100 ns ticks since 1601). Zero means "no
    /// date" and gives `None`, as does a value outside the years 1601–9999.
    pub fn from_filetime(ticks: u64) -> Option<Self> {
        if ticks == 0 {
            return None;
        }
        let days = (i64::try_from(ticks).ok()? - UNIX_EPOCH_AS_TICKS).div_euclid(TICKS_PER_DAY);
        Self::from_days(days)
    }

    /// Days since 1970-01-01 (negative before it).
    pub fn days(self) -> i64 {
        // Howard Hinnant's `days_from_civil`.
        let (y, m, d) = (
            i64::from(self.year),
            i64::from(self.month),
            i64::from(self.day),
        );
        let y = if m <= 2 { y - 1 } else { y };
        let era = y.div_euclid(400);
        let yoe = y - era * 400;
        let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
        let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
        era * 146_097 + doe - 719_468
    }

    /// The day `days` after 1970-01-01.
    pub fn from_days(days: i64) -> Option<Self> {
        // Howard Hinnant's `civil_from_days`.
        let z = days + 719_468;
        let era = z.div_euclid(146_097);
        let doe = z - era * 146_097;
        let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let day = doy - (153 * mp + 2) / 5 + 1;
        let month = if mp < 10 { mp + 3 } else { mp - 9 };
        let year = yoe + era * 400 + i64::from(month <= 2);
        Self::new(
            u16::try_from(year).ok()?,
            u8::try_from(month).ok()?,
            u8::try_from(day).ok()?,
        )
    }

    /// `2026-11-18`: year, month and day, each a number, and a real day.
    pub fn parse(text: &str) -> Option<Date> {
        let mut parts = text.split('-');
        let date = Date::new(
            parts.next()?.parse().ok()?,
            parts.next()?.parse().ok()?,
            parts.next()?.parse().ok()?,
        )?;
        parts.next().is_none().then_some(date)
    }

    /// `20261118`, as `InstallDate` is written in the registry: eight digits and a real day.
    pub fn parse_compact(text: &str) -> Option<Date> {
        let text = text.trim();
        if text.len() != 8 || !text.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        Date::new(
            text[..4].parse().ok()?,
            text[4..6].parse().ok()?,
            text[6..].parse().ok()?,
        )
    }

    /// Whole days from `self` to `later` (negative if `later` is earlier).
    pub fn days_until(self, later: Date) -> i64 {
        later.days() - self.days()
    }
}

fn days_in_month(year: u16, month: u8) -> u8 {
    match month {
        2 if year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400)) => {
            29
        }
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

impl fmt::Display for Date {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }
}

impl Serialize for Date {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for Date {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        Date::parse(&text).ok_or_else(|| serde::de::Error::custom(format!("not a date: {text}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn date(y: u16, m: u8, d: u8) -> Date {
        Date::new(y, m, d).unwrap()
    }

    #[test]
    fn the_compact_form_of_the_registry_is_read_and_a_bad_one_is_not() {
        assert_eq!(Date::parse_compact("20261118"), Date::new(2026, 11, 18));
        assert_eq!(Date::parse_compact(" 20240229 "), Date::new(2024, 2, 29));
        for bad in [
            "",
            "2026118",
            "20261318",
            "20230229",
            "2026-11-1",
            "abcdefgh",
            "202611180",
        ] {
            assert_eq!(Date::parse_compact(bad), None, "{bad}");
        }
    }

    #[test]
    fn only_real_days_are_dates() {
        assert!(Date::new(2024, 2, 29).is_some(), "leap year");
        assert!(Date::new(2023, 2, 29).is_none());
        assert!(Date::new(1900, 2, 29).is_none(), "century, not leap");
        assert!(Date::new(2000, 2, 29).is_some(), "every 400 years");
        assert!(Date::new(2026, 13, 1).is_none());
        assert!(Date::new(2026, 4, 31).is_none());
        assert!(Date::new(2026, 1, 0).is_none());
    }

    #[test]
    fn a_filetime_becomes_its_day() {
        // 2026-11-18 00:00:00 UTC = 20775 days after 1970.
        let ticks = (20_775 * TICKS_PER_DAY + UNIX_EPOCH_AS_TICKS) as u64;
        assert_eq!(Date::from_filetime(ticks), Some(date(2026, 11, 18)));
        // The middle of that day is still that day.
        assert_eq!(
            Date::from_filetime(ticks + TICKS_PER_DAY as u64 / 2),
            Some(date(2026, 11, 18))
        );
        assert_eq!(Date::from_filetime(0), None, "zero means no date");
        assert_eq!(Date::from_filetime(u64::MAX), None);
    }

    #[test]
    fn days_and_dates_go_both_ways() {
        assert_eq!(date(1970, 1, 1).days(), 0);
        assert_eq!(date(2026, 11, 18).days(), 20_775);
        for days in [-100_000, -1, 0, 1, 59, 60, 11_016, 20_775, 1_000_000] {
            assert_eq!(Date::from_days(days).unwrap().days(), days);
        }
    }

    #[test]
    fn it_measures_the_distance_between_days() {
        assert_eq!(date(2026, 1, 1).days_until(date(2027, 1, 1)), 365);
        assert_eq!(date(2024, 1, 1).days_until(date(2025, 1, 1)), 366);
        assert_eq!(date(2026, 5, 1).days_until(date(2026, 4, 1)), -30);
    }

    #[test]
    fn it_orders_and_prints_as_iso() {
        assert!(date(2025, 12, 31) < date(2026, 1, 1));
        assert_eq!(date(2026, 3, 5).to_string(), "2026-03-05");
    }
}
