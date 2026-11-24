//! What the application can do, as one trait: the command line (and the MCP
//! server) ask for things through it, without knowing which ports are behind.
//! [`App`] is the real implementation, built from the services of this crate.

use std::time::Duration;

use purifex_domain::analysis::Analysis;
use purifex_domain::app_traces::Trace;
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
use purifex_domain::startup::StartupItem;

use crate::archive::Archive;
use crate::candidates::Candidates;
use crate::cleaning::{Cleaning, RemovalStart, Removals};
use crate::fixer::Fixer;
use crate::offering::Offering;
use crate::recorder::{Recorder, Recording, WatchOptions};
use crate::scanner::Scanner;
use crate::trace_search::TraceSearch;

/// The services of the application over real (or fake) ports.
pub struct App<I, C, S, C2, F, A> {
    scanner: Scanner<I, C>,
    recorder: Recorder<S, C2, F>,
    archive: Archive<A>,
    fixer: Fixer,
    offering: Offering,
    trace_search: TraceSearch,
    uninstall_prep: crate::uninstall_prep::UninstallPrep,
    candidates: Candidates,
    opener: Option<Box<dyn UrlOpener>>,
    folder_opener: Option<Box<dyn purifex_domain::ports::FolderOpener>>,
    registry_viewer: Option<Box<dyn purifex_domain::ports::RegistryViewer>>,
    app_list_store: Option<Box<dyn purifex_domain::ports::AppListStore>>,
    window_picker: Option<Box<dyn purifex_domain::ports::WindowPicker>>,
    inventory: Option<Box<dyn AppInventory>>,
    cleaning: Cleaning,
    monitoring: crate::monitoring::Monitoring,
    startup: Option<Box<dyn purifex_domain::ports::StartupSource>>,
    elevation: Option<Box<dyn Elevation>>,
}

mod builders;
mod contract;
mod startup;
mod tracing;

pub use contract::{Services, TracesPolled, UninstallPolled};

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

    fn app_still_registered(&self, id: &str) -> Result<bool, String> {
        self.registered(id)
    }

    fn apps(&self) -> Result<Vec<InstalledApp>, String> {
        let inventory = self
            .inventory
            .as_ref()
            .ok_or("esta versión no sabe leer las apps instaladas")?;
        Ok(listed(inventory.apps()?))
    }

    fn monitor_start(&mut self) -> Result<Option<String>, String> {
        let apps = self.apps().unwrap_or_default();
        self.monitoring.start(&apps)
    }

    fn monitor_finish(&mut self) -> Result<purifex_domain::monitor::InstallLog, String> {
        let apps = self.apps()?;
        let today = Some(self.today());
        self.monitoring.finish(&apps, today)
    }

    fn monitor_cancel(&mut self) {
        self.monitoring.cancel();
    }

    fn install_logs(&mut self) -> Result<Vec<purifex_domain::monitor::InstallLog>, String> {
        self.monitoring.logs()
    }

    fn delete_install_log(&mut self, id: &str) -> Result<(), String> {
        self.monitoring.delete(id)
    }

    fn export_install_log(&mut self, id: &str) -> Result<String, String> {
        self.monitoring.export(id)
    }

    fn import_install_log(
        &mut self,
    ) -> Result<Option<purifex_domain::monitor::InstallLog>, String> {
        self.monitoring.import()
    }

    fn startup_items(&mut self) -> Result<Vec<StartupItem>, String> {
        self.read_startup()
    }

    fn startup_quick(&mut self) -> Result<Vec<StartupItem>, String> {
        self.read_startup_quick()
    }

    fn set_startup(&mut self, id: &str, on: bool) -> Result<StartupItem, String> {
        self.change_startup(id, on)
    }

    fn remove_startup(&mut self, id: &str) -> Result<CleanupEntry, String> {
        self.take_out_startup(id)
    }

    fn junk_scan(&mut self) -> Result<Vec<purifex_domain::junk::JunkItem>, String> {
        self.cleaning.scan_junk()
    }

    fn junk_clean(
        &mut self,
        kinds: &[purifex_domain::junk::JunkKind],
    ) -> Result<purifex_domain::junk::JunkOutcome, String> {
        self.cleaning.clean_junk(kinds)
    }

    fn begin_batch(&mut self, count: usize) -> purifex_domain::safeguard::RestorePointOutcome {
        self.cleaning.begin_batch(count)
    }

    fn end_batch(&mut self) {
        self.cleaning.end_batch();
    }

    fn uninstall_app(&mut self, query: &str) -> Result<Started, String> {
        let apps = self.apps()?;
        self.cleaning.uninstall(&apps, query)
    }

    fn start_uninstall(&mut self, app: &InstalledApp) -> Result<(), String> {
        self.prepare(app)
    }

    fn poll_uninstall(&mut self) -> UninstallPolled {
        self.polled_uninstall()
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

    fn start_traces(&mut self, query: &str) -> Result<(), String> {
        self.search_traces(query)
    }

    fn poll_traces(&mut self) -> TracesPolled {
        self.polled_traces()
    }

    fn begin_removal(&mut self, query: &str) -> Result<RemovalStart, String> {
        self.start_removal(query)
    }

    fn remove_trace(
        &mut self,
        app: &str,
        trace: &Trace,
        start: &mut RemovalStart,
        done: &mut Removals,
    ) -> Result<(), String> {
        self.cleaning.remove_one(app, trace, start, done)
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
