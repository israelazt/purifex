//! What the logs of installations add to a search: the places they name for the app, which the
//! rules still judge (Windows' places and the too broad are refused as always).

use purifex_domain::app_traces::Candidate;
use purifex_domain::apps::InstalledApp;
use purifex_domain::ports::InstallLogs;

use super::Cleaning;

/// Whether two candidates are the same place (the log's wins over the finder's).
fn same_place(a: &Candidate, b: &Candidate) -> bool {
    a.kind == b.kind && a.location.eq_ignore_ascii_case(&b.location) && a.value == b.value
}

impl Cleaning {
    /// The logs of installations give their places for the app they belong to.
    pub fn with_logs(mut self, logs: Box<dyn InstallLogs>) -> Self {
        self.logs = Some(logs);
        self
    }

    /// The places that the logs of this app name.
    fn logged(&self, app: &InstalledApp) -> Vec<Candidate> {
        let Some(Ok(all)) = self.logs.as_ref().map(|l| l.all()) else {
            return Vec::new();
        };
        all.iter()
            .filter(|l| l.is_of(&app.name))
            .flat_map(|l| l.candidates())
            .collect()
    }

    /// What the finder found plus what the logs name, each place once (the log's reading wins).
    pub(super) fn with_logged(&self, app: &InstalledApp, found: &[Candidate]) -> Vec<Candidate> {
        let logged = self.logged(app);
        let mut all: Vec<Candidate> = found
            .iter()
            .filter(|c| !logged.iter().any(|l| same_place(l, c)))
            .cloned()
            .collect();
        all.extend(logged);
        all
    }
}
