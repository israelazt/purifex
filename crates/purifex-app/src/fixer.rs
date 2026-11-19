//! The fixes: what is proposed, applying one and undoing one. This is the only
//! way the program changes the computer, so every rule that keeps it safe is
//! here and tested: a fix is applied only if it is proposed *now* (with its
//! measurement), only when nothing blocks it, only after a confirmation; the
//! value that replaced is kept; the change is read back and, if it did not hold,
//! put back; and nothing is changed without a line in the history.

use purifex_domain::analysis::Analysis;
use purifex_domain::fix::{
    evidence_for, is_undone, spec, target_of, FixReport, HistoryEntry, HistoryState, Proposal,
    SettingValue, CATALOG,
};
use purifex_domain::log_warn;
use purifex_domain::ports::{FixBackend, HistoryStore};

pub struct Fixer {
    backend: Box<dyn FixBackend>,
    history: Box<dyn HistoryStore>,
}

/// A backend that knows no setting: what a program without fixes has.
struct Nothing;

impl FixBackend for Nothing {
    fn current(&self, id: &str) -> Result<SettingValue, String> {
        Err(format!("no se conoce el ajuste «{id}»"))
    }
    fn blockers(&self, _: &str) -> Result<Vec<String>, String> {
        Ok(Vec::new())
    }
    fn set(&mut self, id: &str, _: &SettingValue) -> Result<(), String> {
        Err(format!("no se conoce el ajuste «{id}»"))
    }
}

impl HistoryStore for Nothing {
    fn entries(&self) -> Result<Vec<HistoryEntry>, String> {
        Ok(Vec::new())
    }
    fn append(&mut self, _: &HistoryEntry) -> Result<(), String> {
        Err("no hay dónde anotar el historial".into())
    }
}

impl Fixer {
    pub fn new(backend: Box<dyn FixBackend>, history: Box<dyn HistoryStore>) -> Self {
        Self { backend, history }
    }

    /// A fixer that proposes and does nothing.
    pub fn disabled() -> Self {
        Self::new(Box::new(Nothing), Box::new(Nothing))
    }

    /// The fixes worth offering now: those the last game justifies and whose
    /// setting is not already as the fix leaves it. A setting that cannot be read
    /// (the game is not installed) is left out.
    pub fn proposals(&self, latest: Option<&Analysis>) -> Result<Vec<Proposal>, String> {
        let mut out = Vec::new();
        for fix in CATALOG {
            let (Some(evidence), Some(target)) = (evidence_for(fix.id, latest), target_of(fix.id))
            else {
                continue;
            };
            let current = match self.backend.current(fix.id) {
                Ok(current) => current,
                Err(error) => {
                    log_warn!("fix {}: {error}", fix.id);
                    continue;
                }
            };
            if current == target {
                continue;
            }
            out.push(Proposal {
                id: fix.id.into(),
                title: fix.title.into(),
                evidence,
                current,
                target,
                undo: fix.undo.into(),
                needs_admin: fix.needs_admin,
                blockers: self.backend.blockers(fix.id).unwrap_or_else(|e| vec![e]),
            });
        }
        Ok(out)
    }

    fn next_seq(entries: &[HistoryEntry]) -> u64 {
        entries.iter().map(|e| e.seq).max().unwrap_or(0) + 1
    }

