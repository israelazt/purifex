//! What was removed from the computer after an app was uninstalled, line by line, so
//! it can be put back. Lines are only added; a removal that was undone gets a
//! second line that says so.

use serde::{Deserialize, Serialize};

use crate::app_traces::{Trace, TraceGroup};
use crate::date::Date;
use crate::view::{REGISTRY_ROOM, TRACES_LISTED};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CleanupState {
    #[default]
    Removed,
    /// A line that says a removal was put back.
    Restored,
    /// A line that says the quarantine copy of a removal was deleted for good.
    Purged,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CleanupEntry {
    pub seq: u64,
    pub app: String,
    pub trace: Trace,
    /// Where the removed thing is kept (the quarantine folder).
    pub kept: String,
    pub state: CleanupState,
    /// For a `Restored` or `Purged` line: the `seq` of the removal it closes. (Before purging
    /// existed it was called `restores`: old logs still read.)
    #[serde(default, alias = "restores")]
    pub settles: Option<u64>,
    /// The day of this line: when it was removed, put back or deleted (a line written before
    /// the day was kept has none).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on: Option<Date>,
}

impl CleanupEntry {
    pub fn to_line(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    pub fn from_line(line: &str) -> Result<Self, String> {
        serde_json::from_str(line).map_err(|e| e.to_string())
    }
}

/// Whether the removal `seq` was already put back.
pub fn is_restored(entries: &[CleanupEntry], seq: u64) -> bool {
    entries
        .iter()
        .any(|e| e.state == CleanupState::Restored && e.settles == Some(seq))
}

/// Whether the quarantine copy of the removal `seq` was deleted for good.
pub fn is_purged(entries: &[CleanupEntry], seq: u64) -> bool {
    entries
        .iter()
        .any(|e| e.state == CleanupState::Purged && e.settles == Some(seq))
}

/// The next number for a line.
pub fn next_seq(entries: &[CleanupEntry]) -> u64 {
    entries.iter().map(|e| e.seq).max().unwrap_or(0) + 1
}

/// What `clean` did, or would do.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct CleanReport {
    pub app: String,
    /// Only said what it would remove: nothing was touched.
    pub preview: bool,
    pub removed: Vec<CleanupEntry>,
    /// What was proposed (for a preview) and what could not be removed, with why.
    pub proposed: Vec<Trace>,
    pub failed: Vec<(Trace, String)>,
    /// The search ran out of time (reading shortcuts on a slow disk) and may have missed
    /// some traces: it should be looked for again.
    pub incomplete: bool,
    /// The app is still installed: its traces are in use and are not removed until its
    /// uninstaller has finished.
    pub still_installed: bool,
}

impl CleanReport {
    /// How many of the proposed traces only an administrator can remove.
    pub fn needing_admin(&self) -> usize {
        self.proposed.iter().filter(|t| t.needs_admin()).count()
    }

    /// The traces the window lists, in the order it lists them: files and folders first,
    /// then the registry, each biggest first. At most `TRACES_LISTED`; when both kinds exist
    /// the registry keeps some room. What does not fit is only removed by taking all.
    pub fn listed(&self) -> Vec<&Trace> {
        let (registry, mut files): (Vec<&Trace>, Vec<&Trace>) =
            self.proposed.iter().partition(|t| t.is_registry());
        // The tasks go after the files (the sort keeps what was already in order).
        files.sort_by_key(|t| t.group() == TraceGroup::Tasks);
        let files_room = TRACES_LISTED - registry.len().min(REGISTRY_ROOM);
        let shown_files = files.len().min(files_room);
        let shown_registry = registry.len().min(TRACES_LISTED - shown_files);
        files
            .into_iter()
            .take(shown_files)
            .chain(registry.into_iter().take(shown_registry))
            .collect()
    }

    pub fn to_text(&self) -> String {
        use std::fmt::Write;
        let mut out = String::new();
        if self.preview {
            out.push_str(&crate::trace_text::traces_to_text_for(
                &self.app,
                &self.proposed,
            ));
            if !self.proposed.is_empty() {
                let _ = writeln!(
                    out,
                    "
Para quitarlos (a cuarentena, con deshacer): purifex apps clean \"{}\" --yes",
                    self.app
                );
            }
            return out;
        }
        for e in &self.removed {
            let _ = writeln!(
                out,
                "#{} quitado: {} (guardado en {})",
                e.seq, e.trace.location, e.kept
            );
        }
        for (t, why) in &self.failed {
            let _ = writeln!(out, "no se pudo quitar {}: {why}", t.location);
        }
        if !self.removed.is_empty() {
            let removed: Vec<Trace> = self.removed.iter().map(|e| e.trace.clone()).collect();
            let _ = writeln!(
                out,
                "Quitado: {}",
                crate::trace_text::Counts::of(&removed).to_text()
            );
            out.push_str("Se pueden devolver con `purifex apps restore N`.");
        }
        out
    }

    /// The report as JSON: each trace with its `kind` (`folder`, `file`, `registry_key`,
    /// `registry_value`), `evidence` and, for a value, its `value`; and `counts` of what
    /// the report is about (the proposed traces of a preview, the removed ones otherwise).
    pub fn to_json(&self) -> String {
        let mut json = serde_json::to_value(self).unwrap_or_default();
        let traces: Vec<Trace> = if self.preview {
            self.proposed.clone()
        } else {
            self.removed.iter().map(|e| e.trace.clone()).collect()
        };
        let counts = crate::trace_text::Counts::of(&traces);
        if let Some(object) = json.as_object_mut() {
            object.insert(
                "counts".into(),
                serde_json::json!({
                    "files": counts.files,
                    "registry": counts.registry,
                    "bytes": counts.bytes,
                }),
            );
        }
        serde_json::to_string_pretty(&json).unwrap_or_else(|_| "{}".into())
    }
}

/// The copy of one registry value kept in the quarantine: where it was, its type code
/// and its bytes (as hexadecimal text, so the file is plain JSON), exactly as read.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValueCopy {
    pub key: String,
    pub name: String,
    pub kind: u32,
    pub hex: String,
}

