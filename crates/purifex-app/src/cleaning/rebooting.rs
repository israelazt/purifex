//! What a program has open cannot be moved to the quarantine. The only way left is to ask
//! Windows to delete it at the next restart, and that cannot be undone: so it is only for
//! what the rules still propose for an app that is gone, that is open right now, and that
//! the person asked for knowing it. Each one is a line of the log, without a copy.

use purifex_domain::apps::InstalledApp;
use purifex_domain::cleanup_log::{next_seq, CleanupEntry, CleanupState};
use purifex_domain::quarantine::RebootReport;

use super::Cleaning;

impl Cleaning {
    pub fn delete_at_reboot(
        &mut self,
        apps: &[InstalledApp],
        all: &[InstalledApp],
        query: &str,
    ) -> Result<RebootReport, String> {
        let (app, installed) = self.resolve(apps, all, query)?;
        if installed {
            return Err(format!(
                "{} sigue instalada: sus archivos no se tocan",
                app.name
            ));
        }
        let finder = self
            .finder
            .as_ref()
            .ok_or("esta versión no sabe buscar restos")?;
        let others: Vec<InstalledApp> = all.iter().filter(|a| a.id != app.id).cloned().collect();
        let found = finder.search(&app)?;
        let proposed = self.propose(&app, &others, &found);
        let today = self.today();
        let store = self.store()?;
        let mut log = store.entries()?;
        let mut report = RebootReport {
            app: app.name.clone(),
            ..Default::default()
        };
        let open: Vec<_> = proposed
            .into_iter()
            .filter(|t| store.is_locked(t))
            .collect();
        for trace in open {
            match store.delete_at_reboot(&trace) {
                Ok(()) => {
                    let line = CleanupEntry {
                        seq: next_seq(&log),
                        app: app.name.clone(),
                        trace: trace.clone(),
                        state: CleanupState::Scheduled,
                        on: today,
                        ..Default::default()
                    };
                    store.append(&line)?;
                    log.push(line);
                    report.scheduled.push(trace);
                }
                Err(why) => report.failed.push((trace, why)),
            }
        }
        Ok(report)
    }
}
