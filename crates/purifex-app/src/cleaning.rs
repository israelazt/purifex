//! Uninstalling an app and cleaning what it leaves. Three steps, each its own
//! decision: start the app's own uninstaller; look for traces (a preview that
//! touches nothing); remove them to a quarantine that can give them back. The
//! rules of what may be removed are the domain's (`app_traces`); this only
//! orders the steps and refuses the ones that are not safe yet (cleaning an app
//! that is still installed).

use std::sync::Arc;

use purifex_domain::app_traces::{propose_guessing, propose_with, simple, Trace};
use purifex_domain::apps::InstalledApp;
use purifex_domain::cleanup_log::{
    is_purged, is_restored, next_seq, CleanReport, CleanupEntry, CleanupState,
};
use purifex_domain::date::Date;
use purifex_domain::forced::{forced_app, installed_there, is_path};
use purifex_domain::ports::{
    Backups, CleanupStore, Clock, Processes, RestorePoints, SeedProbe, TraceFinder, Uninstaller,
};
use purifex_domain::safeguard::RestorePointOutcome;
use purifex_domain::scan_mode::ScanMode;

mod backing_up;
mod closing;
mod forcing;
mod junk;
mod logging;
mod purging;
mod rebooting;
mod removing;
mod restoring;
mod searching;
mod uninstalling;
pub use removing::RemovalStart;
pub use searching::SearchJob;
pub use uninstalling::{Prepared, UninstallJob};
mod startup;

#[derive(Default)]
pub struct Cleaning {
    finder: Option<Arc<dyn TraceFinder>>,
    launcher: Option<Box<dyn Uninstaller>>,
    store: Option<Box<dyn CleanupStore>>,
    probe: Option<Arc<dyn SeedProbe>>,
    clock: Option<Box<dyn Clock>>,
    restore_points: Option<Arc<dyn RestorePoints>>,
    backups: Option<Box<dyn Backups>>,
    processes: Option<Box<dyn Processes>>,
    mode: ScanMode,
    /// While a batch runs: the restore point taken once for all of it.
    batch_point: Option<RestorePointOutcome>,
    /// The logs of installations: what they name for the app is evidence.
    logs: Option<Box<dyn purifex_domain::ports::InstallLogs>>,
    junk: Option<Box<dyn purifex_domain::ports::JunkFinder>>,
}

/// The app that `query` names (its id, its name, or a part of it that only one has).
pub fn find<'a>(apps: &'a [InstalledApp], query: &str) -> Result<&'a InstalledApp, String> {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return Err("falta el nombre de la app".into());
    }
    if let Some(app) = apps
        .iter()
        .find(|a| a.id.to_lowercase() == q || a.name.to_lowercase() == q)
    {
        return Ok(app);
    }
    let matching: Vec<&InstalledApp> = apps
        .iter()
        .filter(|a| a.name.to_lowercase().contains(&q))
        .collect();
    match matching.as_slice() {
        [one] => Ok(one),
        [] => Err(format!("no hay ninguna app que se llame «{query}»")),
        many => {
            let names: Vec<&str> = many.iter().take(5).map(|a| a.name.as_str()).collect();
            Err(format!(
                "«{query}» coincide con varias apps ({}…): escribe el nombre completo",
                names.join(", ")
            ))
        }
    }
}

/// What removing a selection of traces did: the entries logged, and what could not be
/// removed with why.
#[derive(Default)]
pub struct Removals {
    pub removed: Vec<CleanupEntry>,
    pub failed: Vec<(Trace, String)>,
    /// The ones that failed because a program has them open.
    pub in_use: Vec<Trace>,
}

/// With a selection only those places go, and only if the rules still propose them: what
/// is asked for is always checked against what `propose` returned.
fn select(proposed: Vec<Trace>, only: Option<&[String]>) -> Vec<Trace> {
    proposed
        .into_iter()
        .filter(|t| only.is_none_or(|keep| keep.iter().any(|k| k.eq_ignore_ascii_case(&t.key()))))
        .collect()
}

impl Cleaning {
    pub fn new(
        finder: Box<dyn TraceFinder>,
        launcher: Box<dyn Uninstaller>,
        store: Box<dyn CleanupStore>,
    ) -> Self {
        Self {
            finder: Some(Arc::from(finder)),
            launcher: Some(launcher),
            store: Some(store),
            probe: None,
            clock: None,
            restore_points: None,
            backups: None,
            processes: None,
            mode: ScanMode::default(),
            batch_point: None,
            logs: None,
            junk: None,
        }
    }

