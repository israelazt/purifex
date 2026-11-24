//! The search for traces in three parts, so a window can run the slow one off its own thread:
//! prepare (quick, needs the app's state), run (slow, owns nothing of it) and finish (the rules).

use std::sync::Arc;

use purifex_domain::apps::InstalledApp;
use purifex_domain::cleanup_log::CleanReport;
use purifex_domain::ports::{Found, TraceFinder};
use purifex_domain::search_step::SearchStep;

use super::Cleaning;

/// What a search needs to run on another thread.
pub struct SearchJob {
    pub(super) app: InstalledApp,
    others: Vec<InstalledApp>,
    pub(super) installed: bool,
    finder: Arc<dyn TraceFinder>,
}

impl SearchJob {
    /// The slow part: looks, saying when each part finishes.
    pub fn run(&self, step: &dyn Fn(SearchStep)) -> Result<Found, String> {
        self.finder.search_reporting(&self.app, step)
    }
}

impl Cleaning {
    /// The quick part: which app is meant and which others must be left alone.
    pub fn prepare_search(
        &self,
        apps: &[InstalledApp],
        all: &[InstalledApp],
        query: &str,
    ) -> Result<SearchJob, String> {
        let (app, installed) = self.resolve(apps, all, query)?;
        let finder = self
            .finder
            .clone()
            .ok_or("esta versión no sabe buscar restos")?;
        let others = all.iter().filter(|a| a.id != app.id).cloned().collect();
        Ok(SearchJob {
            app,
            others,
            installed,
            finder,
        })
    }

    /// What the rules propose from what the search found, as a preview that touches nothing.
    pub fn finish_search(&self, job: &SearchJob, found: &Found) -> CleanReport {
        CleanReport {
            app: job.app.name.clone(),
            preview: true,
            proposed: self.propose(&job.app, &job.others, found),
            incomplete: found.incomplete,
            other_users_unread: found.other_users_unread,
            still_installed: job.installed,
            ..Default::default()
        }
    }
}
