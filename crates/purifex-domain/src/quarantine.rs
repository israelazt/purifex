//! What the quarantine holds and when it is let go. A removal moves a trace to the quarantine
//! (see `cleanup_log`); it stays there to be put back until the person empties it, or until
//! it has been there `QUARANTINE_DAYS` days. These are the rules and the wording; the
//! deleting itself is the store's, and only inside the quarantine folder.

use serde::Serialize;

use crate::apps::size_text;
use crate::cleanup_log::{CleanupEntry, CleanupState};
use crate::date::Date;

/// How many days a removed trace stays in the quarantine before it is let go.
pub const QUARANTINE_DAYS: i64 = 7;

/// The removals whose copy is still in the quarantine: not put back, not let go.
pub fn open_removals(entries: &[CleanupEntry]) -> Vec<&CleanupEntry> {
    entries
        .iter()
        .filter(|e| e.state == CleanupState::Removed)
        .filter(|e| {
            !entries.iter().any(|later| {
                matches!(later.state, CleanupState::Restored | CleanupState::Purged)
                    && later.settles == Some(e.seq)
            })
        })
        .collect()
}

/// The open removals of one app (what putting its traces back means).
pub fn open_removals_of<'a>(entries: &'a [CleanupEntry], app: &str) -> Vec<&'a CleanupEntry> {
    open_removals(entries)
        .into_iter()
        .filter(|e| e.app == app)
        .collect()
}

/// The open removals that are `QUARANTINE_DAYS` days old or more on `today`. A removal
/// written before the date was kept has no age: it never expires by itself, only when the
/// person empties the quarantine.
pub fn expired(entries: &[CleanupEntry], today: Date) -> Vec<&CleanupEntry> {
    open_removals(entries)
        .into_iter()
        .filter(|e| {
            e.on.is_some_and(|day| day.days_until(today) >= QUARANTINE_DAYS)
        })
        .collect()
}

/// The app that was cleaned last and how many of its traces are still in the quarantine:
/// what the button to put things back refers to.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Latest {
    pub app: String,
    pub items: usize,
}

/// What the quarantine holds now.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct QuarantineInfo {
    /// Traces that can still be put back.
    pub items: usize,
    /// What they take on disk.
    pub bytes: u64,
    /// The day of the oldest one that has a date.
    pub oldest: Option<Date>,
    /// How many are old enough to be let go now.
    pub expired: usize,
    /// The app whose removal is the most recent one still open.
    pub latest: Option<Latest>,
}

impl QuarantineInfo {
    pub fn of(entries: &[CleanupEntry], bytes: u64, today: Date) -> QuarantineInfo {
        let open = open_removals(entries);
        QuarantineInfo {
            items: open.len(),
            bytes,
            oldest: open.iter().filter_map(|e| e.on).min(),
            expired: expired(entries, today).len(),
            latest: open.iter().max_by_key(|e| e.seq).map(|last| Latest {
                app: last.app.clone(),
                items: open_removals_of(entries, &last.app).len(),
            }),
        }
    }

    pub fn to_text(&self) -> String {
        if self.items == 0 {
            return "La cuarentena está vacía.".to_string();
        }
        let mut text = format!(
            "La cuarentena guarda {} restos ({}). Se vacía sola a los {QUARANTINE_DAYS} días.",
            self.items,
            size_text(Some(self.bytes.div_ceil(1024)))
        );
        if let Some(day) = self.oldest {
            text.push_str(&format!(" El más antiguo es del {day}."));
        }
        text
    }
}

/// What putting back the traces of one app did.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct RestoreReport {
    pub app: String,
    pub restored: usize,
    /// The ones that could not be put back (the place is taken, the copy is gone), with why.
    pub failed: Vec<(u64, String)>,
}

impl RestoreReport {
    pub fn to_text(&self) -> String {
        let mut text = format!(
            "Se devolvieron {} restos de {} a su sitio.",
            self.restored, self.app
        );
        for (seq, why) in &self.failed {
            text.push_str(&format!(
                "
no se pudo devolver #{seq}: {why}"
            ));
        }
        text
    }
}

/// What emptying the quarantine did, or would do.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct PurgeReport {
    /// Only said what it would delete: nothing was touched.
    pub preview: bool,
    /// How many traces were (or would be) let go for good.
    pub purged: usize,
    /// The space freed (or that would be).
    pub bytes: u64,
    /// The removals that could not be let go, with why.
    pub failed: Vec<(u64, String)>,
}

/// What handing the open leftovers of an app to Windows (to delete at the next restart) did.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct RebootReport {
    pub app: String,
    /// What Windows will delete at the next restart.
    pub scheduled: Vec<crate::app_traces::Trace>,
    /// What it could not be asked to delete, with why.
    pub failed: Vec<(crate::app_traces::Trace, String)>,
}

impl RebootReport {
    pub fn to_text(&self) -> String {
        let mut text = match self.scheduled.len() {
            0 => format!(
                "No hay restos de {} abiertos que borrar al reiniciar.",
                self.app
            ),
            n => format!(
                "Windows borrará {n} restos de {} cuando reinicies el PC. No se podrán devolver.",
                self.app
            ),
        };
        for (trace, why) in &self.failed {
            text.push_str(&format!("\nno se pudo programar {}: {why}", trace.location));
        }
        text
    }
}

