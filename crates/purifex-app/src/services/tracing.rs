//! What the window asks for that takes long: preparing an uninstall, the search for traces and
//! the removal in slices.

use super::*;
use crate::trace_search::Polled;

impl<I, C, S, C2, F, A> App<I, C, S, C2, F, A> {
    pub(super) fn search_traces(&mut self, query: &str) -> Result<(), String> {
        let (listed, all) = self.installed()?;
        let job = self.cleaning.prepare_search(&listed, &all, query)?;
        self.trace_search.start(job)
    }

    pub(super) fn polled_traces(&mut self) -> TracesPolled {
        match self.trace_search.poll() {
            Polled::Idle => TracesPolled::Idle,
            Polled::Working(steps) => TracesPolled::Working(steps),
            Polled::Done(report) => TracesPolled::Done(report),
        }
    }

    pub(super) fn prepare(&mut self, app: &InstalledApp) -> Result<(), String> {
        let job = self
            .cleaning
            .prepare_uninstall(std::slice::from_ref(app), &app.id)?;
        self.uninstall_prep.start(job)
    }

    pub(super) fn polled_uninstall(&mut self) -> UninstallPolled {
        use crate::uninstall_prep::Polled as Prep;
        match self.uninstall_prep.poll() {
            Prep::Idle => UninstallPolled::Idle,
            Prep::Working(steps) => UninstallPolled::Working(steps),
            Prep::Done(prepared) => UninstallPolled::Done(
                prepared
                    .and_then(|ready| self.cleaning.finish_uninstall(*ready))
                    .map(Box::new),
            ),
        }
    }

    pub(super) fn start_removal(&mut self, query: &str) -> Result<RemovalStart, String> {
        let (listed, all) = self.installed()?;
        self.cleaning.begin_removal(&listed, &all, query)
    }
}