    /// How much of what the search finds is proposed (and so removed).
    pub fn set_mode(&mut self, mode: ScanMode) {
        self.mode = mode;
    }

    /// What the rules propose for the app, kept to what the mode allows.
    fn propose(
        &self,
        app: &InstalledApp,
        others: &[InstalledApp],
        found: &purifex_domain::ports::Found,
    ) -> Vec<Trace> {
        let candidates = self.with_logged(app, &found.candidates);
        let judging = if self.mode == ScanMode::Advanced {
            propose_guessing
        } else {
            propose_with
        };
        let mut traces = judging(app, others, &found.seeds, &candidates);
        traces.retain(|t| self.mode.allows(t.evidence));
        traces
    }

    /// A restore point of Windows is asked for before each uninstaller starts.
    pub fn with_restore_points(mut self, points: Box<dyn RestorePoints>) -> Self {
        self.restore_points = Some(Arc::from(points));
        self
    }

    /// One restore point for a batch of `count` apps, taken now; the uninstallers that follow
    /// do not ask again until `end_batch`. Returns what Windows said.
    pub fn begin_batch(&mut self, count: usize) -> RestorePointOutcome {
        let outcome = uninstalling::asked(
            self.restore_points.as_deref(),
            &format!("Purifex: antes de desinstalar {count} apps"),
        );
        self.batch_point = Some(outcome.clone());
        outcome
    }

    pub fn end_batch(&mut self) {
        self.batch_point = None;
    }

    /// The day is written on each removal, so the quarantine can let go of old ones.
    pub fn with_clock(mut self, clock: Box<dyn Clock>) -> Self {
        self.clock = Some(clock);
        self
    }

    fn today(&self) -> Option<Date> {
        self.clock.as_ref().map(|c| c.today())
    }

    /// What is observed of an app before its uninstaller starts (kept with it).
    pub fn with_probe(mut self, probe: Box<dyn SeedProbe>) -> Self {
        self.probe = Some(Arc::from(probe));
        self
    }

    fn store(&mut self) -> Result<&mut Box<dyn CleanupStore>, String> {
        self.store
            .as_mut()
            .ok_or_else(|| "esta versión no puede limpiar restos".to_string())
    }

    /// Keeps what is known of an app that has just disappeared from the list, so its
    /// traces can be looked for (the system no longer knows it).
    pub fn remember(&mut self, app: &InstalledApp) -> Result<(), String> {
        self.store()?.remember(app)
    }

    /// The app to clean: one still installed (`true`); or one that is gone, either
    /// remembered (its uninstaller was started here, or it was seen to disappear) or
    /// known only by the name the user gives, which is then searched strictly.
    fn resolve(
        &self,
        apps: &[InstalledApp],
        all: &[InstalledApp],
        query: &str,
    ) -> Result<(InstalledApp, bool), String> {
        if let Ok(app) = find(apps, query) {
            return Ok((app.clone(), true));
        }
        // A name that fits something still installed (even if several do) is never
        // taken for an uninstalled app: its folders are in use.
        let wanted = simple(query);
        let overlaps = |name: &str| {
            let n = simple(name);
            !n.is_empty()
                && !wanted.is_empty()
                && (n.contains(&wanted) || (n.len() >= 4 && wanted.contains(&n)))
        };
        if !is_path(query) && all.iter().any(|a| overlaps(&a.name)) {
            return Err(format!(
                "«{query}» coincide con una app que sigue instalada: escribe su nombre completo"
            ));
        }
        let q = query.trim().to_lowercase();
        let store = self
            .store
            .as_ref()
            .ok_or("esta versión no puede limpiar restos")?;
        let remembered = store.remembered()?.into_iter().rev().find(|a| {
            a.name.to_lowercase() == q
                || a.id.to_lowercase() == q
                || a.name.to_lowercase().contains(&q)
        });
        if let Some(app) = remembered {
            return Ok((app, false));
        }
        // Never seen: only what the person wrote is known (a name, a folder or a `.exe`).
        // The rules of the domain still apply: what is too short, too common or too broad
        // to be safe is refused, and so is a folder that an installed app uses.
        let unknown = forced_app(query)?;
        if let Some(other) = installed_there(&unknown, all) {
            return Err(format!(
                "esa carpeta es de {}, que sigue instalada: sus archivos están en uso",
                other.name
            ));
        }
        Ok((unknown, false))
    }