    /// Applies a fix. Without `confirmed`, or with `dry_run`, it only says what
    /// it would do.
    pub fn apply(
        &mut self,
        id: &str,
        latest: Option<&Analysis>,
        confirmed: bool,
        dry_run: bool,
    ) -> Result<FixReport, String> {
        let proposal = self
            .proposals(latest)?
            .into_iter()
            .find(|p| p.id == id)
            .ok_or_else(|| format!("«{id}» no está propuesto ahora: solo se aplica lo que lo que se ha medido justifica (mira `purifex fixes`)"))?;
        if !proposal.blockers.is_empty() {
            return Err(format!(
                "ahora no se puede: {}",
                proposal.blockers.join(" · ")
            ));
        }
        let report = |preview: bool, seq: Option<u64>| FixReport {
            fix: proposal.id.clone(),
            title: proposal.title.clone(),
            before: proposal.current.clone(),
            after: proposal.target.clone(),
            preview,
            seq,
            undo: proposal.undo.clone(),
            needs_confirmation: !dry_run && !confirmed,
            was_undo: false,
        };
        if dry_run || !confirmed {
            return Ok(report(true, None));
        }
        let entries = self.history.entries()?;
        self.backend.set(id, &proposal.target)?;
        // Read it back: a change that did not hold is put back, not trusted.
        if self.backend.current(id).as_ref() != Ok(&proposal.target) {
            let _ = self.backend.set(id, &proposal.current);
            return Err("el ajuste no se quedó como se pidió; se dejó como estaba".into());
        }
        let seq = Self::next_seq(&entries);
        let entry = HistoryEntry {
            seq,
            fix: id.into(),
            before: proposal.current.clone(),
            after: proposal.target.clone(),
            state: HistoryState::Applied,
            undoes: None,
            session: latest.map(|a| a.id.clone()),
        };
        // Nothing is changed without a line that says so.
        if let Err(error) = self.history.append(&entry) {
            let _ = self.backend.set(id, &proposal.current);
            return Err(format!(
                "no se pudo anotar en el historial ({error}); se dejó como estaba"
            ));
        }
        Ok(report(false, Some(seq)))
    }

    /// Undoes the change `seq`: puts the value back as it was.
    pub fn undo(&mut self, seq: u64, confirmed: bool) -> Result<FixReport, String> {
        let entries = self.history.entries()?;
        let entry = entries
            .iter()
            .find(|e| e.seq == seq && e.state == HistoryState::Applied)
            .ok_or_else(|| format!("no hay un cambio #{seq} que deshacer"))?;
        if is_undone(&entries, seq) {
            return Err(format!("el cambio #{seq} ya está deshecho"));
        }
        let now = self.backend.current(&entry.fix)?;
        if now != entry.after {
            return Err(format!(
                "el ajuste se cambió fuera de Purifex (ahora: {now}, esperaba: {}); no lo toco",
                entry.after
            ));
        }
        let blockers = self.backend.blockers(&entry.fix)?;
        if !blockers.is_empty() {
            return Err(format!("ahora no se puede: {}", blockers.join(" · ")));
        }
        let title = spec(&entry.fix)
            .map_or(entry.fix.as_str(), |s| s.title)
            .to_string();
        let report = |preview: bool, seq: Option<u64>, after: SettingValue| FixReport {
            fix: entry.fix.clone(),
            title: title.clone(),
            before: entry.after.clone(),
            after,
            preview,
            seq,
            undo: String::new(),
            needs_confirmation: !confirmed,
            was_undo: true,
        };
        if !confirmed {
            return Ok(report(true, None, entry.before.clone()));
        }
        self.backend.set(&entry.fix, &entry.before)?;
        let after = self.backend.current(&entry.fix)?;
        // An `Unset` is put back as the default, so "it changed away" is what counts.
        let held = if entry.before == SettingValue::Unset {
            after != entry.after
        } else {
            after == entry.before
        };
        if !held {
            let _ = self.backend.set(&entry.fix, &entry.after);
            return Err("el ajuste no volvió a su valor anterior; se dejó como estaba".into());
        }
        let line = HistoryEntry {
            seq: Self::next_seq(&entries),
            fix: entry.fix.clone(),
            before: entry.after.clone(),
            after: after.clone(),
            state: HistoryState::Undone,
            undoes: Some(seq),
            session: None,
        };
        if let Err(error) = self.history.append(&line) {
            let _ = self.backend.set(&entry.fix, &entry.after);
            return Err(format!(
                "no se pudo anotar en el historial ({error}); se dejó como estaba"
            ));
        }
        Ok(report(false, Some(line.seq), after))
    }

