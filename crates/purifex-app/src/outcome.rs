//! What an order produced, as data. The program renders it as text or JSON; a
//! server (the MCP one) can use the data itself.

use purifex_domain::analysis::Analysis;
use purifex_domain::compare::ComparisonReport;
use purifex_domain::driver_offers::{offers_to_text, OfferReport};
use purifex_domain::fix::{history_to_text, proposals_to_text, FixReport, HistoryEntry, Proposal};
use purifex_domain::report::ScanReport;
use purifex_domain::session_list::{rows_to_text, SessionRow};

use crate::help::HELP;
use crate::recorder::Recording;

#[derive(Debug, Clone, PartialEq)]
pub enum Outcome {
    Report(ScanReport),
    Recording(Recording),
    Sessions(Vec<SessionRow>),
    Analysis(Box<Analysis>),
    Comparison(Box<ComparisonReport>),
    Offers(OfferReport),
    Fixes(Vec<Proposal>),
    Fix(Box<FixReport>),
    History(Vec<HistoryEntry>),
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
            Self::Comparison(report) => report.to_text(),
            Self::Offers(offers) => offers_to_text(offers),
            Self::Fixes(proposals) => proposals_to_text(proposals),
            Self::Fix(report) => report.to_text(),
            Self::History(entries) => history_to_text(entries),
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
            Self::Comparison(report) => return report.to_json(),
            Self::Offers(report) => {
                return serde_json::to_string_pretty(report).unwrap_or_else(|_| "{}".into())
            }
            Self::Fixes(proposals) => serde_json::json!({ "fixes": proposals }),
            Self::Fix(report) => return report.to_json(),
            Self::History(entries) => serde_json::json!({ "history": entries }),
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