    /// Looks for the traces; with `confirmed` and the app gone, removes them to the
    /// quarantine. `apps` are the ones listed to the user (to name one); `all` is
    /// everything installed, which is what must be left alone.
    pub fn clean(
        &mut self,
        apps: &[InstalledApp],
        all: &[InstalledApp],
        query: &str,
        confirmed: bool,
        only: Option<&[String]>,
    ) -> Result<CleanReport, String> {
        let job = self.prepare_search(apps, all, query)?;
        let found = job.run(&|_| {})?;
        let preview = self.finish_search(&job, &found);
        if !confirmed {
            return Ok(preview);
        }
        if job.installed {
            return Err(format!("{} sigue instalada: termina su desinstalador primero (los restos de una app en uso no se tocan)", job.app.name));
        }
        let chosen = select(preview.proposed, only);
        let done = self.remove_all(&job.app.name, &chosen)?;
        Ok(CleanReport {
            app: job.app.name,
            removed: done.removed,
            failed: done.failed,
            in_use: done.in_use,
            incomplete: preview.incomplete,
            ..Default::default()
        })
    }

    /// Moves each trace to the quarantine and logs it. A trace that cannot be removed
    /// (in use, needs an administrator) is reported with why, and the rest go on.
    fn remove_all(&mut self, app: &str, traces: &[Trace]) -> Result<Removals, String> {
        let mut start = RemovalStart {
            log: self.store()?.entries()?,
            today: self.today(),
        };
        let mut done = Removals::default();
        for trace in traces {
            self.remove_one(app, trace, &mut start, &mut done)?;
        }
        Ok(done)
    }

    /// Puts back what removal `seq` took out.
    pub fn restore(&mut self, seq: u64) -> Result<CleanupEntry, String> {
        let today = self.today();
        let store = self.store()?;
        let log = store.entries()?;
        let entry = log
            .iter()
            .find(|e| e.seq == seq && e.state == CleanupState::Removed)
            .ok_or_else(|| format!("no hay ninguna eliminación #{seq}"))?
            .clone();
        if is_restored(&log, seq) {
            return Err(format!("la eliminación #{seq} ya se devolvió"));
        }
        if is_purged(&log, seq) {
            return Err(format!(
                "la copia de la eliminación #{seq} ya se borró de la cuarentena: no se puede devolver"
            ));
        }
        store.restore(&entry)?;
        let line = CleanupEntry {
            seq: next_seq(&log),
            app: entry.app.clone(),
            trace: entry.trace.clone(),
            kept: entry.kept,
            state: CleanupState::Restored,
            settles: Some(seq),
            on: today,
        };
        store.append(&line)?;
        Ok(line)
    }

