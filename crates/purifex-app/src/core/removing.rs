//! Moving the traces the window showed to the quarantine, a slice at a time: each tick moves what
//! fits in a few milliseconds and the progress is drawn, so the window never freezes.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use purifex_domain::app_traces::Trace;
use purifex_domain::view::{InUse, Removing};

use super::Core;
use crate::cleaning::{RemovalStart, Removals};

/// How long one tick may spend moving traces.
const SLICE: Duration = Duration::from_millis(30);

/// A removal under way.
pub(super) struct RemovalQueue {
    app: String,
    queue: VecDeque<Trace>,
    start: RemovalStart,
    done: Removals,
    total: usize,
    bytes_total: u64,
}

impl Core {
    /// Starts moving the checked traces (by key) of the list that was shown.
    pub(super) fn begin_removal(&mut self, name: &str, keys: &[String]) {
        let queue: VecDeque<Trace> = self
            .model
            .traces
            .as_ref()
            .map(|report| report.listed())
            .unwrap_or_default()
            .into_iter()
            .filter(|t| keys.contains(&t.key()))
            .cloned()
            .collect();
        let start = match self.services.begin_removal(name) {
            Ok(start) => start,
            Err(error) => return self.model.error = Some(error),
        };
        let bytes_total = queue.iter().map(|t| t.bytes).sum();
        let total = queue.len();
        self.model.removing = Some(Removing {
            app: name.to_string(),
            done: 0,
            total,
            bytes_done: 0,
            bytes_total,
        });
        self.model.traces_open = false;
        self.removing = Some(RemovalQueue {
            app: name.to_string(),
            queue,
            start,
            done: Removals::default(),
            total,
            bytes_total,
        });
        self.pending = true;
    }

    /// Moves traces until the slice is spent. Returns whether the model changed.
    pub(super) fn remove_some(&mut self) -> bool {
        let Some(mut work) = self.removing.take() else {
            return false;
        };
        let began = Instant::now();
        let mut failure = None;
        while let Some(trace) = work.queue.pop_front() {
            let RemovalQueue {
                app, start, done, ..
            } = &mut work;
            if let Err(error) = self.services.remove_trace(app, &trace, start, done) {
                failure = Some(error);
                break;
            }
            if began.elapsed() >= SLICE {
                break;
            }
        }
        if failure.is_none() && !work.queue.is_empty() {
            self.model.removing = Some(work.progress());
            self.removing = Some(work);
            return true;
        }
        self.model.removing = None;
        self.removal_finished(&work.app, &work.done);
        if failure.is_some() {
            self.model.error = failure;
        }
        true
    }

    /// Every trace was tried: what failed stays left alone, and the window says what happened.
    fn removal_finished(&mut self, name: &str, done: &Removals) {
        (self.model.traces_open, self.model.unchecked) = (false, Vec::new());
        // What failed now is added to what failed before for this app: it stays left alone.
        for (t, why) in &done.failed {
            let key = t.key();
            self.model.stuck.retain(|(k, _)| *k != key);
            self.model.stuck.push((key, why.clone()));
        }
        self.removal_said(name, done.removed.len(), done.failed.len());
        (self.model.traces, self.model.apps) = (None, None);
        self.model.in_use = (!done.in_use.is_empty()).then(|| InUse {
            app: name.to_string(),
            items: done.in_use.len(),
        });
        self.expect_reading();
    }
}

impl RemovalQueue {
    fn progress(&self) -> Removing {
        let tried = self.done.removed.len() + self.done.failed.len();
        let bytes_done = self.done.removed.iter().map(|e| e.trace.bytes).sum();
        Removing {
            app: self.app.clone(),
            done: tried,
            total: self.total,
            bytes_done,
            bytes_total: self.bytes_total,
        }
    }
}
