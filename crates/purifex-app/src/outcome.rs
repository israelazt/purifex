//! What an order produced, as data. The program renders it as text or JSON; a
//! server (the MCP one) can use the data itself.

use purifex_domain::analysis::Analysis;
use purifex_domain::report::ScanReport;
use purifex_domain::session_list::{rows_to_text, SessionRow};

use crate::command::HELP;
use crate::recorder::Recording;

#[derive(Debug, Clone, PartialEq)]
pub enum Outcome {
    Report(ScanReport),
    Recording(Recording),
    Sessions(Vec<SessionRow>),
    Analysis(Box<Analysis>),
    Help,
}

impl Outcome {
    pub fn to_text(&self) -> String {
        match self {
            Self::Report(report) => report.to_text(),
            Self::Recording(done) => format!(
                "\n{}\nDatos segundo a segundo: {}",
                done.summary.to_text(),
                done.file
            ),
            Self::Sessions(rows) => rows_to_text(rows),
            Self::Analysis(analysis) => analysis.to_text(),
            Self::Help => HELP.to_string(),
        }
    }

    pub fn to_json(&self) -> String {
        let value = match self {
            Self::Report(report) => return report.to_json(),
            Self::Recording(done) => serde_json::json!({
                "file": done.file,
                "summary": done.summary,
            }),
            Self::Sessions(rows) => serde_json::json!({ "sessions": rows }),
            Self::Analysis(analysis) => return analysis.to_json(),
            Self::Help => serde_json::json!({ "help": HELP }),
        };
        serde_json::to_string_pretty(&value).unwrap_or_else(|_| "{}".into())
    }

    /// The text or the JSON form.
    pub fn render(&self, json: bool) -> String {
        if json {
            self.to_json()
        } else {
            self.to_text()
        }
    }
}

#[cfg(test)]
mod tests {
    use purifex_domain::session::SessionSummary;

    use super::*;

    fn recording() -> Outcome {
        Outcome::Recording(Recording {
            summary: SessionSummary::default(),
            file: "fake/lol.csv".into(),
        })
    }

    #[test]
    fn a_recording_says_where_its_data_is() {
        assert!(recording().to_text().contains("fake/lol.csv"));
        let value: serde_json::Value = serde_json::from_str(&recording().to_json()).unwrap();
        assert_eq!(value["file"], "fake/lol.csv");
        assert_eq!(value["summary"]["samples"], 0);
    }

    #[test]
    fn help_has_a_json_form_too() {
        let value: serde_json::Value = serde_json::from_str(&Outcome::Help.to_json()).unwrap();
        assert!(value["help"].as_str().unwrap().contains("Uso:"));
        assert_eq!(Outcome::Help.render(false), HELP);
    }
}
