//! The name of a recording (`lol-20261118-165329`): the game, the day and the
//! time it started. Names can come from outside the program (a command line, a
//! tool call), so what may be a name is strict: it is never a path.

use std::fmt;

use crate::date::Date;
use crate::ports::SessionInfo;

/// The longest name accepted.
const MAX_LEN: usize = 64;

/// Whether `id` is a safe name: letters, digits, `-` and `_` only, so it can
/// never point outside the folder of recordings (`..`, `\`, `/`, `:`).
pub fn is_valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= MAX_LEN
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

/// When a recording started, read from its name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct SessionStamp {
    pub date: Date,
    pub hour: u8,
    pub minute: u8,
    pub second: u8,
}

impl fmt::Display for SessionStamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} {:02}:{:02}:{:02}",
            self.date, self.hour, self.minute, self.second
        )
    }
}

/// The start of a recording from its name: the last two parts, `YYYYMMDD` and
/// `HHMMSS`. `None` for a name that does not follow that shape.
pub fn stamp_of(id: &str) -> Option<SessionStamp> {
    let mut parts = id.rsplit('-');
    let time = parts.next()?;
    let day = parts.next()?;
    let digits = |s: &str, n: usize| s.len() == n && s.bytes().all(|b| b.is_ascii_digit());
    if !digits(time, 6) || !digits(day, 8) {
        return None;
    }
    let num = |s: &str| s.parse::<u16>().ok();
    let stamp = SessionStamp {
        date: Date::new(
            num(&day[..4])?,
            num(&day[4..6])? as u8,
            num(&day[6..])? as u8,
        )?,
        hour: num(&time[..2])? as u8,
        minute: num(&time[2..4])? as u8,
        second: num(&time[4..])? as u8,
    };
    (stamp.hour < 24 && stamp.minute < 60 && stamp.second < 60).then_some(stamp)
}

/// Puts the newest recording first. Names without a start sort after the ones
/// with it, by name.
pub fn sort_newest_first(sessions: &mut [SessionInfo]) {
    sessions.sort_by(|a, b| (stamp_of(&b.id), &b.id).cmp(&(stamp_of(&a.id), &a.id)));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_name_is_never_a_path() {
        for good in ["lol-20261118-165329", "a", "x_y-1"] {
            assert!(is_valid_id(good), "{good}");
        }
        for bad in [
            "",
            "..",
            "../x",
            r"a\b",
            "a/b",
            "C:x",
            "a.b",
            "a b",
            &"x".repeat(65),
        ] {
            assert!(!is_valid_id(bad), "{bad:?}");
        }
    }

    #[test]
    fn the_start_is_read_from_the_name() {
        let stamp = stamp_of("lol-20261118-165329").unwrap();
        assert_eq!(stamp.to_string(), "2026-11-18 16:53:29");
        assert!(stamp_of("lol-20261118-165329") > stamp_of("lol-20261118-145915"));
    }

    fn info(id: &str) -> SessionInfo {
        SessionInfo {
            id: id.into(),
            bytes: 1,
            has_summary: false,
        }
    }

    #[test]
    fn the_newest_recording_goes_first() {
        let mut list = vec![
            info("lol-20261118-145915"),
            info("lol-20261119-080000"),
            info("odd-name"),
            info("lol-20261118-165329"),
        ];
        sort_newest_first(&mut list);
        let ids: Vec<&str> = list.iter().map(|s| s.id.as_str()).collect();
        assert_eq!(
            ids,
            [
                "lol-20261119-080000",
                "lol-20261118-165329",
                "lol-20261118-145915",
                "odd-name"
            ]
        );
    }

    #[test]
    fn a_name_of_another_shape_has_no_start() {
        for bad in [
            "lol",
            "lol-2026-1",
            "lol-20261318-165329",
            "lol-20261118-256000",
            "lol-20261118-16532",
        ] {
            assert_eq!(stamp_of(bad), None, "{bad}");
        }
    }
}
