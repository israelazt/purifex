//! Starting an uninstall without freezing the window: the wizard opens at once on «Preparing»,
//! the safety nets are made on another thread and the uninstaller is started when they are ready.

use std::time::Instant;

use purifex_domain::safeguard::Started;
use purifex_domain::view::{Preparing, Wizard};

use super::Core;
use crate::services::UninstallPolled;

impl Core {
    /// Uninstalls an app the window lists, after the person confirmed.
    pub(super) fn uninstalled(&mut self, id: &str) {
        let Some(app) = self
            .model
            .apps
            .iter()
            .flatten()
            .find(|a| a.id == id)
            .cloned()
        else {
            self.model.error = Some("Esa app ya no está en la lista.".into());
            return;
        };
        match self.services.start_uninstall(&app) {
            Ok(()) => {
                (self.model.message, self.model.traces) = (None, None);
                self.model.traces_open = false;
                self.model.wizard = Some(Wizard::new(app.name.clone()));
                self.model.preparing = Some(Preparing {
                    app: app.name,
                    done: Vec::new(),
                });
                self.pending = true;
            }
            Err(error) => self.model.error = Some(error),
        }
    }

    /// Picks up the preparation if it has moved. It never waits. Returns whether the model changed.
    pub(super) fn poll_preparing(&mut self) -> bool {
        if self.model.preparing.is_none() {
            return false;
        }
        match self.services.poll_uninstall() {
            UninstallPolled::Working(done) => {
                let Some(preparing) = self.model.preparing.as_mut() else {
                    return false;
                };
                let changed = preparing.done != done;
                preparing.done = done;
                changed
            }
            UninstallPolled::Done(Ok(started)) => {
                self.model.preparing = None;
                self.uninstaller_started(&started);
                true
            }
            UninstallPolled::Done(Err(error)) => {
                (self.model.preparing, self.model.wizard) = (None, None);
                self.model.error = Some(error);
                true
            }
            UninstallPolled::Idle => {
                self.model.preparing = None;
                true
            }
        }
    }

    /// The uninstaller is open: from now on Purifex waits for the app to disappear.
    fn uninstaller_started(&mut self, started: &Started) {
        let app = started.app.clone();
        self.model.message = Some(format!(
            "Se abrió el desinstalador de {}. Cuando termine, Purifex busca sus restos y te los muestra. {}",
            app.name,
            started.safeguards_text()
        ));
        self.model.uninstalled = Some(app.name.clone());
        // A window the person hid stays hidden.
        if self.model.wizard.is_some() {
            self.model.wizard = Some(Wizard::new(app.name.clone()));
        }
        self.model.waiting = true;
        // From now on the app is looked for now and then: when it is gone the uninstaller has
        // finished and its traces are looked for.
        self.watching = Some((app, Instant::now(), Instant::now()));
        self.pending = true;
    }
}
