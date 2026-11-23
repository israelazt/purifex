//! The Startup page in the core: what starts with Windows, turning an entry on or off, taking it
//! out to the quarantine and opening the folder of its program.

use purifex_domain::startup::StartupItem;
use purifex_domain::view::Confirm;

use super::Core;

impl Core {
    pub(super) fn read_startup_page(&mut self, errors: &mut Vec<String>) {
        self.read_quarantine();
        if self.model.startup.is_none() {
            match self.services.startup_items() {
                Ok(items) => self.model.startup = Some(items),
                Err(why) => {
                    errors.push(format!("inicio: {why}"));
                    self.model.startup = Some(Vec::new());
                }
            }
        }
    }

    /// An entry of the list as it was read (what is acted on is always one of these, never
    /// something that only arrived in an action).
    fn startup_item(&self, id: &str) -> Option<&StartupItem> {
        let quick = self.model.startup_quick.iter().flatten();
        self.model
            .startup
            .iter()
            .flatten()
            .chain(quick)
            .find(|i| i.id == id)
    }

    /// Turns an entry on if it was off and the other way round. Nothing is deleted.
    pub(super) fn startup_toggled(&mut self, id: &str) {
        let Some(item) = self.startup_item(id) else {
            return;
        };
        let (name, on) = (item.name.clone(), !item.enabled);
        match self.services.set_startup(id, on) {
            Ok(now) => {
                if let Some(slot) = self.model.startup.iter_mut().flatten().find(|i| i.id == id) {
                    *slot = StartupItem {
                        app: slot.app.clone(),
                        ..now
                    };
                }
                self.model.error = None;
                self.model.message = Some(if on {
                    format!("{name} volverá a arrancar con Windows.")
                } else {
                    format!("{name} ya no arranca con Windows. Sigue instalada y puedes volver a activarla.")
                });
            }
            Err(error) => self.model.error = Some(error),
        }
    }

    /// Asks to take an entry out; services (and anything that is not in the list) are not.
    pub(super) fn startup_remove_asked(&mut self, id: String) {
        if self.startup_item(&id).is_some_and(|i| i.trace().is_some()) {
            self.ask(Confirm::RemoveStartup(id));
        }
    }

    pub(super) fn startup_removed(&mut self, id: &str) {
        let Some(name) = self.startup_item(id).map(|i| i.name.clone()) else {
            return;
        };
        match self.services.remove_startup(id) {
            Ok(entry) => {
                self.model.message = Some(format!(
                    "{name} se quitó del inicio y está en la cuarentena. Para devolverlo: purifex apps restore {}.",
                    entry.seq
                ));
                (self.model.startup, self.model.startup_quick) = (None, None);
                self.model.quarantine = None;
                self.expect_reading();
            }
            Err(error) => self.model.error = Some(error),
        }
    }

    /// Opens the folder of the program an entry runs.
    pub(super) fn startup_folder_opened(&mut self, id: &str) {
        let folder = self.startup_item(id).and_then(StartupItem::folder);
        self.model.error = match folder {
            Some(path) => self.services.open_folder(&path).err(),
            None => Some("no se sabe en qué carpeta está ese programa".into()),
        };
    }
}
