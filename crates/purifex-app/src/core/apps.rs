//! What the core does about apps: starting an uninstaller, waiting for it to finish,
//! seeing which app disappeared, and removing its traces.

use std::time::Instant;

use super::{Core, WATCH_EVERY, WATCH_FOR};

impl Core {
    /// The app that was in `old` and is no longer listed (the user uninstalled it,
    /// here or elsewhere): remembered, shown, and its name returned to look for its
    /// traces. Protected apps are not considered: their ids change when they update.
    pub(super) fn vanished(
        &mut self,
        old: &[purifex_domain::apps::InstalledApp],
    ) -> Option<String> {
        let now = self.model.apps.as_deref()?;
        let gone = old
            .iter()
            .find(|a| a.protection().is_none() && !now.iter().any(|n| n.id == a.id))?
            .clone();
        let _ = self.services.note_uninstalled(&gone);
        self.model.uninstalled = Some(gone.name.clone());
        self.model.traces = None;
        self.model.message = Some(format!(
            "{} ya no está instalada. Estos son los restos que dejó.",
            gone.name
        ));
        Some(gone.name)
    }

    /// Starts the uninstaller of an app; its traces are looked for afterwards.
    pub(super) fn uninstalled(&mut self, id: &str) {
        match self.services.uninstall_app(id) {
            Ok(app) => {
                self.model.message = Some(format!(
                    "Se abrió el desinstalador de {}. Cuando termine, Purifex busca sus restos y te los muestra.",
                    app.name
                ));
                (self.model.uninstalled, self.model.traces) = (Some(app.name.clone()), None);
                self.model.traces_open = false;
                // From now on the list is read now and then: when the app is gone the
                // uninstaller has finished and its traces are looked for.
                self.watching = Some((app, Instant::now(), Instant::now()));
                self.pending = true;
            }
            Err(error) => self.model.error = Some(error),
        }
    }

    /// Removes the traces of an app to the quarantine: all of them, or only the ones
    /// the user left checked.
    pub(super) fn cleaned(&mut self, name: &str, everything: bool) {
        let checked: Option<Vec<String>> = if everything {
            None
        } else {
            self.model.traces.as_ref().map(|r| {
                r.listed()
                    .into_iter()
                    .map(|t| t.key())
                    .filter(|key| !self.model.unchecked.contains(key))
                    .collect()
            })
        };
        if checked.as_ref().is_some_and(Vec::is_empty) {
            self.model.message = Some("No hay ningún resto marcado.".into());
            return;
        }
        match self.services.clean_app(name, true, checked.as_deref()) {
            Ok(report) => {
                (self.model.traces_open, self.model.unchecked) = (false, Vec::new());
                let (done, failed) = (report.removed.len(), report.failed.len());
                self.model.message = Some(format!(
                    "Se quitaron {done} restos de {name} a la cuarentena{}. Para devolverlos: purifex apps restore N.",
                    if failed > 0 { format!(" ({failed} no se pudieron quitar)") } else { String::new() }
                ));
                (self.model.traces, self.model.apps) = (None, None);
                self.expect_reading();
            }
            Err(error) => self.model.error = Some(error),
        }
    }

    /// Starts the program again as an administrator after the person confirmed (Windows
    /// asks too). If they said no, nothing changes and the window stays as it was.
    pub(super) fn restarted_as_admin(&mut self, name: &str) {
        match self.services.restart_elevated(Some(name)) {
            Ok(()) => self.quit = true,
            Err(error) => self.model.error = Some(error),
        }
    }

    /// Puts back the traces of an app from the quarantine and says what happened.
    pub(super) fn apps_restored(&mut self, app: &str) {
        self.model.error = None;
        match self.services.restore_app_cleanup(app) {
            Ok(report) => {
                self.model.message = Some(report.to_text());
                self.model.quarantine = None;
                self.expect_reading();
            }
            Err(error) => self.model.error = Some(error),
        }
    }

    /// Deletes for good what the quarantine holds, after the person confirmed.
    pub(super) fn quarantine_emptied(&mut self) {
        match self.services.empty_quarantine(true) {
            Ok(report) => {
                self.model.message = Some(report.to_text());
                self.model.quarantine = None;
                self.expect_reading();
            }
            Err(error) => self.model.error = Some(error),
        }
    }

    /// While an uninstaller started here runs: reads the list now and then, and when the
    /// app is gone, remembers it and asks for its traces. Returns whether the model changed.
    pub(super) fn check_watch(&mut self) -> bool {
        let Some((app, last, started)) = &mut self.watching else {
            return false;
        };
        if started.elapsed() > WATCH_FOR {
            self.watching = None;
            return false;
        }
        if last.elapsed() < WATCH_EVERY {
            return false;
        }
        *last = Instant::now();
        let app = app.clone();
        let Ok(now) = self.services.apps() else {
            return false;
        };
        if now.iter().any(|a| a.id == app.id) {
            return false;
        }
        self.watching = None;
        self.model.apps = Some(now);
        let _ = self.services.note_uninstalled(&app);
        self.model.uninstalled = Some(app.name.clone());
        self.model.traces = None;
        self.model.message = Some(format!(
            "{} ya se desinstaló. Estos son los restos que dejó.",
            app.name
        ));
        self.model.finding = Some(app.name);
        true
    }
}
