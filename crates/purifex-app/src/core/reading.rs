//! Reading what the page being shown is missing. Each page has its own small reader;
//! a failure is collected (with what was being read) and the rest is still read.

use purifex_domain::driver_offers::Offers;
use purifex_domain::trace_text::Counts;
use purifex_domain::view::Page;

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
        if self.model.apps.is_none() {
            if let Some(apps) = read(self.services.apps(), "apps", errors) {
                let old = self.previous_apps.take();
                self.model.apps = Some(apps);
                if let Some(gone) = old.as_deref().and_then(|o| self.vanished(o)) {
                    self.model.finding = Some(gone);
                }
            }
        }
        if let Some(name) = self.model.finding.take() {
            self.find_traces_of(&name, errors);
        }
    }

    /// Looks for the traces of an app. When some are found the window with the list
    /// opens by itself, with everything checked.
    fn find_traces_of(&mut self, name: &str, errors: &mut Vec<String>) {
        let Some(report) = read(self.services.clean_app(name, false, None), "restos", errors)
        else {
            return;
        };
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
