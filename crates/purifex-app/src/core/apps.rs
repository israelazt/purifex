//! What the core does about apps: starting an uninstaller, waiting for it to finish,
//! seeing which app disappeared, and removing its traces.

use std::time::Instant;

use purifex_domain::batch::BatchResult;
use purifex_domain::scan_mode::ScanMode;
use purifex_domain::view::{Model, Wizard};

use super::{Core, LIST_EVERY, WATCH_EVERY, WATCH_FOR};

impl Core {
    /// Starts looking for the traces of an app without blocking the window: the search runs on
    /// its own thread and `poll_traces` picks the result up.
    pub(super) fn begin_finding(&mut self, name: String) {
        self.model.search_steps.clear();
        match self.services.start_traces(&name) {
            Ok(()) => self.model.finding = Some(name),
            Err(error) => {
                self.model.finding = None;
                self.model.error = Some(format!("restos: {error}"));
            }
        }
        self.pending = true;
    }

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
            Ok(started) => {
                let app = started.app.clone();
                self.model.message = Some(format!(
                    "Se abrió el desinstalador de {}. Cuando termine, Purifex busca sus restos y te los muestra. {}",
                    app.name,
                    started.safeguards_text()
                ));
                (self.model.uninstalled, self.model.traces) = (Some(app.name.clone()), None);
                self.model.traces_open = false;
                self.model.wizard = Some(Wizard::new(app.name.clone()));
                self.model.waiting = true;
                // From now on the list is read now and then: when the app is gone the
                // uninstaller has finished and its traces are looked for.
                self.watching = Some((app, Instant::now(), Instant::now()));
                self.pending = true;
            }
            Err(error) => self.model.error = Some(error),
        }
    }

    /// Forces the removal of an app after the person confirmed: no uninstaller runs, its entry
    /// goes to the quarantine, and what it left is looked for and shown.
    pub(super) fn force_uninstalled(&mut self, id: &str) {
        match self.services.force_uninstall(id) {
            Ok(started) => {
                let name = started.app.name.clone();
                self.model.apps = None;
                self.model.app_selected = None;
                self.find_traces(name.clone());
                self.model.message = Some(format!(
                    "Se quitó {name} sin su desinstalador. Estos son los restos que dejó. {}",
                    started.safeguards_text()
                ));
            }
            Err(error) => self.model.error = Some(error),
        }
    }

    /// Removes the traces of an app to the quarantine: all of them, or only the ones
    /// the user left checked. What the window showed is what goes, with no second search; the
    /// app must still be gone.
    pub(super) fn cleaned(&mut self, name: &str, everything: bool) {
        let checked = self.checked_traces(everything).unwrap_or_default();
        if checked.is_empty() {
            self.model.message = Some("No hay ningún resto marcado.".into());
            return;
        }
        self.begin_removal(name, &checked);
    }

    /// The traces to remove, by key: all of them (but the guesses that were not checked and what
    /// already failed), or only the checked ones. `None` when there is no list.
    fn checked_traces(&self, everything: bool) -> Option<Vec<String>> {
        let report = self.model.traces.as_ref()?;
        let (stuck, off) = (&self.model.stuck, &self.model.unchecked);
        Some(
            report
                .listed()
                .into_iter()
                .filter(|t| {
                    let key = t.key();
                    if !everything {
                        return !off.contains(&key);
                    }
                    // «Everything» is everything that is not a guess; a guess goes only if checked.
                    let failed_before = stuck.iter().any(|(k, _)| *k == key);
                    !failed_before && (!t.evidence.is_guess() || !off.contains(&key))
                })
                .map(|t| t.key())
                .collect(),
        )
    }

    /// Says what a removal did: in the page's message and as the last step of the window.
    pub(super) fn removal_said(&mut self, name: &str, done: usize, failed: usize) {
        let failures = match failed {
            0 => String::new(),
            n => format!(" ({n} no se pudieron quitar)"),
        };
        let start = format!("Se quitaron {done} restos de {name} a la cuarentena{failures}.");
        self.model.message = Some(format!("{start} Para devolverlos: purifex apps restore N."));
        self.model.wizard = Some(Wizard {
            app: name.to_string(),
            done: Some(format!(
                "{start} Si algo hacía falta, «Devolver todo» lo pone otra vez en su sitio."
            )),
        });
    }

    /// Changes how much is proposed and looks again for the traces being shown.
    pub(super) fn scan_mode_set(&mut self, mode: ScanMode) {
        if mode == self.model.scan_mode {
            return;
        }
        self.model.scan_mode = mode;
        self.services.set_scan_mode(mode);
        if let Some(name) = self.model.uninstalled.clone() {
            self.find_traces(name);
        }
    }

    /// Leaves to Windows, after the person confirmed, what the app left open.
    pub(super) fn deleted_at_reboot(&mut self, name: &str) {
        match self.services.delete_app_leftovers_at_reboot(name) {
            Ok(report) => {
                self.model.message = Some(report.to_text());
                if report.failed.is_empty() {
                    self.model.in_use = None;
                }
            }
            Err(error) => self.model.error = Some(error),
        }
    }

    /// Closes, after the person confirmed, the programs of the app that still run from its
    /// folder, and looks again for what it left (their files are free now).
    pub(super) fn programs_closed(&mut self, name: &str) {
        match self.services.close_programs(name) {
            Ok(report) => {
                let said = report.to_text();
                self.model.running = None;
                self.find_traces(name.to_string());
                self.model.message = Some(said);
            }
            Err(error) => self.model.error = Some(error),
        }
    }

    /// Changes how the list of apps looks and keeps the choice for the next run. Not being able
    /// to keep it is logged and nothing more: the list already changed.
    pub(super) fn app_list_chosen(&mut self, change: impl FnOnce(&mut Model)) {
        change(&mut self.model);
        let settings = purifex_domain::app_list::AppListSettings {
            filter: self.model.app_filter,
            sort: self.model.app_sort,
            view: self.model.app_view,
        };
        if let Err(why) = self.services.save_app_list_settings(&settings) {
            purifex_domain::log_warn!("the view of the apps could not be kept: {why}");
        }
    }

    /// Opens the details of an app, or closes them if they were open.
    pub(super) fn select_app(&mut self, id: String) {
        self.model.app_selected = (self.model.app_selected.as_ref() != Some(&id)).then_some(id);
    }

    /// Opens the folder of an app of the list. Only a folder some app registered is opened,
    /// whatever asked for it.
    pub(super) fn open_app_folder(&mut self, path: &str) {
        let known = self
            .model
            .apps
            .iter()
            .flatten()
            .any(|a| a.install_location.as_deref() == Some(path));
        self.model.error = if known {
            self.services.open_folder(path).err()
        } else {
            Some("esa carpeta no es de ninguna app de la lista".into())
        };
    }

    /// Shows where the app registered itself, in the registry editor. The key comes from the
    /// app of the list, never from what asked for it.
    pub(super) fn open_app_registry_key(&mut self, id: &str) {
        let key = self
            .model
            .apps
            .iter()
            .flatten()
            .find(|a| a.id == id)
            .and_then(|a| a.registry_key());
        self.model.error = match key {
            Some(key) => self.services.open_registry_key(&key).err(),
            None => Some("esa app no está registrada en el registro".into()),
        };
    }

    /// Opens the page an app registered; only the ones the list shows.
    pub(super) fn open_app_website(&mut self, url: &str) {
        let known = self
            .model
            .apps
            .iter()
            .flatten()
            .any(|a| a.website.as_deref() == Some(url));
        self.model.error = if known {
            self.services.open_website(url).err()
        } else {
            Some("esa página no es de ninguna app de la lista".into())
        };
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
                (self.model.quarantine, self.model.startup, self.model.junk) = (None, None, None);
                self.model.startup_quick = None;
                self.expect_reading();
            }
            Err(error) => self.model.error = Some(error),
        }
    }

    /// Registers an app again from the copy taken before it was uninstalled.
    pub(super) fn backup_restored(&mut self, registry_copy: &str) {
        self.model.error = None;
        match self.services.restore_backup(registry_copy) {
            Ok(entry) => {
                self.model.message = Some(format!(
                    "{} está registrada otra vez: Windows la vuelve a listar. Si sus archivos siguen en el disco, ya puedes abrir su desinstalador.",
                    entry.app
                ));
                (self.model.apps, self.model.backups) = (None, None);
                self.expect_reading();
            }
            Err(error) => self.model.error = Some(error),
        }
    }

    /// Deletes, after the person confirmed, a copy of the registration of an app.
    pub(super) fn backup_deleted(&mut self, registry_copy: &str) {
        match self.services.delete_backup(registry_copy) {
            Ok(entry) => {
                self.model.message = Some(format!(
                    "Se borró la copia del registro de {}: ya no se puede registrar de nuevo con ella.",
                    entry.app
                ));
                self.model.backups = None;
                self.expect_reading();
            }
            Err(error) => self.model.error = Some(error),
        }
    }

    /// Puts back everything of an app: its traces from the quarantine first (so its files are
    /// there) and then its registration. One failing does not stop the other; the message says
    /// what each did.
    pub(super) fn everything_restored(&mut self, app: &str, registry_copy: &str) {
        self.model.error = None;
        let traces = match self.services.restore_app_cleanup(app) {
            Ok(report) => report.to_text(),
            Err(why) => format!("Restos: {why}."),
        };
        let registration = match self.services.restore_backup(registry_copy) {
            Ok(_) => format!("{app} está registrada otra vez."),
            Err(why) => format!("Registro: {why}."),
        };
        self.model.message = Some(format!("{traces} {registration}"));
        (self.model.quarantine, self.model.apps, self.model.backups) = (None, None, None);
        self.expect_reading();
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

    /// The person does not want to wait any more: nothing else changes.
    pub(super) fn stop_waiting(&mut self) {
        if self.batch_running() {
            return self.batch_cancelled();
        }
        self.watching = None;
        self.model.waiting = false;
        self.expect_reading();
        self.model.message =
            Some("Dejé de esperar al desinstalador. Cuando termine, pulsa «Buscar restos».".into());
    }

    /// While an uninstaller started here runs: reads the list now and then, and when the
    /// app is gone, remembers it and asks for its traces. Returns whether the model changed.
    pub(super) fn check_watch(&mut self) -> bool {
        let Some((app, last, started)) = &mut self.watching else {
            return false;
        };
        if started.elapsed() > WATCH_FOR {
            let name = app.name.clone();
            if self.batch_running() {
                self.batch_step(BatchResult::StillInstalled);
                return true;
            }
            self.watching = None;
            self.model.waiting = false;
            self.model.message = Some(format!(
                "{name} sigue instalada después de esperar a su desinstalador. Si ya la desinstalaste, pulsa «Buscar restos»."
            ));
            return true;
        }
        if last.elapsed() < WATCH_EVERY {
            return false;
        }
        *last = Instant::now();
        let app = app.clone();
        match self.services.app_still_registered(&app.id) {
            Ok(true) => return false,
            Ok(false) => {}
            // Only its list can tell, and it is slow: the next look waits longer.
            Err(_) => *last = Instant::now() + (LIST_EVERY - WATCH_EVERY),
        }
        // The whole list is read once, when the app is gone (or when only it can tell).
        let Ok(now) = self.services.apps() else {
            return false;
        };
        if now.iter().any(|a| a.id == app.id) {
            return false;
        }
        self.model.apps = Some(now);
        let _ = self.services.note_uninstalled(&app);
        if self.batch_running() {
            self.batch_step(BatchResult::Uninstalled);
            return true;
        }
        self.watching = None;
        self.model.waiting = false;
        self.model.uninstalled = Some(app.name.clone());
        self.model.traces = None;
        self.model.message = Some(format!(
            "{} ya se desinstaló. Estos son los restos que dejó.",
            app.name
        ));
        self.begin_finding(app.name);
        true
    }
}
