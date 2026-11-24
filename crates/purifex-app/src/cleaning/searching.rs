//! The search for traces in two parts, so a window can run the slow one off its own thread:
//! prepare (quick, needs the app's state) and run (looks and judges, owns nothing of it).

use std::sync::Arc;

use purifex_domain::app_traces::{propose_guessing, propose_with, Candidate, Trace};
use purifex_domain::apps::InstalledApp;
use purifex_domain::cleanup_log::CleanReport;
use purifex_domain::ports::{Found, TraceFinder};
use purifex_domain::scan_mode::ScanMode;
use purifex_domain::search_step::SearchStep;

use super::logging::merge_logged;
use super::Cleaning;

/// What the rules propose for the app, from what was found and what the logs of installations
/// name, kept to what the mode allows. It only computes (with many apps it takes a while).
pub(super) fn judge(
    app: &InstalledApp,
    others: &[InstalledApp],
    found: &Found,
    logged: Vec<Candidate>,
    mode: ScanMode,
) -> Vec<Trace> {
    let candidates = merge_logged(&found.candidates, logged);
    let judging = if mode == ScanMode::Advanced {
        propose_guessing
    } else {
        propose_with
    };
    let mut traces = judging(app, others, &found.seeds, &candidates);
    traces.retain(|t| mode.allows(t.evidence));
    traces
}

/// What a search needs to run on another thread.
pub struct SearchJob {
    pub(super) app: InstalledApp,
    others: Vec<InstalledApp>,
    pub(super) installed: bool,
    finder: Arc<dyn TraceFinder>,
    /// What the logs of installations name for the app (read before: they are not for threads).
    logged: Vec<Candidate>,
    mode: ScanMode,
}

impl SearchJob {
    /// The slow part: looks, saying when each part finishes, judges and answers with a preview
    /// that touches nothing.
    pub fn run(&self, step: &dyn Fn(SearchStep)) -> Result<CleanReport, String> {
        let found = self.finder.search_reporting(&self.app, step)?;
        Ok(CleanReport {
            app: self.app.name.clone(),
            preview: true,
            proposed: judge(
                &self.app,
                &self.others,
                &found,
                self.logged.clone(),
                self.mode,
            ),
            incomplete: found.incomplete,
            other_users_unread: found.other_users_unread,
            still_installed: self.installed,
            ..Default::default()
        })
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
        let logged = self.logged(&app);
        Ok(SearchJob {
            app,
            others,
            installed,
            finder,
            logged,
            mode: self.mode,
        })
    }
}
