//! What the application can do, as one trait: the command line (and the MCP
//! server) ask for things through it, without knowing which ports are behind.
//! [`App`] is the real implementation, built from the services of this crate.

use purifex_domain::analysis::Analysis;
use purifex_domain::compare::ComparisonReport;
use purifex_domain::drivers_view::DriversView;
use purifex_domain::fix::{FixReport, HistoryEntry, Proposal};
use purifex_domain::overview::Overview;
use purifex_domain::ports::{
    Clock, PerformanceSampler, SessionArchive, SessionFile, SystemInventory,
};
use purifex_domain::report::{ScanReport, Scope};
use purifex_domain::session_list::SessionRow;

use crate::archive::Archive;
use crate::fixer::Fixer;
use crate::recorder::{Recorder, Recording, WatchOptions};
use crate::scanner::Scanner;

pub trait Services {
    /// Reads the devices and their drivers.
    fn scan(&self, scope: Scope) -> Result<ScanReport, String>;
    /// Waits for the game (or not) and records it. `progress` gets the lines
    /// worth showing while it works.
    fn record(
        &mut self,
        options: WatchOptions,
        progress: &mut dyn FnMut(&str),
    ) -> Result<Recording, String>;
    /// The recordings made so far, the newest first.
    fn sessions(&self) -> Result<Vec<SessionRow>, String>;
    /// The analysis of a recording; with no name, of the newest.
    fn analyze(&self, id: Option<&str>) -> Result<Analysis, String>;
    /// Two recordings compared; with no names, the two newest games.
    fn compare(
        &self,
        before: Option<&str>,
        after: Option<&str>,
    ) -> Result<ComparisonReport, String>;
    /// The fixes worth offering now (what the last game justifies).
    fn fixes(&self) -> Result<Vec<Proposal>, String>;
    /// Applies a fix; without `confirmed` (or with `dry_run`) it only says what it would do.
    fn apply_fix(&mut self, id: &str, confirmed: bool, dry_run: bool) -> Result<FixReport, String>;
    /// Undoes a change of the history.
    fn undo_fix(&mut self, seq: u64, confirmed: bool) -> Result<FixReport, String>;
    fn history(&self) -> Result<Vec<HistoryEntry>, String>;
    /// The devices and their drivers, as the table shows them.
    fn drivers(&self) -> Result<DriversView, String>;
    /// The general state: a fresh scan and the last recording.
    fn overview(&self) -> Result<Overview, String>;
}

/// The services of the application over real (or fake) ports.
pub struct App<I, C, S, C2, F, A> {
    scanner: Scanner<I, C>,
    recorder: Recorder<S, C2, F>,
    archive: Archive<A>,
    fixer: Fixer,
}

impl<I, C, S, C2, F, A> App<I, C, S, C2, F, A> {
    pub fn new(scanner: Scanner<I, C>, recorder: Recorder<S, C2, F>, archive: Archive<A>) -> Self {
        Self {
            scanner,
            recorder,
            archive,
            fixer: Fixer::disabled(),
        }
    }

    /// The fixes this application can make (none until given).
    pub fn with_fixer(mut self, fixer: Fixer) -> Self {
        self.fixer = fixer;
        self
    }
}

impl<I, C, S, C2, F, A> Services for App<I, C, S, C2, F, A>
where
    I: SystemInventory,
    C: Clock,
    S: PerformanceSampler,
    C2: Clock,
    F: SessionFile,
    A: SessionArchive,
{
    fn scan(&self, scope: Scope) -> Result<ScanReport, String> {
        self.scanner.scan(scope)
    }

    fn record(
        &mut self,
        options: WatchOptions,
        progress: &mut dyn FnMut(&str),
    ) -> Result<Recording, String> {
        self.recorder.run(options, "lol", progress)
    }

    fn sessions(&self) -> Result<Vec<SessionRow>, String> {
        self.archive.rows()
    }

    fn analyze(&self, id: Option<&str>) -> Result<Analysis, String> {
        self.archive.analyze(id)
    }

    fn compare(
        &self,
        before: Option<&str>,
        after: Option<&str>,
    ) -> Result<ComparisonReport, String> {
        self.archive.compare(before, after)
    }

    fn fixes(&self) -> Result<Vec<Proposal>, String> {
        self.fixer.proposals(self.archive.latest().as_ref())
    }

    fn apply_fix(&mut self, id: &str, confirmed: bool, dry_run: bool) -> Result<FixReport, String> {
        let latest = self.archive.latest();
        self.fixer.apply(id, latest.as_ref(), confirmed, dry_run)
    }

    fn undo_fix(&mut self, seq: u64, confirmed: bool) -> Result<FixReport, String> {
        self.fixer.undo(seq, confirmed)
    }

    fn history(&self) -> Result<Vec<HistoryEntry>, String> {
        self.fixer.history()
    }

    fn drivers(&self) -> Result<DriversView, String> {
        self.scanner
            .scan(Scope::Hardware)
            .map(|report| DriversView::build(&report))
    }

    fn overview(&self) -> Result<Overview, String> {
        let scan = self.scanner.scan(Scope::Hardware)?;
        Ok(Overview::build(&scan, self.archive.latest().as_ref()))
    }
}