    pub fn history(&self) -> Result<Vec<HistoryEntry>, String> {
        self.history.entries()
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use purifex_domain::fix::LOL_CLOSE_CLIENT;
    use purifex_domain::session_csv::parse;

    use super::*;
    use crate::testing::{FakeFixes, FixState};

    fn game(client: f32) -> Analysis {
        let csv = format!("t_s,cpu_total,cpu_max_core,game_cpu,game_mb,groups\n1.0,40.0,80.0,13.0,900,league_client={client}\n300.0,40.0,80.0,13.0,900,league_client={client}\n");
        Analysis::of("lol-1", parse(&csv).unwrap()).unwrap()
    }

    fn fixer(value: Option<SettingValue>) -> (Fixer, Rc<RefCell<FixState>>) {
        let (fake, state) = FakeFixes::with(value);
        (fake.fixer(), state)
    }

    fn off() -> Option<SettingValue> {
        Some(SettingValue::Bool(false))
    }

    #[test]
    fn it_proposes_only_what_the_measurement_justifies_and_is_not_already_done() {
        let (f, _) = fixer(off());
        let proposals = f.proposals(Some(&game(16.0))).unwrap();
        assert_eq!(proposals.len(), 1);
        assert_eq!(
            (proposals[0].current.clone(), proposals[0].target.clone()),
            (SettingValue::Bool(false), SettingValue::Bool(true))
        );
        assert!(proposals[0].evidence.contains("16.0 %"));
        assert!(
            f.proposals(Some(&game(1.0))).unwrap().is_empty(),
            "no evidence"
        );
        assert!(f.proposals(None).unwrap().is_empty(), "no recording");
        let (done, _) = fixer(Some(SettingValue::Bool(true)));
        assert!(
            done.proposals(Some(&game(16.0))).unwrap().is_empty(),
            "already as the fix leaves it"
        );
        let (missing, _) = fixer(None);
        assert!(
            missing.proposals(Some(&game(16.0))).unwrap().is_empty(),
            "the game is not installed"
        );
    }

    #[test]
    fn an_id_that_was_not_proposed_is_rejected() {
        let (mut f, state) = fixer(off());
        for id in ["unknown.fix", LOL_CLOSE_CLIENT] {
            let analysis = (id == LOL_CLOSE_CLIENT).then(|| game(1.0));
            let error = f.apply(id, analysis.as_ref(), true, false).unwrap_err();
            assert!(error.contains("no está propuesto"), "{error}");
        }
        assert!(state.borrow().writes.is_empty() && state.borrow().history.is_empty());
    }

    #[test]
    fn blockers_stop_the_change() {
        let (mut f, state) = fixer(off());
        state.borrow_mut().blockers = vec!["Cierra el cliente de League".into()];
        let error = f
            .apply(LOL_CLOSE_CLIENT, Some(&game(16.0)), true, false)
            .unwrap_err();
        assert!(error.contains("Cierra el cliente de League"), "{error}");
        assert!(state.borrow().writes.is_empty());
        assert!(
            f.proposals(Some(&game(16.0))).unwrap()[0].blockers.len() == 1,
            "the proposal shows what blocks it"
        );
    }

    #[test]
    fn dry_run_and_a_missing_confirmation_change_nothing() {
        let (mut f, state) = fixer(off());
        let dry = f
            .apply(LOL_CLOSE_CLIENT, Some(&game(16.0)), true, true)
            .unwrap();
        assert!(dry.preview && !dry.needs_confirmation);
        let asked = f
            .apply(LOL_CLOSE_CLIENT, Some(&game(16.0)), false, false)
            .unwrap();
        assert!(asked.preview && asked.needs_confirmation);
        assert!(state.borrow().writes.is_empty() && state.borrow().history.is_empty());
        assert_eq!(state.borrow().value, off());
    }

    #[test]
    fn an_applied_fix_changes_the_setting_and_is_in_the_history_with_its_before_and_its_evidence() {
        let (mut f, state) = fixer(off());
        let report = f
            .apply(LOL_CLOSE_CLIENT, Some(&game(16.0)), true, false)
            .unwrap();
        assert_eq!((report.preview, report.seq), (false, Some(1)));
        assert_eq!(state.borrow().value, Some(SettingValue::Bool(true)));
        let history = state.borrow().history.clone();
        assert_eq!(history.len(), 1);
        assert_eq!(
            (
                history[0].before.clone(),
                history[0].after.clone(),
                history[0].state
            ),
            (
                SettingValue::Bool(false),
                SettingValue::Bool(true),
                HistoryState::Applied
            )
        );
        assert_eq!(history[0].session.as_deref(), Some("lol-1"));
    }

    #[test]
    fn a_write_that_does_not_stick_is_rolled_back_and_not_recorded() {
        let (mut f, state) = fixer(off());
        state.borrow_mut().does_not_stick = true;
        let error = f
            .apply(LOL_CLOSE_CLIENT, Some(&game(16.0)), true, false)
            .unwrap_err();
        assert!(
            error.contains("no se quedó") && error.contains("como estaba"),
            "{error}"
        );
        assert_eq!(
            state.borrow().writes.last(),
            Some(&SettingValue::Bool(false)),
            "the old value was written back"
        );
        assert!(state.borrow().history.is_empty());
    }

    #[test]
    fn a_change_that_cannot_be_recorded_is_put_back() {
        let (mut f, state) = fixer(off());
        state.borrow_mut().history_full = true;
        let error = f
            .apply(LOL_CLOSE_CLIENT, Some(&game(16.0)), true, false)
            .unwrap_err();
        assert!(
            error.contains("historial") && error.contains("como estaba"),
            "{error}"
        );
        assert_eq!(
            state.borrow().value,
            off(),
            "nothing is changed without a line that says so"
        );
    }

    #[test]
    fn undo_puts_the_value_back_and_says_so_in_the_history() {
        let (mut f, state) = fixer(off());
        f.apply(LOL_CLOSE_CLIENT, Some(&game(16.0)), true, false)
            .unwrap();
        let preview = f.undo(1, false).unwrap();
        assert!(preview.preview && preview.was_undo);
        assert_eq!(
            state.borrow().value,
            Some(SettingValue::Bool(true)),
            "a preview changes nothing"
        );
        let done = f.undo(1, true).unwrap();
        assert_eq!((done.preview, done.seq), (false, Some(2)));
        assert_eq!(state.borrow().value, off());
        let history = f.history().unwrap();
        assert_eq!(
            (history[1].state, history[1].undoes),
            (HistoryState::Undone, Some(1))
        );
        assert!(f.undo(1, true).unwrap_err().contains("ya está deshecho"));
    }

    #[test]
    fn undo_refuses_when_the_setting_was_changed_from_outside() {
        let (mut f, state) = fixer(off());
        f.apply(LOL_CLOSE_CLIENT, Some(&game(16.0)), true, false)
            .unwrap();
        state.borrow_mut().value = Some(SettingValue::Text("otra cosa".into()));
        let error = f.undo(1, true).unwrap_err();
        assert!(
            error.contains("fuera de Purifex") && error.contains("no lo toco"),
            "{error}"
        );
        assert_eq!(
            state.borrow().value,
            Some(SettingValue::Text("otra cosa".into()))
        );
        assert!(f
            .undo(99, true)
            .unwrap_err()
            .contains("no hay un cambio #99"));
    }

    #[test]
    fn a_setting_that_was_not_written_goes_back_to_its_default_when_undone() {
        let (mut f, state) = fixer(Some(SettingValue::Unset));
        f.apply(LOL_CLOSE_CLIENT, Some(&game(16.0)), true, false)
            .unwrap();
        f.undo(1, true).unwrap();
        assert_eq!(state.borrow().writes.last(), Some(&SettingValue::Unset));
    }

    #[test]
    fn a_fixer_without_backends_proposes_nothing_and_changes_nothing() {
        let mut f = Fixer::disabled();
        assert!(f.proposals(Some(&game(16.0))).unwrap().is_empty());
        assert!(f
            .apply(LOL_CLOSE_CLIENT, Some(&game(16.0)), true, false)
            .is_err());
        assert!(f.history().unwrap().is_empty());
    }
}
