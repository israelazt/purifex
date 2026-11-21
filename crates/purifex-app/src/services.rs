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
use purifex_domain::forced::{CloseReport, RunningProcess};
use purifex_domain::overview::Overview;
use purifex_domain::ports::{
    AppInventory, Clock, Elevation, PerformanceSampler, SessionArchive, SessionFile,
    SystemInventory, UrlOpener,
};
use purifex_domain::quarantine::{PurgeReport, QuarantineInfo, RebootReport, RestoreReport};
use purifex_domain::report::{ScanReport, Scope};
use purifex_domain::safeguard::{BackupEntry, Started};
use purifex_domain::scan_mode::ScanMode;
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
    /// Opens the install folder of an app in Explorer.
    fn open_folder(&mut self, folder: &str) -> Result<(), String>;
    /// What the person chose for the list of apps (the defaults when nothing was kept).
    fn app_list_settings(&self) -> purifex_domain::app_list::AppListSettings;
    /// Keeps that choice for the next run.
    fn save_app_list_settings(
        &mut self,
        settings: &purifex_domain::app_list::AppListSettings,
    ) -> Result<(), String>;
    /// Takes an app out without its uninstaller: its entry goes to the quarantine.
    fn force_uninstall(&mut self, query: &str) -> Result<Started, String>;
    /// Shows the registry key of an app in the registry editor.
    fn open_registry_key(&mut self, key: &str) -> Result<(), String>;
    /// Opens the web page an app registered (any `https` address).
    fn open_website(&mut self, url: &str) -> Result<(), String>;
    /// Today's day, by the clock of the program.
    fn today(&self) -> purifex_domain::date::Date;
    /// Starts the uninstaller of an app (by name or id), after asking Windows for a restore point.
    fn uninstall_app(&mut self, query: &str) -> Result<Started, String>;
    /// The traces of an app; with `confirmed` (and the app already uninstalled) it
    /// removes them to the quarantine, without it only says which.
    fn clean_app(
        &mut self,
        query: &str,
        confirmed: bool,
        only: Option<&[String]>,
    ) -> Result<CleanReport, String>;
    /// Puts back what removal `seq` took out.
    fn restore_cleanup(&mut self, seq: u64) -> Result<CleanupEntry, String>;
    fn cleanup_history(&self) -> Result<Vec<CleanupEntry>, String>;
    /// What the quarantine holds: how many traces, how big, how old.
    fn quarantine(&mut self) -> Result<QuarantineInfo, String>;
    /// How much of what the search finds is proposed from now on.
    fn set_scan_mode(&mut self, mode: ScanMode);
    /// Asks Windows to delete at the next restart what the app left open. It cannot be undone.
    fn delete_app_leftovers_at_reboot(&mut self, query: &str) -> Result<RebootReport, String>;
    /// The programs of an app that is gone and still run from its folder.
    fn running_programs(&mut self, query: &str) -> Result<Vec<RunningProcess>, String>;
    /// The program that owns the window under the pointer.
    fn window_under_pointer(&self) -> Result<Option<RunningProcess>, String>;
    /// Closes one program the sight landed on, if it is still the same one. The person must
    /// have confirmed.
    fn close_process(&mut self, expected: &RunningProcess) -> Result<(), String>;
    /// Closes them. The person must have confirmed: a program may lose what it had not saved.
    fn close_programs(&mut self, query: &str) -> Result<CloseReport, String>;
    /// Puts back every trace of the app that is still in the quarantine.
    fn restore_app_cleanup(&mut self, app: &str) -> Result<RestoreReport, String>;
    /// Deletes for good what the quarantine holds; without `confirmed` it only says how much.
    fn empty_quarantine(&mut self, confirmed: bool) -> Result<PurgeReport, String>;
    /// Lets go of what has been in the quarantine for seven days.
    fn expire_quarantine(&mut self) -> Result<PurgeReport, String>;
    /// The copies of the registration taken before uninstalling, newest first.
    fn backups(&self) -> Result<Vec<BackupEntry>, String>;
    /// Registers an app again from the copy kept in `registry_copy`.
    fn restore_backup(&mut self, registry_copy: &str) -> Result<BackupEntry, String>;
    /// Deletes that copy (after the person confirmed).
    fn delete_backup(&mut self, registry_copy: &str) -> Result<BackupEntry, String>;
    /// Whether the program runs as an administrator.
    fn is_elevated(&self) -> bool;
    /// Starts the program again as an administrator, looking for the traces of `resume`.
    fn restart_elevated(&mut self, resume: Option<&str>) -> Result<(), String>;
    /// An app was seen to disappear from the list: keep what was known of it.
    fn note_uninstalled(&mut self, app: &InstalledApp) -> Result<(), String>;
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
    folder_opener: Option<Box<dyn purifex_domain::ports::FolderOpener>>,
    registry_viewer: Option<Box<dyn purifex_domain::ports::RegistryViewer>>,
    app_list_store: Option<Box<dyn purifex_domain::ports::AppListStore>>,
    window_picker: Option<Box<dyn purifex_domain::ports::WindowPicker>>,
    inventory: Option<Box<dyn AppInventory>>,
    cleaning: Cleaning,
    elevation: Option<Box<dyn Elevation>>,
}

