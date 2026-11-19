//! Fixes: changes Purifex can make to the computer to get better games. Each one
//! is **proposed only from what was measured**, explained before it is done,
//! recorded with the value it replaced, and can be undone. This module has the
//! data and the rules; the application carries them out (`purifex-app`) and the
//! system's adapters read and write the setting (`purifex-platform`).

use std::fmt::{self, Write};

use serde::{Deserialize, Serialize};

use crate::analysis::Analysis;
use crate::diagnosis::limit;
use crate::process_group::Group;

/// The fix of the League client that closes during the game.
pub const LOL_CLOSE_CLIENT: &str = "lol.close-client-during-game";

/// The value of a setting.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SettingValue {
    /// The setting is not written anywhere: the program uses its default.
    Unset,
    Bool(bool),
    Text(String),
}

impl fmt::Display for SettingValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unset => f.write_str("sin definir"),
            Self::Bool(true) => f.write_str("activado"),
            Self::Bool(false) => f.write_str("desactivado"),
            Self::Text(text) => f.write_str(text),
        }
    }
}

/// What the catalog knows of a fix.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FixSpec {
    pub id: &'static str,
    pub title: &'static str,
    /// How it is undone, in words.
    pub undo: &'static str,
    pub needs_admin: bool,
}

pub const CATALOG: [FixSpec; 1] = [FixSpec {
    id: LOL_CLOSE_CLIENT,
    title: "Cerrar el cliente de League durante la partida",
    undo:
        "Vuelve a poner el ajuste como estaba (el archivo de Riot se restaura a su valor anterior).",
    needs_admin: false,
}];

pub fn spec(id: &str) -> Option<&'static FixSpec> {
    CATALOG.iter().find(|s| s.id == id)
}

/// The value a fix leaves the setting at.
pub fn target_of(id: &str) -> Option<SettingValue> {
    (id == LOL_CLOSE_CLIENT).then_some(SettingValue::Bool(true))
}

/// Why this fix is worth proposing, from the last game: the measurement. `None`
/// if what was measured does not justify it (a fix is never offered "just in case").
pub fn evidence_for(id: &str, latest: Option<&Analysis>) -> Option<String> {
    let latest = latest.filter(|a| a.is_game())?;
    match id {
        LOL_CLOSE_CLIENT => {
            let client = latest
                .summary
                .groups
                .iter()
                .find(|g| g.group == Group::LeagueClient)?;
            (client.cpu.avg >= limit::LEAGUE_CLIENT).then(|| {
                format!(
                    "El cliente de League usó de media un {:.1} % del CPU durante tu última partida ({}), y el juego un {:.1} %.",
                    client.cpu.avg, latest.id, latest.summary.game_cpu.avg
                )
            })
        }
        _ => None,
    }
}

/// A fix being offered.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Proposal {
    pub id: String,
    pub title: String,
    /// The measurement that justifies it.
    pub evidence: String,
    pub current: SettingValue,
    pub target: SettingValue,
    pub undo: String,
    pub needs_admin: bool,
    /// What stops it right now, and what to do (`Cierra el cliente de League`).
    pub blockers: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HistoryState {
    Applied,
    Undone,
}

/// A line of the history: a change, or the undoing of one. Lines are only added.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub seq: u64,
    pub fix: String,
    pub before: SettingValue,
    pub after: SettingValue,
    pub state: HistoryState,
    /// For an `Undone` line, the `seq` of the change it undoes.
    pub undoes: Option<u64>,
    /// The recording that justified it.
    pub session: Option<String>,
}

impl HistoryEntry {
    /// The line of the history file for this entry (JSON, one line, no line end).
    pub fn to_line(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    /// An entry from a line of the history file.
    pub fn from_line(line: &str) -> Result<Self, String> {
        serde_json::from_str(line).map_err(|e| e.to_string())
    }
}

/// Whether the change `seq` was already undone by a later line.
pub fn is_undone(history: &[HistoryEntry], seq: u64) -> bool {
    history
        .iter()
        .any(|e| e.state == HistoryState::Undone && e.undoes == Some(seq))
}

/// What a `fix apply` or `fix undo` did (or would do).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FixReport {
    pub fix: String,
    pub title: String,
    pub before: SettingValue,
    pub after: SettingValue,
    /// Nothing was changed: it only says what would be.
    pub preview: bool,
    /// The history line, when something was changed.
    pub seq: Option<u64>,
    pub undo: String,
    /// A preview made only because no confirmation was given.
    pub needs_confirmation: bool,
    pub was_undo: bool,
}

