//! The first line of a session file: what was recorded and with which version of
//! the format, so a reader never has to guess (`# purifex-session v1 game="League
//! of Legends.exe" cores=12 started=2026-11-18`).

use crate::date::Date;

/// The version of the CSV format written today.
pub const FORMAT_VERSION: u32 = 1;
const PREFIX: &str = "# purifex-session v";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionHeader {
    pub version: u32,
    /// The program of the game that was recorded.
    pub game: String,
    /// Logical cores of the CPU (what a "share of the whole CPU" is a share of).
    pub cores: u32,
    pub started: Date,
}

impl SessionHeader {
    pub fn new(game: &str, cores: u32, started: Date) -> Self {
        Self {
            version: FORMAT_VERSION,
            game: game.to_string(),
            cores,
            started,
        }
    }

    /// The line written at the top of the file (no trailing newline).
    pub fn to_line(&self) -> String {
        format!(
            "{PREFIX}{} game=\"{}\" cores={} started={}",
            self.version,
            self.game.replace('"', ""),
            self.cores,
            self.started
        )
    }

    /// Reads the first line of a file. `None` if it is not a header (a file
    /// recorded before there were headers starts straight with the columns).
    pub fn parse(line: &str) -> Option<Self> {
        let rest = line.trim().strip_prefix(PREFIX)?;
        let (version, mut rest) = rest.split_once(' ').unwrap_or((rest, ""));
        let version = version.parse().ok()?;
        let (mut game, mut cores, mut started) = (None, None, None);
        while let Some((key, after)) = rest.trim_start().split_once('=') {
            let (value, next) = match after.strip_prefix('"') {
                Some(quoted) => quoted.split_once('"')?,
                None => after.split_once(' ').unwrap_or((after, "")),
            };
            match key {
                "game" => game = Some(value.to_string()),
                "cores" => cores = value.parse().ok(),
                "started" => started = parse_date(value),
                _ => {}
            }
            rest = next;
        }
        Some(Self {
            version,
            game: game?,
            cores: cores?,
            started: started?,
        })
    }
}

fn parse_date(text: &str) -> Option<Date> {
    let mut parts = text.split('-');
    let date = Date::new(
        parts.next()?.parse().ok()?,
        parts.next()?.parse().ok()?,
        parts.next()?.parse().ok()?,
    )?;
    parts.next().is_none().then_some(date)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn header() -> SessionHeader {
        SessionHeader::new(
            "League of Legends.exe",
            12,
            Date::new(2026, 11, 18).unwrap(),
        )
    }

    #[test]
    fn the_line_reads_back_as_the_same_header() {
        let line = header().to_line();
        assert_eq!(
            line,
            "# purifex-session v1 game=\"League of Legends.exe\" cores=12 started=2026-11-18"
        );
        assert_eq!(SessionHeader::parse(&line), Some(header()));
    }

    #[test]
    fn a_file_without_header_is_recognized_by_it_not_being_one() {
        assert_eq!(SessionHeader::parse("t_s,cpu_total,cpu_max_core"), None);
        assert_eq!(SessionHeader::parse(""), None);
    }

    #[test]
    fn a_damaged_header_is_not_guessed() {
        for bad in [
            "# purifex-session vX game=\"a\" cores=1 started=2026-11-18",
            "# purifex-session v1 game=\"a\" cores=1",
            "# purifex-session v1 game=\"a\" cores=1 started=2026-13-01",
            "# purifex-session v1 game=\"unterminated cores=1 started=2026-11-18",
        ] {
            assert_eq!(SessionHeader::parse(bad), None, "{bad}");
        }
    }

    #[test]
    fn unknown_keys_of_a_future_version_are_ignored() {
        let line = "# purifex-session v2 game=\"a.exe\" cores=8 started=2027-01-02 gpu=nvidia";
        let parsed = SessionHeader::parse(line).unwrap();
        assert_eq!((parsed.version, parsed.cores), (2, 8));
    }

    #[test]
    fn quotes_in_a_name_cannot_break_the_line() {
        let tricky = SessionHeader::new("a\".exe", 4, Date::new(2026, 1, 1).unwrap());
        assert_eq!(
            SessionHeader::parse(&tricky.to_line()).unwrap().game,
            "a.exe"
        );
    }
}