mod builders;

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

    fn today(&self) -> purifex_domain::date::Date {
        self.scanner.today()
    }

    fn open_folder(&mut self, folder: &str) -> Result<(), String> {
        self.folder_opener
            .as_ref()
            .ok_or("esta versión no puede abrir carpetas")?
            .open(folder)
    }

    fn app_list_settings(&self) -> purifex_domain::app_list::AppListSettings {
        self.app_list_store
            .as_ref()
            .map(|s| s.load())
            .unwrap_or_default()
    }

    fn save_app_list_settings(
        &mut self,
        settings: &purifex_domain::app_list::AppListSettings,
    ) -> Result<(), String> {
        self.app_list_store
            .as_ref()
            .ok_or("esta versión no guarda la vista")?
            .save(settings)
    }

    fn force_uninstall(&mut self, query: &str) -> Result<Started, String> {
        let (listed, _) = self.installed()?;
        self.cleaning.force_uninstall(&listed, query)
    }

    fn open_registry_key(&mut self, key: &str) -> Result<(), String> {
        self.registry_viewer
            .as_ref()
            .ok_or("esta versión no puede abrir el editor del registro")?
            .show(key)
    }

    fn open_website(&mut self, url: &str) -> Result<(), String> {
        if !url.starts_with("https://") {
            return Err("solo se abren direcciones https".into());
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

    fn uninstall_app(&mut self, query: &str) -> Result<Started, String> {
        let apps = self.apps()?;
        self.cleaning.uninstall(&apps, query)
    }

    fn clean_app(
        &mut self,
        query: &str,
        confirmed: bool,
        only: Option<&[String]>,
    ) -> Result<CleanReport, String> {
        let (listed, all) = self.installed()?;
        self.cleaning.clean(&listed, &all, query, confirmed, only)
    }

    fn set_scan_mode(&mut self, mode: ScanMode) {
        self.cleaning.set_mode(mode);
    }

    fn delete_app_leftovers_at_reboot(&mut self, query: &str) -> Result<RebootReport, String> {
        let (listed, all) = self.installed()?;
        self.cleaning.delete_at_reboot(&listed, &all, query)
    }

    fn running_programs(&mut self, query: &str) -> Result<Vec<RunningProcess>, String> {
        let (listed, all) = self.installed()?;
        self.cleaning.running_of(&listed, &all, query)
    }

    fn window_under_pointer(&self) -> Result<Option<RunningProcess>, String> {
        self.window_picker
            .as_ref()
            .ok_or("esta versión no tiene el Modo Cazador")?
            .under_pointer()
    }

    fn close_process(&mut self, expected: &RunningProcess) -> Result<(), String> {
        self.cleaning.close_one(expected)
    }

    fn close_programs(&mut self, query: &str) -> Result<CloseReport, String> {
        let (listed, all) = self.installed()?;
        self.cleaning.close_running(&listed, &all, query)
    }

    fn restore_cleanup(&mut self, seq: u64) -> Result<CleanupEntry, String> {
        self.cleaning.restore(seq)
    }

    fn cleanup_history(&self) -> Result<Vec<CleanupEntry>, String> {
        self.cleaning.history()
    }

    fn quarantine(&mut self) -> Result<QuarantineInfo, String> {
        self.cleaning.quarantine_info()
    }

    fn restore_app_cleanup(&mut self, app: &str) -> Result<RestoreReport, String> {
        self.cleaning.restore_app(app)
    }

    fn backups(&self) -> Result<Vec<BackupEntry>, String> {
        self.cleaning.backups()
    }

    fn delete_backup(&mut self, registry_copy: &str) -> Result<BackupEntry, String> {
        self.cleaning.delete_backup(registry_copy)
    }

    fn restore_backup(&mut self, registry_copy: &str) -> Result<BackupEntry, String> {
        self.cleaning.restore_backup(registry_copy)
    }

    fn empty_quarantine(&mut self, confirmed: bool) -> Result<PurgeReport, String> {
        self.cleaning.empty_quarantine(confirmed)
    }

    fn expire_quarantine(&mut self) -> Result<PurgeReport, String> {
        self.cleaning.expire_quarantine()
    }

    fn is_elevated(&self) -> bool {
        self.elevation.as_ref().is_some_and(|e| e.is_elevated())
    }

    fn restart_elevated(&mut self, resume: Option<&str>) -> Result<(), String> {
        let elevation = self
            .elevation
            .as_ref()
            .ok_or("esta versión no sabe reiniciarse como administrador")?;
        elevation.restart_elevated(resume)
    }

    fn note_uninstalled(&mut self, app: &InstalledApp) -> Result<(), String> {
        self.cleaning.remember(app)
    }
}