impl FixReport {
    pub fn to_text(&self) -> String {
        let mut out = String::new();
        let verb = match (self.preview, self.was_undo) {
            (true, false) => "Se aplicaría",
            (true, true) => "Se desharía",
            (false, false) => "Aplicado",
            (false, true) => "Deshecho",
        };
        let _ = writeln!(out, "{verb}: {}", self.title);
        let _ = writeln!(out, "  ajuste: {} → {}", self.before, self.after);
        if let Some(seq) = self.seq {
            let _ = writeln!(out, "  anotado en el historial como #{seq}");
        }
        if !self.was_undo {
            let _ = writeln!(out, "  cómo se deshace: {}", self.undo);
        }
        if self.preview && self.needs_confirmation {
            let _ = writeln!(out, "\nNo se ha cambiado nada. Añade --yes para aplicarlo.");
        } else if self.preview {
            let _ = writeln!(out, "\nSimulación: no se ha cambiado nada.");
        }
        out
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_else(|_| "{}".into())
    }
}

pub fn proposals_to_text(proposals: &[Proposal]) -> String {
    if proposals.is_empty() {
        return "No hay arreglos que proponer: lo que se ha medido no los justifica.\n(Graba una partida de al menos 2 minutos con `purifex watch`.)".into();
    }
    let mut out = String::new();
    for p in proposals {
        let _ = writeln!(out, "• {}   [{}]", p.title, p.id);
        let _ = writeln!(out, "  por qué: {}", p.evidence);
        let _ = writeln!(out, "  cambia: {} → {}", p.current, p.target);
        let _ = writeln!(out, "  se deshace: {}", p.undo);
        for blocker in &p.blockers {
            let _ = writeln!(out, "  ahora no se puede: {blocker}");
        }
        let _ = writeln!(out, "  para aplicarlo: purifex fix apply {} --yes\n", p.id);
    }
    out
}

pub fn history_to_text(entries: &[HistoryEntry]) -> String {
    if entries.is_empty() {
        return "Todavía no se ha cambiado nada.".into();
    }
    let mut out = String::new();
    for e in entries.iter().rev() {
        let what = match e.state {
            HistoryState::Applied if is_undone(entries, e.seq) => {
                "aplicado (deshecho después)".to_string()
            }
            HistoryState::Applied => "aplicado".to_string(),
            HistoryState::Undone => format!("deshace #{}", e.undoes.unwrap_or(0)),
        };
        let _ = writeln!(
            out,
            "#{:<3} {:<32} {} → {}   {what}",
            e.seq, e.fix, e.before, e.after
        );
    }
    out
}

