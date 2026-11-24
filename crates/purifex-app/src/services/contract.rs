//! The contract: everything the window, the command line and the MCP server may ask for.

use purifex_domain::app_traces::Trace;
use purifex_domain::junk::{JunkItem, JunkKind, JunkOutcome};
use purifex_domain::monitor::InstallLog;
use purifex_domain::startup::StartupItem;

use super::*;

/// What preparing an uninstall, started with `start_uninstall`, has done since the last poll.
pub enum UninstallPolled {
    /// None is going.
    Idle,
    /// Still preparing; these parts have finished.
    Working(Vec<purifex_domain::prep_step::PrepStep>),
    /// The uninstaller was started (or it could not be).
    Done(Result<Box<Started>, String>),
}

/// What the search for traces started with `start_traces` has done since the last poll.
pub enum TracesPolled {
    /// None is running.
    Idle,
    /// Still looking; these parts of the search have finished.
    Working(Vec<purifex_domain::search_step::SearchStep>),
    Done(Result<CleanReport, String>),
}

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
    /// Whether an app is still registered, without reading the whole list (`Err`: only the list
    /// can tell).
    fn app_still_registered(&self, id: &str) -> Result<bool, String>;
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
    /// Asks Windows for one restore point for a batch of `count` apps: the uninstallers that
    /// follow, until `end_batch`, do not ask again.
    fn begin_batch(&mut self, count: usize) -> purifex_domain::safeguard::RestorePointOutcome;
    /// The batch is over: each uninstaller asks for its own restore point again.
    fn end_batch(&mut self);
    /// Starts the uninstaller of an app (by name or id), after asking Windows for a restore point.
    fn uninstall_app(&mut self, query: &str) -> Result<Started, String>;
    /// Starts preparing to uninstall an app the window already lists, without waiting. The
    /// uninstaller is started by `poll_uninstall` once the safety nets are ready.
    fn start_uninstall(&mut self, app: &InstalledApp) -> Result<(), String>;
    /// What the preparation has done; it never waits for it.
    fn poll_uninstall(&mut self) -> UninstallPolled;
    /// The traces of an app; with `confirmed` (and the app already uninstalled) it
    /// removes them to the quarantine, without it only says which.
    fn clean_app(
        &mut self,
        query: &str,
        confirmed: bool,
        only: Option<&[String]>,
    ) -> Result<CleanReport, String>;
    /// Starts looking for the traces of an app without waiting for the result (a new search
    /// replaces the one going). The result comes by `poll_traces`.
    fn start_traces(&mut self, query: &str) -> Result<(), String>;
    /// What the search of `start_traces` has done; it never waits.
    fn poll_traces(&mut self) -> TracesPolled;
    /// Checks that the app is gone before its traces are moved one by one with `remove_trace`.
    fn begin_removal(&mut self, query: &str) -> Result<RemovalStart, String>;
    /// Moves one trace that was shown to the quarantine, without looking again.
    fn remove_trace(
        &mut self,
        app: &str,
        trace: &Trace,
        start: &mut RemovalStart,
        done: &mut Removals,
    ) -> Result<(), String>;
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
    /// Asks for an installer, takes the picture before and starts it (`None`: cancelled).
    fn monitor_start(&mut self) -> Result<Option<String>, String>;
    /// The installer is done: takes the picture after and keeps what is new as a log.
    fn monitor_finish(&mut self) -> Result<InstallLog, String>;
    /// Gives up the watching without keeping anything.
    fn monitor_cancel(&mut self);
    /// The logs of installations kept, the newest first.
    fn install_logs(&mut self) -> Result<Vec<InstallLog>, String>;
    fn delete_install_log(&mut self, id: &str) -> Result<(), String>;
    /// Writes a log to the Documents folder; returns where.
    fn export_install_log(&mut self, id: &str) -> Result<String, String>;
    /// Asks for a log file and keeps it (as one that was not made here); `None`: cancelled.
    fn import_install_log(&mut self) -> Result<Option<InstallLog>, String>;
    /// What starts with Windows, each entry with the app it runs from when it is one.
    fn startup_items(&mut self) -> Result<Vec<StartupItem>, String>;
    /// The same without the tasks (they take seconds to read).
    fn startup_quick(&mut self) -> Result<Vec<StartupItem>, String>;
    /// Turns an entry on or off without deleting it; returns it as it is now.
    fn set_startup(&mut self, id: &str, on: bool) -> Result<StartupItem, String>;
    /// Takes an entry out to the quarantine (it can be put back); returns what was kept.
    fn remove_startup(&mut self, id: &str) -> Result<CleanupEntry, String>;
    /// The junk of Windows found now, by kind.
    fn junk_scan(&mut self) -> Result<Vec<JunkItem>, String>;
    /// Moves the junk of these kinds to the quarantine.
    fn junk_clean(&mut self, kinds: &[JunkKind]) -> Result<JunkOutcome, String>;
}
