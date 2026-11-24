//! Removing traces one at a time, so a window can do it in slices and show the progress.

use purifex_domain::app_traces::Trace;
use purifex_domain::apps::InstalledApp;
use purifex_domain::cleanup_log::{next_seq, CleanupEntry, CleanupState};
use purifex_domain::date::Date;

use super::{Cleaning, Removals};

/// What a removal in slices carries from one slice to the next.
pub struct RemovalStart {
    pub log: Vec<CleanupEntry>,
    pub today: Option<Date>,
}

impl Cleaning {
    /// Checks that the app is gone (the traces of one in use are never touched) and takes the
    /// log the removals will be added to.
    pub fn begin_removal(
        &mut self,
        apps: &[InstalledApp],
        all: &[InstalledApp],
        query: &str,
    ) -> Result<RemovalStart, String> {
        let (app, installed) = self.resolve(apps, all, query)?;
        if installed {
            return Err(format!("{} sigue instalada: termina su desinstalador primero (los restos de una app en uso no se tocan)", app.name));
        }
        Ok(RemovalStart {
            log: self.store()?.entries()?,
            today: self.today(),
        })
    }

    /// Moves one trace to the quarantine and logs it. One that cannot be removed (in use, needs
    /// an administrator) goes to `done.failed` with why; only a log that cannot be written is an
    /// error.
    pub fn remove_one(
        &mut self,
        app: &str,
        trace: &Trace,
        start: &mut RemovalStart,
        done: &mut Removals,
    ) -> Result<(), String> {
        let store = self.store()?;
        let seq = next_seq(&start.log);
        match store.remove(trace, seq) {
            Ok(kept) => {
                let entry = CleanupEntry {
                    seq,
                    app: app.to_string(),
                    trace: trace.clone(),
                    kept,
                    state: CleanupState::Removed,
                    on: start.today,
                    ..Default::default()
                };
                store.append(&entry)?;
                start.log.push(entry.clone());
                done.removed.push(entry);
            }
            Err(why) => {
                if store.is_locked(trace) {
                    done.in_use.push(trace.clone());
                }
                done.failed.push((trace.clone(), why));
            }
        }
        Ok(())
    }
}