/// The folder of League of Legends from the file that Riot writes with the
/// installations (`RiotClientInstalls.json`): the first one in `associated_client`.
pub fn league_folder(installs_json: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(installs_json).ok()?;
    value
        .get("associated_client")?
        .as_object()?
        .keys()
        .find(|path| path.to_lowercase().contains("league of legends"))
        .cloned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session_csv::parse;

    fn game(client: f32) -> Analysis {
        let csv = format!("t_s,cpu_total,cpu_max_core,game_cpu,game_mb,groups\n1.0,40.0,80.0,13.0,900,league_client={client}\n300.0,40.0,80.0,13.0,900,league_client={client}\n");
        Analysis::of("lol-20261118-145915", parse(&csv).unwrap()).unwrap()
    }

    #[test]
    fn a_fix_is_proposed_only_with_the_measurement_that_justifies_it() {
        let evidence = evidence_for(LOL_CLOSE_CLIENT, Some(&game(15.9))).unwrap();
        assert!(
            evidence.contains("15.9 %")
                && evidence.contains("13.0 %")
                && evidence.contains("lol-20261118-145915"),
            "{evidence}"
        );
        assert_eq!(
            evidence_for(LOL_CLOSE_CLIENT, Some(&game(1.0))),
            None,
            "a client that barely uses CPU is not worth it"
        );
        assert_eq!(
            evidence_for(LOL_CLOSE_CLIENT, None),
            None,
            "no measurement, no proposal"
        );
        assert_eq!(evidence_for("unknown.fix", Some(&game(15.9))), None);
    }

    #[test]
    fn a_short_test_is_no_evidence() {
        let test = parse("t_s,cpu_total,cpu_max_core,game_cpu,game_mb,groups\n1.0,1.0,1.0,5.0,900,league_client=30.0\n").unwrap();
        let analysis = Analysis::of("x", test).unwrap();
        assert_eq!(evidence_for(LOL_CLOSE_CLIENT, Some(&analysis)), None);
    }

    #[test]
    fn the_catalog_knows_each_fix_and_its_target() {
        assert!(spec(LOL_CLOSE_CLIENT).is_some() && spec("nope").is_none());
        assert_eq!(target_of(LOL_CLOSE_CLIENT), Some(SettingValue::Bool(true)));
        assert!(
            CATALOG.iter().all(|s| target_of(s.id).is_some()),
            "every fix has a target"
        );
        assert!(
            CATALOG.iter().all(|s| !s.needs_admin),
            "the first fix needs no administrator"
        );
    }

    fn entry(seq: u64, state: HistoryState, undoes: Option<u64>) -> HistoryEntry {
        HistoryEntry {
            seq,
            fix: LOL_CLOSE_CLIENT.into(),
            before: SettingValue::Bool(false),
            after: SettingValue::Bool(true),
            state,
            undoes,
            session: None,
        }
    }

    #[test]
    fn a_change_is_undone_only_when_a_later_line_says_so() {
        let history = [
            entry(1, HistoryState::Applied, None),
            entry(2, HistoryState::Undone, Some(1)),
            entry(3, HistoryState::Applied, None),
        ];
        assert!(is_undone(&history, 1));
        assert!(!is_undone(&history, 3));
        let text = history_to_text(&history);
        assert!(
            text.contains("deshecho después") && text.contains("deshace #1"),
            "{text}"
        );
        assert!(
            text.find("#3").unwrap() < text.find("#1").unwrap(),
            "newest first"
        );
    }

    #[test]
    fn the_history_survives_a_round_trip_through_json_lines() {
        let line = serde_json::to_string(&entry(7, HistoryState::Undone, Some(5))).unwrap();
        assert!(!line.contains('\n'), "one line per entry");
        let back: HistoryEntry = serde_json::from_str(&line).unwrap();
        assert_eq!(back, entry(7, HistoryState::Undone, Some(5)));
        let unset: SettingValue =
            serde_json::from_str(&serde_json::to_string(&SettingValue::Unset).unwrap()).unwrap();
        assert_eq!(unset, SettingValue::Unset);
    }

    fn report(preview: bool, confirm: bool) -> FixReport {
        FixReport {
            fix: LOL_CLOSE_CLIENT.into(),
            title: "Cerrar el cliente".into(),
            before: SettingValue::Bool(false),
            after: SettingValue::Bool(true),
            preview,
            seq: (!preview).then_some(4),
            undo: "se restaura".into(),
            needs_confirmation: confirm,
            was_undo: false,
        }
    }

    #[test]
    fn a_preview_says_nothing_was_changed_and_how_to_apply_it() {
        let text = report(true, true).to_text();
        assert!(
            text.contains("Se aplicaría")
                && text.contains("desactivado → activado")
                && text.contains("--yes"),
            "{text}"
        );
        let dry = report(true, false).to_text();
        assert!(
            dry.contains("Simulación") && !dry.contains("--yes"),
            "{dry}"
        );
        let done = report(false, false).to_text();
        assert!(
            done.contains("Aplicado") && done.contains("#4") && done.contains("se restaura"),
            "{done}"
        );
    }

    #[test]
    fn proposals_are_listed_with_why_what_changes_how_to_undo_and_what_blocks_them() {
        let proposal = Proposal {
            id: LOL_CLOSE_CLIENT.into(),
            title: "Cerrar el cliente".into(),
            evidence: "usó el 15.9 %".into(),
            current: SettingValue::Bool(false),
            target: SettingValue::Bool(true),
            undo: "se restaura".into(),
            needs_admin: false,
            blockers: vec!["Cierra el cliente de League".into()],
        };
        let text = proposals_to_text(&[proposal]);
        for expected in [
            "por qué: usó el 15.9 %",
            "desactivado → activado",
            "se deshace",
            "ahora no se puede: Cierra el cliente de League",
            "purifex fix apply lol.close-client-during-game --yes",
        ] {
            assert!(text.contains(expected), "{expected}:\n{text}");
        }
        assert!(proposals_to_text(&[]).contains("no los justifica"));
    }

    #[test]
    fn the_league_folder_comes_from_the_installs_file_of_riot() {
        let json = r#"{ "associated_client": { "C:/Riot Games/League of Legends/": "C:/Riot Games/Riot Client/RiotClientServices.exe" }, "rc_default": "x" }"#;
        assert_eq!(
            league_folder(json).as_deref(),
            Some("C:/Riot Games/League of Legends/")
        );
        assert_eq!(league_folder(r#"{ "associated_client": {} }"#), None);
        assert_eq!(
            league_folder(r#"{ "associated_client": { "D:/Games/Valorant/": "x" } }"#),
            None
        );
        assert_eq!(league_folder("not json"), None);
        assert_eq!(league_folder("{}"), None);
    }
}