    pub fn history(&self) -> Result<Vec<CleanupEntry>, String> {
        self.store.as_ref().map_or(Ok(Vec::new()), |s| s.entries())
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use super::*;
    use crate::testing::{CleanLog, FakeCleaning, FakeFinder};
    use purifex_domain::app_traces::{Candidate, TraceKind};

    fn app(name: &str, publisher: &str) -> InstalledApp {
        InstalledApp {
            id: format!("HKLM64:{name}"),
            name: name.into(),
            publisher: Some(publisher.into()),
            version: Some("1.0".into()),
            uninstall: Some("u.exe".into()),
            ..Default::default()
        }
    }

    fn folder(location: &str) -> Candidate {
        Candidate {
            kind: TraceKind::Folder,
            location: location.into(),
            bytes: 100,
            ..Default::default()
        }
    }

    fn cleaning(found: Vec<Candidate>) -> (Cleaning, Rc<RefCell<CleanLog>>) {
        let log = Rc::new(RefCell::new(CleanLog::default()));
        let c = Cleaning::new(
            Box::new(FakeFinder(found)),
            Box::new(FakeCleaning(log.clone())),
            Box::new(FakeCleaning(log.clone())),
        );
        (c, log)
    }

    const DATA: &str = r"C:\Users\u\AppData\Roaming\Discord";
    const LOCAL: &str = r"C:\Users\u\AppData\Local\Discord";

    #[test]
    fn an_app_is_found_by_name_by_a_part_only_it_has_and_never_by_a_guess() {
        let apps = vec![app("Discord", "D"), app("Git", "G"), app("Git LFS", "G")];
        assert_eq!(find(&apps, "discord").unwrap().name, "Discord");
        assert_eq!(find(&apps, "cord").unwrap().name, "Discord");
        assert_eq!(
            find(&apps, "Git").unwrap().name,
            "Git",
            "the exact name wins over a longer one"
        );
        assert!(find(&apps, "gi").unwrap_err().contains("varias apps"));
        assert!(find(&apps, "nada").unwrap_err().contains("no hay ninguna"));
        assert!(find(&apps, " ").is_err());
    }

    struct FakeProbe;

    impl SeedProbe for FakeProbe {
        fn observe(&self, app: &InstalledApp) -> purifex_domain::seeds::Seeds {
            purifex_domain::seeds::Seeds::new(
                Some(r"C:\Program Files\Discord"),
                &[format!("{}.exe", app.name.to_lowercase())],
                &[".dsc".into()],
            )
        }
    }

    #[test]
    fn what_is_observed_of_the_app_is_remembered_before_its_uninstaller_starts() {
        let (c, log) = cleaning(vec![]);
        let mut c = c.with_probe(Box::new(FakeProbe));
        c.uninstall(&[app("Discord", "Discord Inc.")], "Discord")
            .unwrap();
        let kept = log.borrow().remembered[0].observed.clone();
        assert_eq!(kept.exes, ["discord.exe"]);
        assert_eq!(kept.extensions, [".dsc"]);
        assert_eq!(kept.folder.as_deref(), Some(r"C:\Program Files\Discord"));
        // Without a probe nothing is observed and uninstalling still works.
        let (mut plain, log) = cleaning(vec![]);
        plain
            .uninstall(&[app("Discord", "Discord Inc.")], "Discord")
            .unwrap();
        assert!(log.borrow().remembered[0].observed.is_empty());
    }

    #[test]
    fn uninstalling_starts_the_official_uninstaller_and_a_protected_app_is_refused() {
        let (mut c, log) = cleaning(vec![]);
        let apps = vec![
            app("Discord", "Discord Inc."),
            app(
                "Microsoft Visual C++ 2015 Redistributable",
                "Microsoft Corporation",
            ),
        ];
        assert_eq!(c.uninstall(&apps, "Discord").unwrap().app.name, "Discord");
        assert_eq!(log.borrow().launched, ["Discord"]);
        assert_eq!(
            log.borrow().remembered.len(),
            1,
            "remembered for the cleaning"
        );
        let err = c.uninstall(&apps, "Visual C++").unwrap_err();
        assert!(err.contains("no se desinstala desde aquí"));
        assert_eq!(
            log.borrow().launched.len(),
            1,
            "the protected one was not started"
        );
    }

    #[test]
    fn the_log_of_an_installation_names_what_the_app_made_even_without_its_name() {
        use crate::testing::FakeLogs;
        use purifex_domain::app_traces::Evidence;
        use purifex_domain::monitor::{InstallLog, Mark};
        let mark = |kind, location: &str| Mark {
            kind,
            location: location.into(),
            value: None,
            stamp: "1".into(),
            bytes: 50,
        };
        let log = |id: &str, app: &str, own: bool| InstallLog {
            id: id.into(),
            installer: "setup.exe".into(),
            app: Some(app.into()),
            on: None,
            own,
            marks: vec![
                mark(TraceKind::Folder, r"C:\ProgramData\Vendor Stuff 9"),
                mark(TraceKind::Folder, r"C:\Windows\Fonts\zed"),
                mark(TraceKind::RegistryKey, r"HKLM\SOFTWARE\Microsoft\Zed"),
            ],
        };
        let logs = FakeLogs::default();
        logs.0.borrow_mut().push(log("1", "Discord", true));
        logs.0.borrow_mut().push(log("2", "Another App", true));
        let (c, _) = cleaning(vec![folder(r"C:\ProgramData\Vendor Stuff 9")]);
        let mut c = c.with_logs(Box::new(logs.clone()));
        let apps = vec![app("Discord", "Discord Inc.")];
        let report = c.clean(&apps, &apps, "Discord", false, None).unwrap();
        let places: Vec<_> = report
            .proposed
            .iter()
            .map(|t| (t.location.as_str(), t.evidence))
            .collect();
        // The folder shares no name with the app, yet the log names it; Windows' places never go.
        assert_eq!(
            places,
            [(r"C:\ProgramData\Vendor Stuff 9", Evidence::Monitored)]
        );
        // A log brought from elsewhere only guesses: Moderate leaves it out.
        logs.0.borrow_mut().clear();
        logs.0.borrow_mut().push(log("3", "discord", false));
        let report = c.clean(&apps, &apps, "Discord", false, None).unwrap();
        assert!(report.proposed.is_empty(), "{:?}", report.proposed);
        c.set_mode(ScanMode::Advanced);
        let report = c.clean(&apps, &apps, "Discord", false, None).unwrap();
        assert_eq!(report.proposed[0].evidence, Evidence::ImportedLog);
        assert!(report.proposed[0].evidence.is_guess());
    }

    #[test]
    fn a_batch_asks_for_one_restore_point_and_each_uninstaller_reuses_it() {
        use crate::testing::FakeRestorePoints;
        let (c, _) = cleaning(vec![]);
        let points = FakeRestorePoints::default();
        let mut c = c.with_restore_points(Box::new(points.clone()));
        let apps = vec![app("Discord", "Discord Inc."), app("Git", "Git Inc.")];
        assert_eq!(c.begin_batch(2), RestorePointOutcome::Requested);
        for name in ["Discord", "Git"] {
            let started = c.uninstall(&apps, name).unwrap();
            assert_eq!(started.restore_point, RestorePointOutcome::Requested);
        }
        assert_eq!(points.asked.lock().unwrap().len(), 1, "asked once for both");
        assert_eq!(
            points.asked.lock().unwrap()[0],
            "Purifex: antes de desinstalar 2 apps"
        );
        // After the batch each uninstaller asks for its own again.
        c.end_batch();
        c.uninstall(&apps, "Git").unwrap();
        assert_eq!(points.asked.lock().unwrap().len(), 2);
    }

    #[test]
    fn a_restore_point_is_asked_for_before_the_uninstaller_and_a_failure_does_not_stop_it() {
        use crate::testing::FakeRestorePoints;
        let (c, log) = cleaning(vec![]);
        let points = FakeRestorePoints::default();
        let mut c = c.with_restore_points(Box::new(points.clone()));
        let apps = vec![app("Discord", "Discord Inc.")];
        let started = c.uninstall(&apps, "Discord").unwrap();
        assert_eq!(started.restore_point, RestorePointOutcome::Requested);
        assert_eq!(
            points.asked.lock().unwrap()[0],
            "Purifex: antes de desinstalar Discord"
        );
        // Without administrator the point is not made, and the uninstaller starts all the same.
        let (c, log2) = cleaning(vec![]);
        let denied = FakeRestorePoints {
            fails: Some("hace falta ser administrador".into()),
            ..Default::default()
        };
        let mut c = c.with_restore_points(Box::new(denied));
        let started = c.uninstall(&apps, "Discord").unwrap();
        assert!(matches!(
            started.restore_point,
            RestorePointOutcome::Unavailable(_)
        ));
        assert_eq!(log2.borrow().launched, ["Discord"]);
        assert_eq!(log.borrow().launched, ["Discord"]);
    }

    #[test]
    fn the_registration_is_copied_before_the_uninstaller_and_a_failed_copy_does_not_stop_it() {
        use crate::testing::FakeBackups;
        let apps = vec![app("Discord", "Discord Inc.")];
        let (c, _) = cleaning(vec![]);
        let backup = FakeBackups::default();
        let mut c = c.with_backups(Box::new(backup.clone()));
        let started = c.uninstall(&apps, "Discord").unwrap();
        assert_eq!(backup.saved.borrow()[0], "Discord");
        assert!(started.safeguards_text().contains(r"C:\copias\Discord.reg"));
        let (c, log) = cleaning(vec![]);
        let broken = FakeBackups {
            fails: true,
            ..Default::default()
        };
        let mut c = c.with_backups(Box::new(broken));
        let started = c.uninstall(&apps, "Discord").unwrap();
        assert_eq!(started.registry_copy, None);
        assert_eq!(log.borrow().launched, ["Discord"]);
    }

    #[test]
    fn each_copy_is_listed_newest_first_and_only_a_listed_copy_is_registered_again() {
        use crate::testing::FakeBackups;
        let apps = vec![app("Discord", "Discord Inc."), app("OBS Studio", "OBS")];
        let (c, _) = cleaning(vec![]);
        let backup = FakeBackups::default();
        let mut c = c.with_backups(Box::new(backup.clone()));
        c.uninstall(&apps, "Discord").unwrap();
        c.uninstall(&apps, "OBS Studio").unwrap();
        let listed = c.backups().unwrap();
        assert_eq!(listed[0].app, "OBS Studio");
        assert_eq!(listed[1].app, "Discord");
        let entry = c.restore_backup(&listed[1].registry_copy).unwrap();
        assert_eq!(entry.app, "Discord");
        assert_eq!(backup.restored.borrow().as_slice(), ["Discord"]);
        let error = c.restore_backup(r"C:\otro\evil.reg").unwrap_err();
        assert!(error.contains("no está en la lista"));
    }

    #[test]
    fn the_traces_can_be_previewed_while_installed_but_never_removed() {
        let (mut c, log) = cleaning(vec![folder(DATA)]);
        let apps = vec![app("Discord", "Discord Inc.")];
        let preview = c.clean(&apps, &apps, "Discord", false, None).unwrap();
        assert!(preview.preview && preview.proposed.len() == 1);
        let err = c.clean(&apps, &apps, "Discord", true, None).unwrap_err();
        assert!(err.contains("sigue instalada"));
        assert!(log.borrow().removed.is_empty(), "nothing touched");
    }

    #[test]
    fn after_the_uninstall_the_traces_are_removed_to_the_quarantine_and_logged() {
        let (mut c, log) = cleaning(vec![folder(DATA), folder(LOCAL)]);
        let mut apps = vec![app("Discord", "Discord Inc."), app("Git", "G")];
        c.uninstall(&apps, "Discord").unwrap();
        apps.remove(0); // the uninstaller finished: the system no longer lists it
        let report = c.clean(&apps, &apps, "Discord", true, None).unwrap();
        assert_eq!(report.removed.len(), 2);
        assert!(report.removed[0].seq < report.removed[1].seq);
        assert_eq!(log.borrow().removed.len(), 2);
        assert_eq!(log.borrow().entries.len(), 2);
        assert!(report.to_text().contains("purifex apps restore"));
    }

    #[test]
    fn what_the_rules_refuse_is_never_removed_even_if_the_finder_proposes_it() {
        let (mut c, log) = cleaning(vec![
            folder(r"C:\Windows\System32\Discord"),
            folder(r"C:\Program Files"),
            folder(r"C:\Users\u\AppData\Roaming\Slack"),
            folder(DATA),
        ]);
        let mut apps = vec![app("Discord", "Discord Inc.")];
        c.uninstall(&apps, "Discord").unwrap();
        apps.clear();
        let report = c.clean(&apps, &apps, "Discord", true, None).unwrap();
        assert_eq!(log.borrow().removed, [DATA]);
        assert_eq!(report.removed.len(), 1);
    }

    #[test]
    fn in_safe_mode_what_is_known_only_by_its_name_is_neither_shown_nor_removed() {
        let (mut c, log) = cleaning(vec![folder(DATA)]);
        let apps = vec![app("Discord", "D")];
        c.uninstall(&apps, "Discord").unwrap();
        let none: Vec<InstalledApp> = Vec::new();
        c.set_mode(ScanMode::Safe);
        let preview = c.clean(&none, &none, "Discord", false, None).unwrap();
        assert!(
            preview.proposed.is_empty(),
            "the folder is only named like the app"
        );
        let done = c.clean(&none, &none, "Discord", true, None).unwrap();
        assert!(done.removed.is_empty() && log.borrow().removed.is_empty());
        c.set_mode(ScanMode::Moderate);
        let done = c.clean(&none, &none, "Discord", true, None).unwrap();
        assert_eq!(done.removed.len(), 1);
    }

    #[test]
    fn one_that_cannot_be_removed_is_reported_and_the_others_still_go() {
        let (mut c, log) = cleaning(vec![folder(DATA), folder(LOCAL)]);
        log.borrow_mut().stuck.push(LOCAL.into());
        let mut apps = vec![app("Discord", "D")];
        c.uninstall(&apps, "Discord").unwrap();
        apps.clear();
        let report = c.clean(&apps, &apps, "Discord", true, None).unwrap();
        assert_eq!(report.removed.len(), 1);
        assert_eq!(report.failed.len(), 1);
        assert_eq!(
            report.in_use.len(),
            1,
            "it is open elsewhere: it can wait for a restart"
        );
        assert_eq!(report.in_use[0].location, LOCAL);
        let text = report.to_text();
        assert!(text.contains("no se pudo quitar") && text.contains("al reiniciar"));
    }

    #[test]
    fn an_app_that_is_gone_can_be_cleaned_by_its_name_even_if_it_was_not_uninstalled_from_here() {
        let (mut c, log) = cleaning(vec![folder(DATA)]);
        // Uninstalled by the user outside Purifex: nothing remembered, only the name.
        let preview = c.clean(&[], &[], "Discord", false, None).unwrap();
        assert_eq!(preview.proposed.len(), 1);
        let done = c.clean(&[], &[], "Discord", true, None).unwrap();
        assert_eq!(done.removed.len(), 1);
        assert_eq!(log.borrow().removed, [DATA]);
    }

    #[test]
    fn a_broken_app_can_be_forced_by_its_folder_but_never_a_broad_one_or_one_in_use() {
        const OLD: &str = r"D:\Games\Discord";
        let (mut c, log) = cleaning(vec![folder(OLD), folder(DATA)]);
        let preview = c.clean(&[], &[], OLD, false, None).unwrap();
        assert_eq!(preview.app, "Discord");
        assert!(!preview.still_installed);
        // A folder an installed app uses is refused, whichever way it is written.
        let kept = vec![app("Other Tool", "O")];
        let mut in_use = kept[0].clone();
        in_use.install_location = Some(OLD.into());
        let all = vec![in_use];
        let err = c
            .clean(&[], &all, r"D:\Games\Discord\bin\x.exe", true, None)
            .unwrap_err();
        assert!(err.contains("sigue instalada"), "{err}");
        // Broad places are refused before anything is looked at or removed.
        for broad in [r"C:\Program Files", r"C:\Windows\System32", r"C:\"] {
            assert!(c.clean(&[], &kept, broad, true, None).is_err(), "{broad}");
        }
        assert!(log.borrow().removed.is_empty());
    }

    #[test]
    fn a_forced_uninstall_takes_out_the_entry_without_the_uninstaller_and_refuses_what_it_must() {
        const KEY: &str =
            r"HKEY_LOCAL_MACHINE\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\Old Tool";
        let (mut c, log) = cleaning(vec![]);
        let apps = vec![app("Old Tool", "Acme")];
        let started = c.force_uninstall(&apps, "Old Tool").unwrap();
        assert_eq!(started.app.name, "Old Tool");
        assert!(
            log.borrow().launched.is_empty(),
            "its own uninstaller is never started"
        );
        assert_eq!(log.borrow().removed, [KEY]);
        assert_eq!(
            c.history().unwrap().len(),
            1,
            "and it is in the log, so it can be put back"
        );
        // What Windows needs, and what has no entry of its own, is refused before any change.
        let (mut c, log) = cleaning(vec![]);
        let runtime = vec![app("Microsoft Visual C++ 2015", "Microsoft Corporation")];
        let err = c.force_uninstall(&runtime, "Visual C++").unwrap_err();
        assert!(err.contains("no se quita"), "{err}");
        let store = vec![InstalledApp {
            id: "Cool_1_x64__abc".into(),
            name: "Cool Store App".into(),
            kind: purifex_domain::apps::AppKind::Store,
            ..Default::default()
        }];
        let err = c.force_uninstall(&store, "Cool").unwrap_err();
        assert!(err.contains("entrada propia"), "{err}");
        assert!(log.borrow().removed.is_empty());
        // An entry that cannot be taken out (no permission) is an error, not a half job.
        let (mut c, log) = cleaning(vec![]);
        log.borrow_mut().stuck.push(KEY.into());
        let err = c.force_uninstall(&apps, "Old Tool").unwrap_err();
        assert!(err.contains("no se pudo quitar su entrada"), "{err}");
    }

    /// A finder that also says which seeds it used (extensions it found in the registry).
    struct SeedingFinder(Vec<Candidate>, purifex_domain::seeds::Seeds);

    impl TraceFinder for SeedingFinder {
        fn candidates(&self, _: &InstalledApp) -> Result<Vec<Candidate>, String> {
            Ok(self.0.clone())
        }
        fn search(&self, _: &InstalledApp) -> Result<purifex_domain::ports::Found, String> {
            Ok(purifex_domain::ports::Found {
                seeds: self.1.clone(),
                candidates: self.0.clone(),
                ..Default::default()
            })
        }
    }

    #[test]
    fn the_rules_judge_with_the_seeds_the_finder_used_and_the_default_is_the_apps_own() {
        let key = Candidate {
            kind: TraceKind::RegistryKey,
            location: r"HKCU\Software\Microsoft\Windows\CurrentVersion\Explorer\FileExts\.dsc"
                .into(),
            ..Default::default()
        };
        let seeds = purifex_domain::seeds::Seeds::new(None, &[], &[".dsc".into()]);
        let log = Rc::new(RefCell::new(CleanLog::default()));
        let mut with = Cleaning::new(
            Box::new(SeedingFinder(vec![key.clone()], seeds)),
            Box::new(FakeCleaning(log.clone())),
            Box::new(FakeCleaning(log)),
        );
        let report = with.clean(&[], &[], "Discord", false, None).unwrap();
        assert_eq!(report.proposed.len(), 1, "{:?}", report.proposed);
        assert_eq!(
            report.proposed[0].evidence,
            purifex_domain::app_traces::Evidence::AssociatedExtension
        );
        // A finder that does not say gets the app's own seeds: the same key is refused.
        let (mut plain, _) = cleaning(vec![key]);
        let report = plain.clean(&[], &[], "Discord", false, None).unwrap();
        assert!(report.proposed.is_empty());
    }

    #[test]
    fn a_name_that_fits_something_still_installed_is_never_cleaned_and_generic_names_are_refused() {
        let (mut c, log) = cleaning(vec![folder(DATA), folder(r"C:\Program Files\Git")]);
        let installed = vec![app("Git", "G"), app("Git LFS", "G")];
        // "gi" is ambiguous between two installed apps: not taken for an uninstalled one.
        let err = c
            .clean(&installed, &installed, "gi", true, None)
            .unwrap_err();
        assert!(err.contains("sigue instalada"), "{err}");
        // The name of an installed app, in other words, is just as installed.
        let err = c
            .clean(&[], &installed, "git lfs tool", true, None)
            .unwrap_err();
        assert!(err.contains("sigue instalada"), "{err}");
        // Short or broad names cannot be searched safely.
        for generic in ["Go", "Microsoft", "Windows", "  "] {
            assert!(c.clean(&[], &[], generic, true, None).is_err(), "{generic}");
        }
        assert!(log.borrow().removed.is_empty(), "nothing was touched");
    }

    #[test]
    fn a_removal_can_be_put_back_once_and_the_log_says_so() {
        let (mut c, log) = cleaning(vec![folder(DATA)]);
        let mut apps = vec![app("Discord", "D")];
        c.uninstall(&apps, "Discord").unwrap();
        apps.clear();
        let seq = c
            .clean(&apps, &apps, "Discord", true, None)
            .unwrap()
            .removed[0]
            .seq;
        let line = c.restore(seq).unwrap();
        assert_eq!(line.settles, Some(seq));
        assert_eq!(log.borrow().restored, [DATA]);
        assert!(c.restore(seq).unwrap_err().contains("ya se devolvió"));
        assert!(c.restore(99).unwrap_err().contains("no hay ninguna"));
        assert_eq!(c.history().unwrap().len(), 2);
    }

    #[test]
    fn one_value_of_a_key_can_be_removed_without_its_neighbour() {
        let folder_seed = r"C:\Program Files\Discord";
        let value = |name: &str| Candidate {
            kind: TraceKind::RegistryValue,
            location:
                r"HKCU\Software\Classes\Local Settings\Software\Microsoft\Windows\Shell\MuiCache"
                    .into(),
            value: Some(format!(r"{folder_seed}\{name}.exe.FriendlyAppName")),
            ..Default::default()
        };
        let seeds = purifex_domain::seeds::Seeds::new(Some(folder_seed), &[], &[]);
        let log = Rc::new(RefCell::new(CleanLog::default()));
        let mut c = Cleaning::new(
            Box::new(SeedingFinder(vec![value("a"), value("b")], seeds)),
            Box::new(FakeCleaning(log.clone())),
            Box::new(FakeCleaning(log)),
        );
        let preview = c.clean(&[], &[], "Discord", false, None).unwrap();
        assert_eq!(preview.proposed.len(), 2, "{:?}", preview.proposed);
        let only = [preview.proposed[0].key()];
        let done = c.clean(&[], &[], "Discord", true, Some(&only)).unwrap();
        assert_eq!(done.removed.len(), 1, "only the chosen value goes");
        assert_eq!(done.removed[0].trace.key(), only[0]);
    }
}
