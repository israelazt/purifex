//! The analysis of one recording: what it said about itself (its header), its
//! summary and the leads that follow from it.

use std::fmt::Write;

use serde::Serialize;

use crate::diagnosis::{diagnose, Finding};
use crate::session::{summarize, SessionSummary};
use crate::session_csv::ParsedSession;
use crate::session_header::SessionHeader;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Analysis {
    pub id: String,
    pub header: Option<SessionHeader>,
    pub summary: SessionSummary,
    pub findings: Vec<Finding>,
}

impl Analysis {
    /// Analyzes a recording read back. One with no samples says nothing, so it is
    /// an error and not an empty analysis.
    pub fn of(id: &str, session: ParsedSession) -> Result<Self, String> {
        if session.samples.is_empty() {
            return Err(format!("la grabación «{id}» no tiene muestras"));
        }
        let summary = summarize(&session.samples);
        Ok(Self {
            id: id.to_string(),
            header: session.header,
            findings: diagnose(&summary),
            summary,
        })
    }

    pub fn to_text(&self) -> String {
        let mut out = String::new();
        let _ = write!(out, "Grabación {}", self.id);
        if let Some(header) = &self.header {
            let _ = write!(out, " · {} · {} núcleos", header.game, header.cores);
        }
        let _ = writeln!(out, "\n");
        out.push_str(&self.summary.to_text());
        out
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_else(|_| "{}".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::date::Date;
    use crate::session_csv::parse;

    const CSV: &str =
        "# purifex-session v1 game=\"League of Legends.exe\" cores=12 started=2026-11-18\n\
t_s,cpu_total,cpu_max_core,groups\n\
1.0,40.0,99.0,league_client=16.0\n\
2.0,40.0,98.0,league_client=17.0\n";

    #[test]
    fn it_summarizes_and_keeps_the_header() {
        let analysis = Analysis::of("lol-1", parse(CSV).unwrap()).unwrap();
        assert_eq!(analysis.summary.samples, 2);
        assert_eq!(analysis.header.as_ref().unwrap().cores, 12);
        assert_eq!(
            analysis.header.unwrap().started,
            Date::new(2026, 11, 18).unwrap()
        );
        assert!(analysis
            .findings
            .iter()
            .any(|f| f.title.contains("cliente de League")));
    }

    #[test]
    fn a_recording_without_samples_is_an_error() {
        let empty = parse("t_s,cpu_total,cpu_max_core\n").unwrap();
        assert!(Analysis::of("x", empty)
            .unwrap_err()
            .contains("no tiene muestras"));
    }

    #[test]
    fn the_text_names_the_game_and_the_json_has_the_leads() {
        let analysis = Analysis::of("lol-1", parse(CSV).unwrap()).unwrap();
        assert!(analysis
            .to_text()
            .contains("League of Legends.exe · 12 núcleos"));
        let value: serde_json::Value = serde_json::from_str(&analysis.to_json()).unwrap();
        assert_eq!(value["id"], "lol-1");
        assert!(!value["findings"].as_array().unwrap().is_empty());
        assert_eq!(value["header"]["game"], "League of Legends.exe");
    }
}
