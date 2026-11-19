//! What the application can do, as one trait: the command line (and the MCP
//! server) ask for things through it, without knowing which ports are behind.
//! [`App`] is the real implementation, built from the services of this crate.

use std::time::Duration;

use purifex_domain::analysis::Analysis;
use purifex_domain::apps::{listed, InstalledApp};
use purifex_domain::cleanup_log::{CleanReport, CleanupEntry};
use purifex_domain::compare::ComparisonReport;
use purifex_domain::driver_candidate::{is_official_page, Candidate, Submission};
use purifex_domain::driver_offers::OfferReport;
use purifex_domain::driver_tiers::report;
use purifex_domain::drivers_view::DriversView;
use purifex_domain::fix::{FixReport, HistoryEntry, Proposal};
use purifex_domain::overview::Overview;
use purifex_domain::ports::{
    AppInventory, Clock, PerformanceSampler, SessionArchive, SessionFile, SystemInventory,
    UrlOpener,
};
use purifex_domain::report::{ScanReport, Scope};
use purifex_domain::session_list::SessionRow;

use crate::archive::Archive;
use crate::candidates::Candidates;
use crate::cleaning::Cleaning;
use crate::fixer::Fixer;
use crate::offering::Offering;
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
    /// Starts asking Windows Update for newer drivers, in the background.
    fn start_offers(&mut self) -> Result<(), String>;
    /// Waits up to `wait` for that search. `None` = still going.
    fn poll_offers(&mut self, wait: Duration) -> Option<Result<OfferReport, String>>;
    /// Searches Windows Update and waits for it (the MCP server has no window to keep alive).
    fn unresolved(&mut self) -> Result<OfferReport, String>;
    /// Judges and keeps a proposal for a device (nothing is installed or opened).
    fn submit_candidate(
        &mut self,
        device: &str,
        url: &str,
        version: &str,
        title: &str,
    ) -> Result<Submission, String>;
    /// The proposals kept so far.
    fn candidates(&self) -> Result<Vec<Candidate>, String>;
    /// Opens the official page of a proposal in the browser (nothing is downloaded).
    fn open_page(&mut self, url: &str) -> Result<(), String>;
    /// The apps installed (the ones worth listing, by name).
    fn apps(&self) -> Result<Vec<InstalledApp>, String>;
    /// Starts the uninstaller of an app (by name or id).
    fn uninstall_app(&mut self, query: &str) -> Result<InstalledApp, String>;
    /// The traces of an app; with `confirmed` (and the app already uninstalled) it
    /// removes them to the quarantine, without it only says which.
    fn clean_app(&mut self, query: &str, confirmed: bool) -> Result<CleanReport, String>;
    /// Puts back what removal `seq` took out.
    fn restore_cleanup(&mut self, seq: u64) -> Result<CleanupEntry, String>;
    fn cleanup_history(&self) -> Result<Vec<CleanupEntry>, String>;
}

/// The services of the application over real (or fake) ports.
pub struct App<I, C, S, C2, F, A> {
    scanner: Scanner<I, C>,
    recorder: Recorder<S, C2, F>,
    archive: Archive<A>,
    fixer: Fixer,
    offering: Offering,
    candidates: Candidates,
    opener: Option<Box<dyn UrlOpener>>,
    inventory: Option<Box<dyn AppInventory>>,
    cleaning: Cleaning,
}

impl<I, C, S, C2, F, A> App<I, C, S, C2, F, A> {
    pub fn new(scanner: Scanner<I, C>, recorder: Recorder<S, C2, F>, archive: Archive<A>) -> Self {
        Self {
            scanner,
            recorder,
            archive,
            fixer: Fixer::disabled(),
            offering: Offering::default(),
            candidates: Candidates::default(),
            opener: None,
            inventory: None,
            cleaning: Cleaning::default(),
        }
    }

    /// What uninstalls apps and cleans what they leave (none until given).
    pub fn with_cleaning(mut self, cleaning: Cleaning) -> Self {
        self.cleaning = cleaning;
        self
    }

    /// Where the installed apps are read (none until given).
    pub fn with_apps(mut self, inventory: Box<dyn AppInventory>) -> Self {
        self.inventory = Some(inventory);
        self
    }

    /// What opens a web page (none until given).
    pub fn with_opener(mut self, opener: Box<dyn UrlOpener>) -> Self {
        self.opener = Some(opener);
        self
    }

    /// Where the proposals for drivers are kept (none until given).
    pub fn with_candidates(mut self, candidates: Candidates) -> Self {
        self.candidates = candidates;
        self
    }

    /// Where driver offers are searched (none until given).
    pub fn with_offers(mut self, offering: Offering) -> Self {
        self.offering = offering;
        self
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

    fn start_offers(&mut self) -> Result<(), String> {
        self.offering.start()
    }

    fn poll_offers(&mut self, wait: Duration) -> Option<Result<OfferReport, String>> {
        let found = self.offering.poll(wait)?;
        Some(found.and_then(|offers| {
            let devices: Vec<_> = self
                .scanner
                .devices()?
                .into_iter()
                .filter(|d| d.is_physical())
                .collect();
            Ok(report(&devices, &offers, self.scanner.today()))
        }))
    }

    fn unresolved(&mut self) -> Result<OfferReport, String> {
        self.offering.start()?;
        loop {
            if let Some(found) = self.poll_offers(Duration::from_millis(500)) {
                return found;
            }
        }
    }

    fn submit_candidate(
        &mut self,
        device: &str,
        url: &str,
        version: &str,
        title: &str,
    ) -> Result<Submission, String> {
        let devices: Vec<_> = self
            .scanner
            .devices()?
            .into_iter()
            .filter(|d| d.is_physical())
            .collect();
        self.candidates
            .submit(&devices, device, url, version, title)
    }

    fn candidates(&self) -> Result<Vec<Candidate>, String> {
        self.candidates.list()
    }

    fn open_page(&mut self, url: &str) -> Result<(), String> {
        // Whatever asked for it, only an official https page is ever opened.
        if !is_official_page(url) {
            return Err("solo se abren páginas https de fabricantes conocidos".into());
        }
        self.opener
            .as_ref()
            .ok_or("esta versión no puede abrir páginas")?
            .open(url)
    }

    fn apps(&self) -> Result<Vec<InstalledApp>, String> {
        let inventory = self
            .inventory
            .as_ref()
            .ok_or("esta versión no sabe leer las apps instaladas")?;
        Ok(listed(inventory.apps()?))
    }

    fn uninstall_app(&mut self, query: &str) -> Result<InstalledApp, String> {
        let apps = self.apps()?;
        self.cleaning.uninstall(&apps, query)
    }

    fn clean_app(&mut self, query: &str, confirmed: bool) -> Result<CleanReport, String> {
        let all = self
            .inventory
            .as_ref()
            .ok_or("esta versión no sabe leer las apps instaladas")?
            .apps()?;
        let listed = listed(all.clone());
        self.cleaning.clean(&listed, &all, query, confirmed)
    }

    fn restore_cleanup(&mut self, seq: u64) -> Result<CleanupEntry, String> {
        self.cleaning.restore(seq)
    }

    fn cleanup_history(&self) -> Result<Vec<CleanupEntry>, String> {
        self.cleaning.history()
    }
}
