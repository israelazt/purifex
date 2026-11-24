//! The search for traces and the removal in slices, as the window asks for them.

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
            Polled::Done(job, found) => {
                TracesPolled::Done(found.map(|found| self.cleaning.finish_search(&job, &found)))
            }
        }
    }

    pub(super) fn start_removal(&mut self, query: &str) -> Result<RemovalStart, String> {
        let (listed, all) = self.installed()?;
        self.cleaning.begin_removal(&listed, &all, query)
    }
}
