//! The analysis of one recording: what it said about itself (its header), its
//! summary and the leads that follow from it.

use std::fmt::Write;

use serde::Serialize;

use crate::diagnosis::{diagnose, Finding};
use crate::session::{summarize, SessionSummary, Stat};
use crate::session_csv::ParsedSession;
use crate::session_header::SessionHeader;

/// A recording shorter than this is a test, not a game: its figures say nothing
/// about how a match goes.
pub const MIN_GAME_SECONDS: u64 = 120;

/// A line of the table of measures: average, 95th percentile and maximum.
#[derive(Debug, Clone, PartialEq)]
pub struct StatRow {
    pub label: String,
    pub stat: Stat,
    pub unit: &'static str,
    /// Decimals to show (0 or 1).
    pub decimals: usize,
}

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
        let mut summary = summarize(&session.samples);
        // The first recordings (no header line) took the clock speed from a source
        // that always answered the base clock: that reading says nothing.
        if session.header.is_none() {
            summary.cpu_clock_pct = Stat::default();
        }
        Ok(Self {
            id: id.to_string(),
            header: session.header,
            findings: diagnose(&summary),
            summary,
        })
    }

    /// Whether it is a match worth reporting on: long enough, and with the game
    /// running (a recording made with `--now` and no game says nothing of one).
    pub fn is_game(&self) -> bool {
        self.summary.seconds >= MIN_GAME_SECONDS && self.summary.game_cpu.max > 0.0
    }

    /// The table of measures of the recording, in the order it is read: the
    /// processor, the game, the families of programs, the graphics card. A
    /// measure that does not apply (no game, no card, a clock that was not read)
    /// is not in it.
    pub fn stat_rows(&self) -> Vec<StatRow> {
        let s = &self.summary;
        let row = |label: &str, stat: Stat, unit: &'static str, decimals: usize| StatRow {
            label: label.to_string(),
            stat,
            unit,
            decimals,
        };
        let mut rows = vec![
            row("CPU total", s.cpu_total, "%", 1),
            row("Núcleo más cargado", s.cpu_max_core, "%", 1),
            row("Interrupciones / DPC", s.dpc_interrupt, "%", 1),
        ];
        if s.cpu_clock_pct.avg > 0.0 {
            rows.push(row("Frecuencia (de la base)", s.cpu_clock_pct, "%", 0));
        }
        rows.push(row("RAM en uso", s.ram_used, "%", 0));
        if s.game_cpu.max > 0.0 {
            rows.push(row("El juego · CPU", s.game_cpu, "%", 1));
            rows.push(row("El juego · memoria", s.game_memory_mb, "MB", 0));
        }
        for group in &s.groups {
            rows.push(row(
                &format!("{} · CPU", group.group.label()),
                group.cpu,
                "%",
                1,
            ));
        }
        if let Some(gpu) = &s.gpu {
            rows.push(row("GPU · uso", gpu.util, "%", 0));
            rows.push(row("GPU · temperatura", gpu.temp_c, "°C", 0));
            rows.push(row("GPU · consumo", gpu.power_w, "W", 1));
        }
        rows
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
    fn only_a_long_recording_with_the_game_running_is_a_game() {
        let long_with_game = "t_s,cpu_total,cpu_max_core,game_cpu,game_mb
1.0,1.0,1.0,5.0,900
200.0,1.0,1.0,5.0,900
";
        assert!(Analysis::of("a", parse(long_with_game).unwrap())
            .unwrap()
            .is_game());
        let short = "t_s,cpu_total,cpu_max_core,game_cpu,game_mb
1.0,1.0,1.0,5.0,900
2.0,1.0,1.0,5.0,900
";
        assert!(!Analysis::of("a", parse(short).unwrap()).unwrap().is_game());
        let no_game = "t_s,cpu_total,cpu_max_core
1.0,1.0,1.0
500.0,1.0,1.0
";
        assert!(!Analysis::of("a", parse(no_game).unwrap())
            .unwrap()
            .is_game());
    }

    #[test]
    fn the_clock_of_a_recording_without_header_is_not_trusted() {
        let old = "t_s,cpu_total,cpu_max_core,cpu_mhz,cpu_max_mhz\n1.0,1.0,1.0,3801,3801\n";
        let analysis = Analysis::of("old", parse(old).unwrap()).unwrap();
        assert_eq!(analysis.summary.cpu_clock_pct, Stat::default());
        let new = format!("# purifex-session v1 game=\"a.exe\" cores=4 started=2026-11-18\n{old}");
        let analysis = Analysis::of("new", parse(&new).unwrap()).unwrap();
        assert_eq!(analysis.summary.cpu_clock_pct.avg, 100.0);
    }

    #[test]
    fn the_table_of_measures_only_has_the_rows_that_apply() {
        let bare = "t_s,cpu_total,cpu_max_core
1.0,1.0,1.0
2.0,1.0,1.0
";
        let rows = Analysis::of("a", parse(bare).unwrap()).unwrap().stat_rows();
        let labels: Vec<&str> = rows.iter().map(|r| r.label.as_str()).collect();
        assert_eq!(
            labels,
            [
                "CPU total",
                "Núcleo más cargado",
                "Interrupciones / DPC",
                "RAM en uso"
            ],
            "no game, no card, no clock, no families"
        );
        let full = "# purifex-session v1 game=\"a.exe\" cores=4 started=2026-11-18
t_s,cpu_total,cpu_max_core,cpu_mhz,cpu_max_mhz,game_cpu,game_mb,gpu_util,gpu_temp_c,gpu_power_w,groups
1.0,1.0,1.0,4000,3800,5.0,900,30,45,30.0,league_client=15.0
2.0,1.0,1.0,4000,3800,5.0,900,30,45,30.0,league_client=15.0
";
        let rows = Analysis::of("a", parse(full).unwrap()).unwrap().stat_rows();
        let labels: Vec<&str> = rows.iter().map(|r| r.label.as_str()).collect();
        for expected in [
            "Frecuencia (de la base)",
            "El juego · CPU",
            "El juego · memoria",
            "Cliente de League · CPU",
            "GPU · uso",
            "GPU · temperatura",
            "GPU · consumo",
        ] {
            assert!(labels.contains(&expected), "{expected}: {labels:?}");
        }
        assert_eq!(
            rows.iter()
                .find(|r| r.label == "El juego · memoria")
                .unwrap()
                .unit,
            "MB"
        );
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
