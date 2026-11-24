//! Uninstalling in three parts, so a window can run the slow one off its own thread: prepare
//! (quick), run (reads what the app registered and asks Windows for a restore point, which can
//! take a minute) and finish (keeps what was seen, copies the registration and starts the
//! uninstaller, in that order).

use std::sync::Arc;

use purifex_domain::apps::InstalledApp;
use purifex_domain::ports::{RestorePoints, SeedProbe};
use purifex_domain::prep_step::PrepStep;
use purifex_domain::safeguard::{RestorePointOutcome, Started};

use super::{find, Cleaning};

/// What preparing an uninstall needs to run on another thread.
pub struct UninstallJob {
    app: InstalledApp,
    probe: Option<Arc<dyn SeedProbe>>,
    points: Option<Arc<dyn RestorePoints>>,
    /// The one already taken for a batch.
    batch_point: Option<RestorePointOutcome>,
}

/// An app ready to be uninstalled: what was seen of it and what Windows said of the restore point.
pub struct Prepared {
    app: InstalledApp,
    restore_point: RestorePointOutcome,
}

impl UninstallJob {
    /// The slow part, saying when each part finishes.
    pub fn run(mut self, step: &dyn Fn(PrepStep)) -> Prepared {
        // Observed first: once the uninstaller finishes the system forgets the app, and its
        // folder and programs are gone.
        if let Some(probe) = &self.probe {
            self.app.observed = probe.observe(&self.app);
        }
        step(PrepStep::Registry);
        // The way back for the whole system goes before the uninstaller does anything.
        let restore_point = self.batch_point.take().unwrap_or_else(|| {
            asked(
                self.points.as_deref(),
                &format!("Purifex: antes de desinstalar {}", self.app.name),
            )
        });
        step(PrepStep::RestorePoint);
        Prepared {
            app: self.app,
            restore_point,
        }
    }
}

/// Asks Windows for a restore point and says what it answered.
pub(super) fn asked(points: Option<&dyn RestorePoints>, why: &str) -> RestorePointOutcome {
    match points {
        None => RestorePointOutcome::NotAsked,
        Some(points) => match points.create(why) {
            Ok(()) => RestorePointOutcome::Requested,
            Err(refused) => RestorePointOutcome::Unavailable(refused),
        },
    }
}

impl Cleaning {
    /// The restore point before an app is removed without its uninstaller (or the batch's).
    pub(super) fn restore_point_for(&self, app: &InstalledApp) -> RestorePointOutcome {
        self.batch_point.clone().unwrap_or_else(|| {
            asked(
                self.restore_points.as_deref(),
                &format!("Purifex: antes de desinstalar {}", app.name),
            )
        })
    }

    /// The quick part: which app, whether it may be uninstalled and whether this can.
    pub fn prepare_uninstall(
        &self,
        apps: &[InstalledApp],
        query: &str,
    ) -> Result<UninstallJob, String> {
        let app = find(apps, query)?.clone();
        if let Some(why) = app.protection() {
            return Err(format!("{} no se desinstala desde aquí: {why}", app.name));
        }
        if self.launcher.is_none() {
            return Err("esta versión no sabe desinstalar".into());
        }
        Ok(UninstallJob {
            app,
            probe: self.probe.clone(),
            points: self.restore_points.clone(),
            batch_point: self.batch_point.clone(),
        })
    }

    /// Keeps what was seen of the app, copies its registration and starts its uninstaller.
    pub fn finish_uninstall(&mut self, prepared: Prepared) -> Result<Started, String> {
        let Prepared { app, restore_point } = prepared;
        self.store()?.remember(&app)?;
        let registry_copy = self.registry_copy_of(&app);
        if let Some(launcher) = &self.launcher {
            launcher.launch(&app)?;
        }
        Ok(Started {
            app,
            restore_point,
            registry_copy,
        })
    }

    /// Starts the uninstaller of the app, all at once. Protected apps are refused.
    pub fn uninstall(&mut self, apps: &[InstalledApp], query: &str) -> Result<Started, String> {
        let job = self.prepare_uninstall(apps, query)?;
        let prepared = job.run(&|_| {});
        self.finish_uninstall(prepared)
    }
}
