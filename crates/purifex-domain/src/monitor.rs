//! The install monitor: a picture of some places of the system before an installer runs and
//! another after, and the difference kept as a log of what that installer created. Later, when
//! the app is cleaned, the log names what it made (the strongest evidence). Pure: reading the
//! system and keeping the logs are ports.

use serde::{Deserialize, Serialize};

use crate::app_traces::{Candidate, Evidence, TraceKind};
use crate::date::Date;

/// One thing seen in a picture: a file, a folder, a key or a value, with what tells if it
/// changed (size and time, or the data).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Mark {
    pub kind: TraceKind,
    pub location: String,
    /// For a registry value: its name (`location` is the key).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    pub stamp: String,
    #[serde(default)]
    pub bytes: u64,
}

impl Mark {
    fn key(&self) -> (TraceKind, String, Option<String>) {
        (
            self.kind,
            self.location.to_lowercase(),
            self.value.as_ref().map(|v| v.to_lowercase()),
        )
    }
}

/// What is in `after` and was not in `before`. Only what is new: a folder that was already
/// there (and only changed) is not the installer's, and may be shared.
pub fn added(before: &[Mark], after: &[Mark]) -> Vec<Mark> {
    let known: std::collections::HashSet<_> = before.iter().map(Mark::key).collect();
    let mut new: Vec<Mark> = after
        .iter()
        .filter(|m| !known.contains(&m.key()))
        .cloned()
        .collect();
    new.sort_by(|a, b| a.location.cmp(&b.location).then(a.value.cmp(&b.value)));
    new
}

/// What one monitored installation created.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstallLog {
    pub id: String,
    /// The installer that ran (its file name).
    pub installer: String,
    /// The app that appeared in the list afterwards, when one did.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub app: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on: Option<Date>,
    /// Made here (`true`) or brought from another file (`false`).
    #[serde(default)]
    pub own: bool,
    pub marks: Vec<Mark>,
}

impl InstallLog {
    pub fn to_line(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    pub fn from_line(line: &str) -> Result<Self, String> {
        serde_json::from_str(line).map_err(|e| e.to_string())
    }

    /// Whether the log says it belongs to this app (same name, any case).
    pub fn is_of(&self, app_name: &str) -> bool {
        self.app
            .as_deref()
            .is_some_and(|a| a.trim().eq_ignore_ascii_case(app_name.trim()))
    }

    /// Its places as candidates; the rules still judge each one. A log brought from elsewhere
    /// only guesses.
    pub fn candidates(&self) -> Vec<Candidate> {
        let known = if self.own {
            Evidence::Monitored
        } else {
            Evidence::ImportedLog
        };
        self.marks
            .iter()
            .map(|m| Candidate {
                kind: m.kind,
                location: m.location.clone(),
                bytes: m.bytes,
                value: m.value.clone(),
                known: Some(known),
                ..Candidate::default()
            })
            .collect()
    }

    /// How many of each kind, in a sentence.
    pub fn counts_text(&self) -> String {
        let n = |f: fn(TraceKind) -> bool| self.marks.iter().filter(|m| f(m.kind)).count();
        let files = n(|k| matches!(k, TraceKind::File | TraceKind::Folder));
        let keys = n(|k| matches!(k, TraceKind::RegistryKey | TraceKind::RegistryValue));
        let places = match files {
            1 => "1 archivo o carpeta".to_string(),
            n => format!("{n} archivos y carpetas"),
        };
        let entries = match keys {
            1 => "1 entrada del registro".to_string(),
            n => format!("{n} entradas del registro"),
        };
        format!("{places}, {entries}")
    }
}

/// The id of the next log: the day and a number that makes it unique among `existing`.
pub fn next_id(existing: &[InstallLog], today: Option<Date>) -> String {
    let day = today.map_or_else(|| "sin-fecha".to_string(), |d| d.to_string());
    let mut n = 1;
    while existing.iter().any(|l| l.id == format!("{day}-{n}")) {
        n += 1;
    }
    format!("{day}-{n}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mark(kind: TraceKind, location: &str, stamp: &str) -> Mark {
        Mark {
            kind,
            location: location.into(),
            value: None,
            stamp: stamp.into(),
            bytes: 0,
        }
    }

    #[test]
    fn only_what_is_new_is_kept_and_a_changed_old_place_is_not() {
        let before = vec![
            mark(TraceKind::Folder, r"C:\ProgramData\Shared", "1"),
            mark(TraceKind::RegistryKey, r"HKCU\Software\Old", "a"),
        ];
        let after = vec![
            mark(TraceKind::Folder, r"c:\programdata\shared", "2"),
            mark(TraceKind::Folder, r"C:\ProgramData\Zed", "1"),
            mark(TraceKind::RegistryKey, r"HKCU\Software\Old", "a"),
            mark(TraceKind::RegistryKey, r"HKCU\Software\Zed", "x"),
        ];
        let new = added(&before, &after);
        let places: Vec<&str> = new.iter().map(|m| m.location.as_str()).collect();
        assert_eq!(places, [r"C:\ProgramData\Zed", r"HKCU\Software\Zed"]);
    }

    #[test]
    fn a_log_round_trips_and_is_matched_to_an_app_by_name() {
        let log = InstallLog {
            id: "2026-11-23-1".into(),
            installer: "zed-setup.exe".into(),
            app: Some("Zed Editor".into()),
            on: None,
            own: true,
            marks: vec![mark(TraceKind::Folder, r"C:\ProgramData\Zed", "1")],
        };
        assert_eq!(InstallLog::from_line(&log.to_line()).unwrap(), log);
        assert!(log.is_of(" zed editor ") && !log.is_of("Zed"));
        assert_eq!(log.candidates()[0].known, Some(Evidence::Monitored));
        let imported = InstallLog {
            own: false,
            ..log.clone()
        };
        assert_eq!(imported.candidates()[0].known, Some(Evidence::ImportedLog));
        assert!(Evidence::ImportedLog.is_guess() && !Evidence::Monitored.is_guess());
        assert_eq!(
            log.counts_text(),
            "1 archivo o carpeta, 0 entradas del registro"
        );
    }

    #[test]
    fn ids_do_not_repeat_within_a_day() {
        let day = Date::new(2026, 11, 23);
        let log = |id: &str| InstallLog {
            id: id.into(),
            installer: String::new(),
            app: None,
            on: day,
            own: true,
            marks: vec![],
        };
        assert_eq!(next_id(&[], day), "2026-11-23-1");
        assert_eq!(next_id(&[log("2026-11-23-1")], day), "2026-11-23-2");
    }
}
