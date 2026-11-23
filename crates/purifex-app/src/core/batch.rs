//! Uninstalling several apps one after the other: what is checked in the list, the queue, and
//! the summary. Each uninstaller is started and waited for like a single one (same copy of its
//! registration); the restore point is asked for once, before the first.

use std::time::Instant;

use purifex_domain::batch::{Batch, BatchEntry, BatchResult};
use purifex_domain::view::Confirm;

use super::Core;

impl Core {
    /// Whether a batch has an uninstaller running or apps waiting their turn.
    pub(super) fn batch_running(&self) -> bool {
        self.model.batch.as_ref().is_some_and(|b| !b.finished())
    }

    /// Checks or unchecks an app of the list. Only an app that may be uninstalled from here is
    /// checked: not a protected one, and not one that is not in the list.
    pub(super) fn toggle_mark(&mut self, id: String) {
        if let Some(at) = self.model.marked.iter().position(|m| *m == id) {
            self.model.marked.remove(at);
            return;
        }
        let allowed = self
            .model
            .apps
            .iter()
            .flatten()
            .any(|a| a.id == id && a.protection().is_none());
        if allowed {
            self.model.marked.push(id);
        }
    }

    /// The checked apps that are still in the list, in the order they were checked.
    fn marked_ids(&self) -> Vec<String> {
        let apps = self.model.apps.as_deref().unwrap_or_default();
        self.model
            .marked
            .iter()
            .filter(|id| {
                apps.iter()
                    .any(|a| a.id == **id && a.protection().is_none())
            })
            .cloned()
            .collect()
    }

    /// Asks to uninstall the checked apps.
    pub(super) fn ask_uninstall_marked(&mut self) {
        if self.batch_running() {
            self.model.message = Some("Ya hay una desinstalación en lote en marcha.".into());
        } else if self.marked_ids().is_empty() {
            self.model.message = Some("No hay ninguna app marcada.".into());
        } else {
            self.ask(Confirm::UninstallMarked);
        }
    }

    /// After the person confirmed: one restore point for all, and the first uninstaller starts.
    pub(super) fn batch_started(&mut self) {
        let ids = self.marked_ids();
        if ids.is_empty() || self.batch_running() {
            return;
        }
        let safeguard = self.services.begin_batch(ids.len()).to_text();
        self.model.marked.clear();
        self.model.batch = Some(Batch::new(ids, safeguard));
        (self.model.traces, self.model.traces_open) = (None, false);
        self.start_next();
    }

    /// Starts the uninstaller of the next app of the queue; one that cannot be started is noted
    /// and the next one is tried. When none is left the batch ends.
    fn start_next(&mut self) {
        loop {
            let Some(batch) = self.model.batch.as_mut() else {
                return;
            };
            if batch.queue.is_empty() || batch.cancelled {
                return self.batch_ended();
            }
            let id = batch.queue.remove(0);
            match self.services.uninstall_app(&id) {
                Ok(started) => {
                    let app = started.app;
                    if let Some(batch) = self.model.batch.as_mut() {
                        batch.current = Some(app.name.clone());
                    }
                    self.model.message = Some(format!(
                        "Se abrió el desinstalador de {}. Cuando termine sigue la siguiente app.",
                        app.name
                    ));
                    self.model.waiting = true;
                    self.watching = Some((app, Instant::now(), Instant::now()));
                    self.pending = true;
                    return;
                }
                Err(why) => {
                    let name = self.name_of(&id);
                    self.note(name, BatchResult::Failed(why));
                }
            }
        }
    }

    /// The name of an app of the list by its id (the id itself if it is not there).
    fn name_of(&self, id: &str) -> String {
        let apps = self.model.apps.iter().flatten();
        apps.clone()
            .find(|a| a.id == id)
            .map_or_else(|| id.to_string(), |a| a.name.clone())
    }

    fn note(&mut self, name: String, result: BatchResult) {
        if let Some(batch) = self.model.batch.as_mut() {
            batch.done.push(BatchEntry { name, result });
        }
    }

    /// The uninstaller that was running finished (or was given up): it is noted and the next one
    /// starts.
    pub(super) fn batch_step(&mut self, result: BatchResult) {
        let current = self.model.batch.as_mut().and_then(|b| b.current.take());
        if let Some(name) = current {
            self.note(name, result);
        }
        self.watching = None;
        self.model.waiting = false;
        self.start_next();
    }

    /// The person does not want to wait any more: what runs is left alone and no other starts.
    pub(super) fn batch_cancelled(&mut self) {
        if let Some(batch) = self.model.batch.as_mut() {
            batch.cancelled = true;
        }
        self.batch_step(BatchResult::StillInstalled);
    }

    /// The queue is empty: the summary is shown and the apps are read again.
    fn batch_ended(&mut self) {
        self.services.end_batch();
        self.watching = None;
        self.model.waiting = false;
        if let Some(batch) = &self.model.batch {
            let safeguard = batch.safeguard.as_deref().map(|s| format!(" {s}"));
            self.model.message = Some(format!(
                "{}{}",
                batch.summary(),
                safeguard.unwrap_or_default()
            ));
        }
        self.model.apps = None;
        self.expect_reading();
    }
}
