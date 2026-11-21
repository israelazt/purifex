//! Reading what the page being shown is missing. Each page has its own small reader;
//! a failure is collected (with what was being read) and the rest is still read.

use purifex_domain::driver_offers::Offers;
use purifex_domain::log_warn;
use purifex_domain::quarantine::{QuarantineInfo, QUARANTINE_DAYS};
use purifex_domain::trace_text::Counts;
use purifex_domain::view::{Page, Running};

use super::Core;

/// The result of a reading, or `None` after noting the failure in `errors`.
fn read<T>(result: Result<T, String>, what: &str, errors: &mut Vec<String>) -> Option<T> {
    result
        .map_err(|error| errors.push(format!("{what}: {error}")))
        .ok()
}

impl Core {
    /// Reads everything the shown page lacks; returns the failures, worded for the person.
    pub(super) fn read_missing(&mut self) -> Vec<String> {
        let mut errors = Vec::new();
        self.read_overview(&mut errors);
        self.read_sessions(&mut errors);
        match self.model.page {
            Page::Drivers => self.read_drivers_page(&mut errors),
            Page::Apps => self.read_apps_page(&mut errors),
            Page::Fixes => self.read_fixes_page(&mut errors),
            Page::Sessions => self.read_analysis(&mut errors),
            Page::Overview => {}
        }
        errors
    }

    fn read_overview(&mut self, errors: &mut Vec<String>) {
        if self.model.overview.is_none() {
            self.model.overview = read(self.services.overview(), "estado general", errors);
        }
    }

    fn read_sessions(&mut self, errors: &mut Vec<String>) {
        if self.model.sessions.is_none() {
            self.model.sessions = read(self.services.sessions(), "grabaciones", errors);
        }
    }

    fn read_drivers_page(&mut self, errors: &mut Vec<String>) {
        if self.model.drivers.is_none() {
            self.model.drivers = read(self.services.drivers(), "drivers", errors);
        }
        if self.model.candidates.is_none() {
            // A failure is shown once: an empty list stops it being asked on every visit.
            let list = read(self.services.candidates(), "propuestas", errors);
            self.model.candidates = Some(list.unwrap_or_default());
        }
    }

    fn read_apps_page(&mut self, errors: &mut Vec<String>) {
        let refreshing = std::mem::take(&mut self.refreshing_apps);
        if self.model.apps.is_none() || refreshing {
            // A failed reading keeps the list that is on the screen.
            if let Some(apps) = read(self.services.apps(), "apps", errors) {
                let old = self.model.apps.replace(apps);
                if let Some(gone) = old.as_deref().and_then(|o| self.vanished(o)) {
                    self.model.finding = Some(gone);
                }
            }
        }
        if let Some(name) = self.model.finding.take() {
            self.find_traces_of(&name, errors);
        }
        self.read_quarantine();
        if self.model.backups.is_none() {
            self.model.backups = Some(self.services.backups().unwrap_or_else(|error| {
                log_warn!("the copies could not be read: {error}");
                Vec::new()
            }));
        }
        if self.model.elevated.is_none() {
            self.model.elevated = Some(self.services.is_elevated());
        }
    }

    /// What the quarantine holds, after letting go of what has been there for seven days
    /// (once per run; the person is told). A quarantine that cannot be read is not worth an
    /// error on the page: it shows as empty and the reason goes to the log.
    fn read_quarantine(&mut self) {
        if self.model.quarantine.is_some() {
            return;
        }
        if !std::mem::replace(&mut self.expired_checked, true) {
            match self.services.expire_quarantine() {
                Ok(report) if report.purged > 0 => {
                    self.model.message = Some(format!(
                        "Se soltaron de la cuarentena los restos que llevaban {QUARANTINE_DAYS} días: {}",
                        report.to_text()
                    ));
                }
                Ok(_) => {}
                Err(error) => log_warn!("the quarantine could not be tidied: {error}"),
            }
        }
        self.model.quarantine = Some(self.services.quarantine().unwrap_or_else(|error| {
            log_warn!("the quarantine could not be read: {error}");
            QuarantineInfo::default()
        }));
    }

    /// Looks for the traces of an app. When some are found the window with the list
    /// opens by itself, with everything checked.
    fn find_traces_of(&mut self, name: &str, errors: &mut Vec<String>) {
        let Some(report) = read(self.services.clean_app(name, false, None), "restos", errors)
        else {
            return;
        };
        self.model.running = self.running_programs_of(name, report.still_installed);
        self.model.traces_open = !report.proposed.is_empty();
        self.model.unchecked = Vec::new();
        let counts = Counts::of(&report.proposed).to_text();
        self.model.message = Some(
            match (report.proposed.is_empty(), self.model.message.take()) {
                (true, _) => format!("{name}: no se encontraron restos."),
                (false, Some(said)) => format!("{said} {counts}"),
                (false, None) => format!("{name}: {counts}"),
            },
        );
        self.model.traces = Some(report);
    }

    /// The programs of an app that is gone that still run from its folder (none for an app
    /// that is installed: its programs are not ours to close). Not being able to tell is
    /// not an error worth stopping for: it is logged and nothing is offered.
    fn running_programs_of(&mut self, name: &str, installed: bool) -> Option<Running> {
        if installed {
            return None;
        }
        let found = self
            .services
            .running_programs(name)
            .inspect_err(|e| log_warn!("the running programs could not be read: {e}"))
            .ok()?;
        let mut names: Vec<String> = found.iter().map(|p| p.name().to_string()).collect();
        names.dedup_by(|a, b| a.eq_ignore_ascii_case(b));
        (!names.is_empty()).then(|| Running {
            app: name.to_string(),
            names,
        })
    }

    fn read_fixes_page(&mut self, errors: &mut Vec<String>) {
        if self.model.fixes.is_none() {
            self.model.fixes = read(self.services.fixes(), "arreglos", errors);
        }
        if self.model.history.is_none() {
            self.model.history = read(self.services.history(), "historial", errors);
        }
    }

    /// The analysis of the chosen recording (with nothing chosen, the newest that can be
    /// read), and its comparison with the game before it.
    fn read_analysis(&mut self, errors: &mut Vec<String>) {
        if self.model.analysis.is_some() {
            return;
        }
        let Some(id) = self.chosen_session() else {
            return;
        };
        if let Some(analysis) = read(self.services.analyze(Some(&id)), "partida", errors) {
            self.model.selected = Some(id.clone());
            self.model.analysis = Some(analysis);
            self.model.comparison = self.compare_with_previous(&id);
        }
    }

    fn chosen_session(&self) -> Option<String> {
        self.model.selected.clone().or_else(|| {
            let rows = self.model.sessions.as_ref()?;
            rows.iter()
                .find(|r| r.problem.is_none())
                .map(|r| r.id.clone())
        })
    }

    /// Whether the search of offers or the watch of an uninstaller still needs a visit.
    pub(super) fn still_polling(&self) -> bool {
        self.model.offers == Offers::Searching || self.watching.is_some()
    }
}
