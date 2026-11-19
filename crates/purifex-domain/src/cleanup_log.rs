//! What was removed from the computer after an app was uninstalled, line by line, so
//! it can be put back. Lines are only added; a removal that was undone gets a
//! second line that says so.

use serde::{Deserialize, Serialize};

use crate::app_traces::Trace;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CleanupState {
    Removed,
    Restored,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CleanupEntry {
    pub seq: u64,
    pub app: String,
    pub trace: Trace,
    /// Where the removed thing is kept (the quarantine folder).
    pub kept: String,
    pub state: CleanupState,
    /// For a `Restored` line: the `seq` of the removal it undoes.
    pub restores: Option<u64>,
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
        .any(|e| e.state == CleanupState::Restored && e.restores == Some(seq))
}

/// The next number for a line.
pub fn next_seq(entries: &[CleanupEntry]) -> u64 {
    entries.iter().map(|e| e.seq).max().unwrap_or(0) + 1
}

/// What `clean` did, or would do.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CleanReport {
    pub app: String,
    /// Only said what it would remove: nothing was touched.
    pub preview: bool,
    pub removed: Vec<CleanupEntry>,
    /// What was proposed (for a preview) and what could not be removed, with why.
    pub proposed: Vec<Trace>,
    pub failed: Vec<(Trace, String)>,
}

impl CleanReport {
    pub fn to_text(&self) -> String {
        use std::fmt::Write;
        let mut out = String::new();
        if self.preview {
            out.push_str(&crate::app_traces::traces_to_text_for(
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
            out.push_str("Se pueden devolver con `purifex apps restore N`.");
        }
        out
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_else(|_| "{}".into())
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
            CleanupState::Removed => "quitado",
            CleanupState::Restored => "devuelto",
        };
        let _ = writeln!(out, "#{} {} — {}: {}", e.seq, what, e.app, e.trace.location);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_traces::TraceKind;

    fn entry(seq: u64, state: CleanupState, restores: Option<u64>) -> CleanupEntry {
        CleanupEntry {
            seq,
            app: "Discord".into(),
            trace: Trace {
                kind: TraceKind::Folder,
                location: r"C:\Users\u\AppData\Roaming\Discord".into(),
                bytes: 10,
                why: "x".into(),
                user_data: true,
            },
            kept: r"C:\q\1".into(),
            state,
            restores,
        }
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