impl PurgeReport {
    pub fn to_text(&self) -> String {
        let size = size_text(Some(self.bytes.div_ceil(1024)));
        let mut text = match (self.preview, self.purged) {
            (_, 0) => "No hay nada que vaciar en la cuarentena.".to_string(),
            (true, n) => format!(
                "Se borrarían para siempre {n} restos ({size}), y ya no se podrán devolver. Para hacerlo: purifex apps purge --yes"
            ),
            (false, n) => format!("Se borraron para siempre {n} restos ({size})."),
        };
        for (seq, why) in &self.failed {
            text.push_str(&format!("\nno se pudo vaciar #{seq}: {why}"));
        }
        text
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_else(|_| "{}".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn day(d: u8) -> Date {
        Date::new(2026, 11, d).unwrap()
    }

    fn removed(seq: u64, on: Option<Date>) -> CleanupEntry {
        CleanupEntry {
            seq,
            on,
            ..Default::default()
        }
    }

    fn closing(seq: u64, state: CleanupState, settles: u64) -> CleanupEntry {
        CleanupEntry {
            seq,
            state,
            settles: Some(settles),
            ..Default::default()
        }
    }

    #[test]
    fn what_was_put_back_or_let_go_is_no_longer_in_the_quarantine() {
        let log = [
            removed(1, Some(day(1))),
            removed(2, Some(day(2))),
            removed(3, Some(day(3))),
            closing(4, CleanupState::Restored, 1),
            closing(5, CleanupState::Purged, 2),
        ];
        let open: Vec<u64> = open_removals(&log).iter().map(|e| e.seq).collect();
        assert_eq!(open, [3]);
    }

    #[test]
    fn a_removal_expires_at_seven_days_and_one_without_a_date_never_does() {
        let log = [
            removed(1, Some(day(1))),
            removed(2, Some(day(2))),
            removed(3, None),
        ];
        let seqs =
            |today: Date| -> Vec<u64> { expired(&log, today).iter().map(|e| e.seq).collect() };
        assert!(seqs(day(7)).is_empty(), "six days");
        assert_eq!(seqs(day(8)), [1], "seven days");
        assert_eq!(seqs(day(9)), [1, 2]);
        assert_eq!(
            seqs(day(28)),
            [1, 2],
            "the undated one waits for the person"
        );
    }

    #[test]
    fn the_summary_counts_what_is_open_and_says_how_old_and_how_big() {
        let log = [
            removed(1, Some(day(2))),
            removed(2, Some(day(5))),
            closing(3, CleanupState::Purged, 1),
        ];
        let info = QuarantineInfo::of(&log, 3 * 1024 * 1024, day(20));
        assert_eq!((info.items, info.expired), (1, 1));
        assert_eq!(info.oldest, Some(day(5)));
        let text = info.to_text();
        assert!(text.contains("guarda 1 restos (3 MB)") && text.contains("a los 7 días"));
        assert!(text.contains("2026-11-05"));
        assert_eq!(
            QuarantineInfo::of(&[], 0, day(1)).to_text(),
            "La cuarentena está vacía."
        );
    }

    #[test]
    fn the_latest_app_is_the_one_of_the_most_recent_open_removal_with_all_its_traces() {
        let of = |seq: u64, app: &str| CleanupEntry {
            seq,
            app: app.into(),
            ..Default::default()
        };
        let latest = |log: &[CleanupEntry]| QuarantineInfo::of(log, 0, day(1)).latest;
        let mut log = vec![of(1, "Kdenlive"), of(2, "Discord"), of(3, "Kdenlive")];
        assert_eq!(
            latest(&log),
            Some(Latest {
                app: "Kdenlive".into(),
                items: 2
            })
        );
        // Putting #3 back leaves Discord's #2 as the most recent one still open.
        log.push(closing(4, CleanupState::Restored, 3));
        assert_eq!(
            latest(&log),
            Some(Latest {
                app: "Discord".into(),
                items: 1
            })
        );
        assert_eq!(latest(&[]), None);
    }

    #[test]
    fn the_purge_report_says_what_it_would_do_what_it_did_and_what_failed() {
        let preview = PurgeReport {
            preview: true,
            purged: 3,
            bytes: 2048,
            failed: vec![],
        };
        assert!(preview
            .to_text()
            .contains("Se borrarían para siempre 3 restos (2 KB)"));
        assert!(preview.to_text().contains("purifex apps purge --yes"));
        let done = PurgeReport {
            preview: false,
            failed: vec![(4, "en uso".into())],
            ..preview
        };
        assert!(done.to_text().contains("Se borraron para siempre 3 restos"));
        assert!(done.to_text().contains("no se pudo vaciar #4: en uso"));
        assert_eq!(
            PurgeReport::default().to_text(),
            "No hay nada que vaciar en la cuarentena."
        );
        assert!(done.to_json().contains("\"purged\": 3"));
    }
}
