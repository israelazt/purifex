//! What each action of the person does to the model, one small method per action.
//! `Core::apply` only chooses which one.

use purifex_domain::driver_offers::Offers;
use purifex_domain::view::{Action, Confirm, Page};

use super::Core;

impl Core {
    /// The actions of the tools of the apps page: the batch, the install monitor and hunter mode.
    pub(super) fn apply_apps_tools(&mut self, action: Action) {
        match action {
            Action::ToggleMark(id) => self.toggle_mark(id),
            Action::ClearMarks => self.model.marked.clear(),
            Action::AskUninstallMarked => self.ask_uninstall_marked(),
            Action::CloseBatch => self.model.batch = None,
            Action::MonitorStart => self.monitor_started(),
            Action::MonitorFinish => self.monitor_finished(),
            Action::MonitorCancel => self.monitor_cancelled(),
            Action::OpenLog(id) => self.log_opened(id),
            Action::ExportLog(id) => self.log_exported(&id),
            Action::ImportLog => self.log_imported(),
            Action::AskDeleteLog(id) => self.ask(Confirm::DeleteLog(id)),
            Action::StartHunt => self.hunt_started(),
            Action::DropHunt => self.hunt_dropped(),
            Action::CloseHunt => self.model.hunt.found = None,
            Action::AskCloseHunted => self.hunted_close_asked(),
            Action::OpenHuntedFolder => self.hunted_folder_opened(),
            _ => {}
        }
    }

    pub(super) fn go(&mut self, page: Page) {
        if page != self.model.page {
            // The search belongs to the page it was typed in.
            self.model.query.clear();
        }
        self.model.page = page;
        self.model.error = None;
        // What was waiting for a confirmation, or told of a change, stays on its page.
        (self.model.confirm, self.model.notice) = (None, None);
        self.expect_reading();
    }

    /// Forgets everything that was read, so the page reads it again.
    pub(super) fn rescan(&mut self) {
        let m = &mut self.model;
        (m.overview, m.drivers, m.sessions, m.analysis, m.comparison) =
            (None, None, None, None, None);
        (m.candidates, m.apps, m.traces, m.quarantine) = (None, None, None, None);
        (m.fixes, m.history, m.confirm, m.notice) = (None, None, None, None);
        m.error = None;
        self.expect_reading();
    }

    pub(super) fn search_updates(&mut self) {
        match self.services.start_offers() {
            Ok(()) => {
                self.model.offers = Offers::Searching;
                self.pending = true;
            }
            Err(error) => self.model.offers = Offers::Failed(error),
        }
    }

    pub(super) fn open_page(&mut self, url: &str) {
        self.model.error = self.services.open_page(url).err();
    }

    pub(super) fn ask(&mut self, confirm: Confirm) {
        self.model.confirm = Some(confirm);
    }

    /// Checks a trace if it was unchecked and the other way round.
    pub(super) fn toggle_trace(&mut self, location: String) {
        let unchecked = &mut self.model.unchecked;
        match unchecked.iter().position(|l| *l == location) {
            Some(at) => {
                unchecked.remove(at);
            }
            None => unchecked.push(location),
        }
    }

    /// Checks all the traces (`true`) or none of them.
    pub(super) fn select_traces(&mut self, all: bool) {
        self.model.unchecked = match (&self.model.traces, all) {
            (Some(report), false) => report.listed().into_iter().map(|t| t.key()).collect(),
            _ => Vec::new(),
        };
    }

    pub(super) fn open_traces(&mut self) {
        self.model.traces_open = self
            .model
            .traces
            .as_ref()
            .is_some_and(|r| !r.proposed.is_empty());
    }

    pub(super) fn close_traces(&mut self) {
        self.model.traces_open = false;
        self.model.confirm = None;
    }

    pub(super) fn find_traces(&mut self, name: String) {
        if self.model.uninstalled.as_deref() != Some(&name) {
            self.model.stuck.clear();
        }
        (self.model.traces, self.model.message) = (None, None);
        (self.model.uninstalled, self.model.finding) = (Some(name.clone()), Some(name));
        self.expect_reading();
    }

    /// Reads the list of apps again when the window comes back (the person may have
    /// uninstalled something meanwhile), unless a search of traces is going. The list
    /// stays on the screen, as it is, until the new one is read: nothing flashes and the
    /// place the person was reading is kept.
    pub(super) fn refresh_apps(&mut self) {
        if self.model.page != Page::Apps || self.model.finding.is_some() {
            return;
        }
        if self.model.apps.is_some() {
            self.refreshing_apps = true;
            self.pending = true;
        }
    }

    pub(super) fn select_session(&mut self, id: String) {
        self.model.selected = Some(id);
        self.model.analysis = None;
        self.model.comparison = None;
        self.model.error = None;
        self.expect_reading();
    }
}