impl ValueCopy {
    pub fn new(key: &str, name: &str, kind: u32, data: &[u8]) -> ValueCopy {
        use std::fmt::Write;
        let mut hex = String::with_capacity(data.len() * 2);
        for b in data {
            let _ = write!(hex, "{b:02x}");
        }
        ValueCopy {
            key: key.into(),
            name: name.into(),
            kind,
            hex,
        }
    }

    pub fn data(&self) -> Result<Vec<u8>, String> {
        if !self.hex.len().is_multiple_of(2) || !self.hex.is_ascii() {
            return Err("la copia del valor está dañada".into());
        }
        (0..self.hex.len())
            .step_by(2)
            .map(|i| {
                u8::from_str_radix(&self.hex[i..i + 2], 16)
                    .map_err(|_| "la copia del valor está dañada".to_string())
            })
            .collect()
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }

    pub fn from_json(text: &str) -> Result<ValueCopy, String> {
        serde_json::from_str(text).map_err(|e| format!("la copia del valor está dañada: {e}"))
    }
}

pub fn cleanup_to_text(entries: &[CleanupEntry]) -> String {
    use std::fmt::Write;
    if entries.is_empty() {
        return "Todavía no se ha quitado ningún resto.".into();
    }
    let mut out = String::new();
    for e in entries.iter().rev() {
        let what = match e.state {
            CleanupState::Removed if is_restored(entries, e.seq) => "quitado y devuelto",
            CleanupState::Removed if is_purged(entries, e.seq) => "quitado (copia borrada)",
            CleanupState::Removed => "quitado",
            CleanupState::Restored => "devuelto",
            CleanupState::Purged => "copia de la cuarentena borrada",
        };
        let _ = writeln!(out, "#{} {} — {}: {}", e.seq, what, e.app, e.trace.location);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    fn report_of(files: usize, registry: usize) -> CleanReport {
        let file = |i: usize| Trace {
            location: format!(r"C:\x\file{i}"),
            bytes: 100 - i as u64,
            ..Default::default()
        };
        let key = |i: usize| Trace {
            kind: TraceKind::RegistryKey,
            location: format!(r"HKCU\Software\key{i}"),
            ..Default::default()
        };
        CleanReport {
            // Mixed on purpose: the order of `proposed` is by size, not by kind.
            proposed: (0..registry).map(key).chain((0..files).map(file)).collect(),
            ..Default::default()
        }
    }

    fn kinds(report: &CleanReport) -> (usize, usize) {
        let listed = report.listed();
        let registry = listed.iter().filter(|t| t.is_registry()).count();
        (listed.len() - registry, registry)
    }

    #[test]
    fn the_list_has_files_first_then_registry_and_keeps_room_for_both() {
        assert_eq!(
            kinds(&report_of(7, 10)),
            (5, 4),
            "room kept for the registry"
        );
        assert_eq!(
            kinds(&report_of(20, 2)),
            (7, 2),
            "few registry entries: all of them"
        );
        assert_eq!(kinds(&report_of(2, 20)), (2, 7), "few files: all of them");
        assert_eq!(kinds(&report_of(20, 0)), (TRACES_LISTED, 0));
        assert_eq!(kinds(&report_of(0, 20)), (0, TRACES_LISTED));
        assert_eq!(kinds(&report_of(3, 3)), (3, 3));
        assert!(report_of(0, 0).listed().is_empty());
        let report = report_of(7, 10);
        let listed = report.listed();
        assert!(
            !listed[0].is_registry() && listed[8].is_registry(),
            "files first"
        );
        assert_eq!(
            listed[0].location, r"C:\x\file0",
            "each group keeps its order"
        );
    }

    #[test]
    fn a_registry_value_is_told_from_its_neighbours_by_its_key_and_name() {
        let value = |name: &str| Trace {
            kind: TraceKind::RegistryValue,
            location: r"HKCU\Software\Cache".into(),
            value: Some(name.into()),
            ..Default::default()
        };
        assert_ne!(value("a").key(), value("b").key());
        let folder = Trace {
            location: r"C:\x".into(),
            ..Default::default()
        };
        assert_eq!(folder.key(), r"C:\x", "a file or a folder is its place");
        assert!(value("a").is_registry() && !folder.is_registry());
    }

    fn mixed_report(preview: bool) -> CleanReport {
        use crate::app_traces::Evidence;
        let folder = Trace {
            kind: TraceKind::Folder,
            location: r"C:\Users\u\AppData\Roaming\Kdenlive".into(),
            bytes: 2048,
            why: "w".into(),
            user_data: true,
            evidence: Evidence::NameMatch,
            ..Default::default()
        };
        let value = Trace {
            kind: TraceKind::RegistryValue,
            location: r"HKCU\Software\X\MuiCache".into(),
            why: "w".into(),
            value: Some("k.exe.FriendlyAppName".into()),
            evidence: Evidence::PathReference,
            ..Default::default()
        };
        let entries = |ts: &[Trace]| -> Vec<CleanupEntry> {
            ts.iter()
                .enumerate()
                .map(|(i, t)| CleanupEntry {
                    seq: i as u64 + 1,
                    app: "Kdenlive".into(),
                    trace: t.clone(),
                    kept: "k".into(),
                    state: CleanupState::Removed,
                    ..Default::default()
                })
                .collect()
        };
        let all = [folder, value];
        CleanReport {
            app: "Kdenlive".into(),
            preview,
            removed: if preview { vec![] } else { entries(&all) },
            proposed: if preview { all.to_vec() } else { vec![] },
            failed: vec![],
            ..Default::default()
        }
    }

    #[test]
    fn the_text_says_the_evidence_of_each_line_and_the_counts() {
        let text = mixed_report(true).to_text();
        assert!(text.contains("evidencia: nombre de la app ·"), "{text}");
        assert!(
            text.contains("evidencia: referencia a la carpeta de la app"),
            "{text}"
        );
        assert!(text.contains("[valor: k.exe.FriendlyAppName]"));
        assert!(text.contains("Archivos y carpetas: 1 · Registro: 1 · 2 KB"));
        let done = mixed_report(false).to_text();
        assert!(
            done.contains("Quitado: Archivos y carpetas: 1 · Registro: 1"),
            "{done}"
        );
    }

    #[test]
    fn the_json_has_kind_evidence_value_and_counts_for_a_preview_and_for_a_removal() {
        for preview in [true, false] {
            let json: serde_json::Value =
                serde_json::from_str(&mixed_report(preview).to_json()).unwrap();
            let traces: Vec<&serde_json::Value> = if preview {
                json["proposed"].as_array().unwrap().iter().collect()
            } else {
                json["removed"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|e| &e["trace"])
                    .collect()
            };
            assert_eq!(traces[0]["kind"], "folder");
            assert_eq!(traces[0]["evidence"], "name_match");
            assert_eq!(traces[0]["value"], serde_json::Value::Null);
            assert_eq!(traces[1]["kind"], "registry_value");
            assert_eq!(traces[1]["evidence"], "path_reference");
            assert_eq!(traces[1]["value"], "k.exe.FriendlyAppName");
            assert_eq!(json["counts"]["files"], 1);
            assert_eq!(json["counts"]["registry"], 1);
            assert_eq!(json["counts"]["bytes"], 2048);
        }
    }

    #[test]
    fn a_value_copy_keeps_every_byte_and_a_damaged_one_is_an_error() {
        let bytes: Vec<u8> = (0..=255).collect();
        let copy = ValueCopy::new(r"HKCU\Software\X", "n", 3, &bytes);
        let back = ValueCopy::from_json(&copy.to_json()).unwrap();
        assert_eq!(back.data().unwrap(), bytes);
        assert_eq!(
            ValueCopy::new("k", "n", 3, &[]).data().unwrap(),
            Vec::<u8>::new()
        );
        for bad in ["abc", "zz", "é1"] {
            let mut c = copy.clone();
            c.hex = bad.into();
            assert!(c.data().is_err(), "{bad}");
        }
        assert!(ValueCopy::from_json("{rota").is_err());
    }

    use crate::app_traces::TraceKind;

    fn entry(seq: u64, state: CleanupState, settles: Option<u64>) -> CleanupEntry {
        CleanupEntry {
            seq,
            app: "Discord".into(),
            trace: Trace {
                kind: TraceKind::Folder,
                location: r"C:\Users\u\AppData\Roaming\Discord".into(),
                bytes: 10,
                why: "x".into(),
                user_data: true,
                ..Default::default()
            },
            kept: r"C:\q\1".into(),
            state,
            settles,
            ..Default::default()
        }
    }

    #[test]
    fn a_line_written_before_purging_existed_still_reads() {
        let old = r#"{"seq":2,"app":"X","trace":{"kind":"folder","location":"C:\\x\\y","bytes":1,"why":"w","user_data":false},"kept":"k","state":"restored","restores":1}"#;
        let e = CleanupEntry::from_line(old).unwrap();
        assert_eq!(
            (e.state, e.settles, e.on),
            (CleanupState::Restored, Some(1), None)
        );
        let dated = r#"{"seq":3,"app":"X","trace":{"kind":"folder","location":"C:\\x","bytes":1,"why":"w","user_data":false},"kept":"k","state":"removed","settles":null,"on":"2026-11-19"}"#;
        let e = CleanupEntry::from_line(dated).unwrap();
        assert_eq!(e.on, Date::new(2026, 11, 19));
        assert_eq!(CleanupEntry::from_line(&e.to_line()).unwrap(), e);
    }

    #[test]
    fn a_line_survives_the_file_and_a_broken_one_is_an_error() {
        let e = entry(3, CleanupState::Removed, None);
        assert_eq!(CleanupEntry::from_line(&e.to_line()).unwrap(), e);
        assert!(CleanupEntry::from_line("{rota").is_err());
    }

    #[test]
    fn a_removal_is_restored_only_when_a_later_line_says_so_and_numbers_go_on() {
        let log = vec![
            entry(1, CleanupState::Removed, None),
            entry(2, CleanupState::Removed, None),
            entry(3, CleanupState::Restored, Some(1)),
        ];
        assert!(is_restored(&log, 1));
        assert!(!is_restored(&log, 2));
        assert_eq!(next_seq(&log), 4);
        assert_eq!(next_seq(&[]), 1);
        let text = cleanup_to_text(&log);
        assert!(
            text.contains("#1 quitado y devuelto")
                && text.contains("#2 quitado —")
                && text.contains("#3 devuelto")
        );
    }
}
