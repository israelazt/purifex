//! The list of recordings, as the user sees it: when each started, how long it
//! lasted, and whether it can be read.

use std::fmt::Write;

use serde::{Serialize, Serializer};

use crate::session_id::SessionStamp;

impl Serialize for SessionStamp {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

/// One recording of the list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SessionRow {
    pub id: String,
    /// From its name; `None` if the name does not carry it.
    pub started: Option<SessionStamp>,
    pub seconds: u64,
    pub samples: usize,
    pub has_summary: bool,
    /// The game it recorded, when its header says.
    pub game: Option<String>,
    /// Why it cannot be read, if it cannot.
    pub problem: Option<String>,
}

/// `3 s`, `26 min 06 s`, `1 h 05 min`.
pub fn duration_text(seconds: u64) -> String {
    match seconds {
        0..=59 => format!("{seconds} s"),
        60..=3599 => format!("{} min {:02} s", seconds / 60, seconds % 60),
        _ => format!("{} h {:02} min", seconds / 3600, seconds % 3600 / 60),
    }
}

/// The table of the list for a terminal.
pub fn rows_to_text(rows: &[SessionRow]) -> String {
    if rows.is_empty() {
        return "Todavía no hay grabaciones. Graba una partida con `purifex watch`.".to_string();
    }
    let mut out = String::new();
    let _ = writeln!(out, "Grabaciones ({}), la más nueva primero:\n", rows.len());
    let _ = writeln!(
        out,
        "{:<22} {:<20} {:>12} {:>9}",
        "Nombre", "Inicio", "Duración", "Muestras"
    );
    for row in rows {
        let started = row
            .started
            .map_or_else(|| "-".to_string(), |s| s.to_string());
        match &row.problem {
            None => {
                let _ = writeln!(
                    out,
                    "{:<22} {:<20} {:>12} {:>9}",
                    row.id,
                    started,
                    duration_text(row.seconds),
                    row.samples
                );
            }
            Some(problem) => {
                let _ = writeln!(
                    out,
                    "{:<22} {:<20} no se puede leer: {problem}",
                    row.id, started
                );
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session_id::stamp_of;

    fn row(id: &str, seconds: u64) -> SessionRow {
        SessionRow {
            id: id.into(),
            started: stamp_of(id),
            seconds,
            samples: seconds as usize,
            has_summary: true,
            game: None,
            problem: None,
        }
    }

    #[test]
    fn durations_read_in_the_unit_that_fits() {
        assert_eq!(duration_text(3), "3 s");
        assert_eq!(duration_text(1566), "26 min 06 s");
        assert_eq!(duration_text(3900), "1 h 05 min");
        assert_eq!(duration_text(0), "0 s");
    }

    #[test]
    fn the_table_shows_each_recording_and_flags_the_one_that_cannot_be_read() {
        let mut broken = row("lol-20261118-120000", 0);
        broken.problem = Some("falta la columna «t_s»".into());
        let text = rows_to_text(&[row("lol-20261118-165329", 1566), broken]);
        assert!(text.contains("Grabaciones (2)"), "{text}");
        assert!(
            text.contains("2026-11-18 16:53:29") && text.contains("26 min 06 s"),
            "{text}"
        );
        assert!(
            text.contains("no se puede leer: falta la columna"),
            "{text}"
        );
    }

    #[test]
    fn an_empty_list_says_how_to_make_one() {
        assert!(rows_to_text(&[]).contains("purifex watch"));
    }

    #[test]
    fn the_json_carries_the_start_as_text() {
        let value = serde_json::to_value(row("lol-20261118-165329", 5)).unwrap();
        assert_eq!(value["started"], "2026-11-18 16:53:29");
        assert_eq!(value["seconds"], 5);
    }
}
