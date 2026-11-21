//! Hunter mode in the core: the sight is picked up, let go over a window, and what it landed on
//! is shown with what can be done to it. Closing the program is asked first and only ever
//! closes the one that was shown.

use purifex_domain::hunter::hunted;

use super::Core;

impl Core {
    /// The sight was picked up: the pointer turns into a crosshair until it is let go.
    pub(super) fn hunt_started(&mut self) {
        self.model.hunt.aiming = true;
        self.model.hunt.found = None;
        self.model.error = None;
    }

    /// The sight was let go: the program under the pointer, and the app it belongs to.
    pub(super) fn hunt_dropped(&mut self) {
        if !std::mem::take(&mut self.model.hunt.aiming) {
            return;
        }
        match self.services.window_under_pointer() {
            Ok(Some(process)) if process.pid == std::process::id() => {
                self.model.message = Some(
                    "Suelta la mira sobre la ventana de otro programa, no sobre Purifex.".into(),
                );
            }
            Ok(Some(process)) => {
                let listed = self.model.apps.as_deref().unwrap_or_default();
                self.model.hunt.found = Some(hunted(process, listed));
            }
            Ok(None) => {
                self.model.message = Some("No se pudo ver qué programa tiene esa ventana.".into());
            }
            Err(why) => self.model.error = Some(why),
        }
    }

    /// Asks to close the program shown, unless Windows runs it or it is this program.
    pub(super) fn hunted_close_asked(&mut self) {
        let Some(found) = &self.model.hunt.found else {
            return;
        };
        if let Some(why) = found.protected(std::process::id()) {
            self.model.error = Some(format!("{} no se cierra: {why}.", found.process.name()));
            return;
        }
        self.model.confirm = Some(purifex_domain::view::Confirm::CloseHunted(
            found.process.pid,
        ));
    }

    /// Closes, after the person confirmed, the program that was shown (and no other).
    pub(super) fn hunted_closed(&mut self, pid: u32) {
        let Some(found) = self
            .model
            .hunt
            .found
            .take()
            .filter(|h| h.process.pid == pid)
        else {
            return;
        };
        let name = found.process.name().to_string();
        match self.services.close_process(&found.process) {
            Ok(()) => self.model.message = Some(format!("Se cerró {name}.")),
            Err(why) => self.model.error = Some(format!("No se pudo cerrar {name}: {why}.")),
        }
    }

    /// Opens the folder of the program shown.
    pub(super) fn hunted_folder_opened(&mut self) {
        if let Some(found) = &self.model.hunt.found {
            self.model.error = self.services.open_folder(found.folder()).err();
        }
    }
}
