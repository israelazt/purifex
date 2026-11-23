//! What the interface shows (`Model`) and what it can ask for (`Action`): the
//! whole contract between the interface and the application. The interface only
//! reads the model and returns actions; the application decides what each one
//! does. Neither knows the other.

use crate::analysis::Analysis;
use crate::apps::InstalledApp;
use crate::cleanup_log::CleanReport;
use crate::compare::ComparisonReport;
use crate::device_category::Category;
use crate::driver_candidate::Candidate;
use crate::driver_offers::Offers;
use crate::drivers_view::{DriverRow, DriversView};
use crate::fix::{FixReport, HistoryEntry, Proposal};
use crate::overview::Overview;
use crate::quarantine::QuarantineInfo;
use crate::session_list::SessionRow;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Page {
    #[default]
    Overview,
    Drivers,
    Sessions,
    Fixes,
    Apps,
    Startup,
    Junk,
}

/// What the user is being asked to confirm before something is changed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Confirm {
    /// Apply the fix with this id.
    Apply(String),
    /// Undo the change with this number of the history.
    Undo(u64),
    /// Start the uninstaller of the app with this id.
    Uninstall(String),
    /// Remove the checked traces of the app with this name (to the quarantine).
    Clean(String),
    /// Remove every trace of the app with this name, with no selecting.
    CleanAll(String),
    /// Delete for good what the quarantine holds.
    EmptyQuarantine,
    /// Start the program again as an administrator, to look again for the traces of the
    /// app with this name.
    RestartAsAdmin(String),
    /// Ask Windows to delete, at the next restart, what the app left open. Cannot be undone.
    DeleteAtReboot(String),
    /// Take the app out without its own uninstaller (it fails or is gone).
    ForceUninstall(String),
    /// Delete the copy of the registration of an app (by the file of the copy).
    DeleteBackup(String),
    /// Close the programs an app that is gone still runs from its folder.
    CloseProcesses(String),
    /// Close the program (by its pid) the sight was dropped on.
    CloseHunted(u32),
    /// Uninstall the checked apps one after the other.
    UninstallMarked,
    /// Delete a log of an installation (by its id).
    DeleteLog(String),
    /// Take an entry out of what starts with Windows (by its id), to the quarantine.
    RemoveStartup(String),
    /// Move the checked kinds of junk to the quarantine.
    CleanJunk,
}

/// The steps of getting rid of an app, shown in one window: uninstall, look for what it left,
/// remove it, done. Which step it is on is read from the rest of the model (an uninstaller being
/// waited for, a search going, a list of traces, a result), so it can never disagree with it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Wizard {
    pub app: String,
    /// What the removal did, once it was done: the last step.
    pub done: Option<String>,
}

impl Wizard {
    pub fn new(app: impl Into<String>) -> Self {
        Self {
            app: app.into(),
            done: None,
        }
    }
}

/// Programs of an app that is gone that still run from its folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Running {
    pub app: String,
    /// The file names of their `.exe`, without repeats.
    pub names: Vec<String>,
}

/// The leftovers of an app that could not be moved because a program has them open.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InUse {
    pub app: String,
    pub items: usize,
}

/// Which devices the drivers table lists.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DriverFilter {
    #[default]
    All,
    /// Only what needs attention (no driver, a fault).
    Attention,
    Category(Category),
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Model {
    pub page: Page,
    pub overview: Option<Overview>,
    pub drivers: Option<DriversView>,
    pub sessions: Option<Vec<SessionRow>>,
    pub analysis: Option<Analysis>,
    /// The chosen game against the game before it, when there is one.
    pub comparison: Option<ComparisonReport>,
    /// The fixes worth offering and the changes made so far.
    pub fixes: Option<Vec<Proposal>>,
    pub history: Option<Vec<HistoryEntry>>,
    /// Waiting for the user to confirm a change.
    pub confirm: Option<Confirm>,
    /// What the last change did (shown until the user goes elsewhere).
    pub notice: Option<FixReport>,
    /// The recording shown in the Sessions page.
    pub selected: Option<String>,
    pub filter: DriverFilter,
    /// Which apps the list of apps shows, and by which column it is ordered.
    pub app_filter: crate::app_list::AppFilter,
    pub app_sort: crate::app_list::AppSort,
    pub app_view: crate::app_list::AppView,
    /// The app whose details are open in the list (its id).
    pub app_selected: Option<String>,
    /// Today, as read with the apps (to judge «installed recently»).
    pub today: Option<crate::date::Date>,
    pub query: String,
    /// What Windows Update offers (only asked when the user asks).
    pub offers: Offers,
    /// Driver pages proposed by the user's AI (read on the drivers page).
    pub candidates: Option<Vec<Candidate>>,
    /// The installed apps (read on the apps page).
    pub apps: Option<Vec<InstalledApp>>,
    /// The app whose uninstaller was started here (its traces can be looked for).
    pub uninstalled: Option<String>,
    /// The traces found for it (nothing removed yet).
    pub traces: Option<CleanReport>,
    /// The name of an app whose traces are being looked for.
    pub finding: Option<String>,
    /// An uninstaller started here is running: Purifex waits for the app to disappear.
    pub waiting: bool,
    /// What the quarantine holds (read on the apps page).
    pub quarantine: Option<QuarantineInfo>,
    /// The copies of the registration of apps taken before uninstalling them (read on the
    /// apps page), newest first.
    pub backups: Option<Vec<crate::safeguard::BackupEntry>>,
    /// Leftovers of an app that a program has open: they were not removed, and can be left to
    /// Windows to delete at the next restart.
    pub in_use: Option<InUse>,
    /// Programs of the app being cleaned that are still running (closing them lets go of its files).
    pub running: Option<Running>,
    /// Hunter mode: the sight being dragged and what it landed on.
    pub hunt: crate::hunter::Hunt,
    /// How much of what the search finds is proposed (the window switches it).
    pub scan_mode: crate::scan_mode::ScanMode,
    /// Whether the program runs as an administrator (read on the apps page).
    pub elevated: Option<bool>,
    /// The window with the list of traces is open.
    pub traces_open: bool,
    /// Traces of the list the user unchecked (all start checked), by `Trace::key`.
    pub unchecked: Vec<String>,
    /// What the last removal could not take, by `Trace::key`, with why. It starts unchecked and
    /// «everything» leaves it alone, so the same list does not come back to fail again.
    pub stuck: Vec<(String, String)>,
    /// Apps checked in the list to uninstall together (their ids), in the order checked.
    pub marked: Vec<String>,
    /// The batch being run, or the summary of the one that just finished.
    pub batch: Option<crate::batch::Batch>,
    /// What starts with Windows without the tasks (read with the apps): tells which apps start
    /// with it and which entry a program pointed at belongs to.
    pub startup_quick: Option<Vec<crate::startup::StartupItem>>,
    /// What starts with Windows (read on the Startup page).
    pub startup: Option<Vec<crate::startup::StartupItem>>,
    /// The junk found (read on the Junk page).
    pub junk: Option<Vec<crate::junk::JunkItem>>,
    /// The kinds of junk that are left unchecked (all start checked).
    pub junk_unchecked: Vec<crate::junk::JunkKind>,
    /// The installer being watched (its file name): the picture before was taken.
    pub monitoring: Option<String>,
    /// The logs of installations kept (read on the apps page), newest first.
    pub install_logs: Option<Vec<crate::monitor::InstallLog>>,
    /// The log whose contents are open (its id).
    pub log_open: Option<String>,
    /// The window that walks through removing an app, when it is open.
    pub wizard: Option<Wizard>,
    /// What an apps action did, in a sentence.
    pub message: Option<String>,
    /// Something is being read: the picture says so before the wait.
    pub busy: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    Go(Page),
    /// Read everything again.
    Rescan,
    /// Ask Windows Update for newer drivers.
    SearchUpdates,
    /// Open the official page of a proposed driver in the browser.
    OpenPage(String),
    /// Asks to uninstall an app (by id): the user is asked to confirm.
    AskUninstall(String),
    /// Look for the traces of an app that was uninstalled (by name).
    FindTraces(String),
    /// The window came back to the front: read the apps again (the user may have
    /// uninstalled one meanwhile).
    RefreshApps,
    /// Stop waiting for the uninstaller that was started here.
    StopWaiting,
    /// Register an app again from the copy kept in this file.
    RestoreBackup(String),
    /// Asks to remove the checked traces: the user is asked to confirm.
    AskClean(String),
    /// Asks to remove every trace without selecting: same, it must be confirmed.
    AskCleanAll(String),
    /// Checks or unchecks one place of the list of traces.
    ToggleTrace(String),
    /// Checks all of them (`true`) or none.
    SelectTraces(bool),
    /// Opens the window of traces again.
    OpenTraces,
    /// Closes the window of traces without removing anything.
    CloseTraces,
    /// Closes the window that walks through removing an app (a wait or a search going on goes on).
    CloseWizard,
    /// Asks to empty the quarantine: the user is asked to confirm, it cannot be undone.
    AskEmptyQuarantine,
    /// Puts back the traces of this app that are in the quarantine (it never overwrites,
    /// so it needs no question).
    RestoreApp(String),
    /// Asks to restart as an administrator (to remove what needs it): the user is asked
    /// to confirm, and Windows asks once more.
    AskRestartAsAdmin(String),
    /// Asks to leave the open leftovers of this app to Windows, to delete at the next restart.
    AskDeleteAtReboot(String),
    /// Asks to close the programs of this app that are still running.
    AskCloseProcesses(String),
    /// Change how much of what the search finds is proposed, and look again.
    SetScanMode(crate::scan_mode::ScanMode),
    /// The sight was picked up (the button is held).
    StartHunt,
    /// The sight was let go: the program under the pointer is looked up.
    DropHunt,
    /// Closes the window that says what the sight landed on.
    CloseHunt,
    /// Asks to close the program the sight landed on.
    AskCloseHunted,
    /// Opens the folder of the program the sight landed on.
    OpenHuntedFolder,
    SetFilter(DriverFilter),
    SetAppFilter(crate::app_list::AppFilter),
    /// Shows every app again: no filter and no search.
    ClearAppFilters,
    SetAppSort(crate::app_list::AppSort),
    SetAppView(crate::app_list::AppView),
    /// Asks to delete a copy of the registration, by its file.
    AskDeleteBackup(String),
    /// Puts back everything of an app that was cleaned: its traces from the quarantine and its
    /// registration from the copy (the app, and the file of the copy).
    RestoreAll(String, String),
    /// Checks or unchecks a kind of junk.
    ToggleJunk(crate::junk::JunkKind),
    /// Asks to move the checked junk to the quarantine.
    AskCleanJunk,
    /// Turns an entry of the startup list on or off, by its id.
    ToggleStartup(String),
    /// Asks to take an entry out of the startup list, by its id.
    AskRemoveStartup(String),
    /// Opens the folder of the program an entry runs, by its id.
    OpenStartupFolder(String),
    /// Chooses an installer, takes the picture of the system and starts it.
    MonitorStart,
    /// The installer is done: takes the picture after and keeps what changed as a log.
    MonitorFinish,
    /// Stops watching the installer; nothing is kept.
    MonitorCancel,
    /// Opens (or closes) what a log holds, by its id.
    OpenLog(String),
    /// Writes a log to the Documents folder, by its id.
    ExportLog(String),
    /// Asks for a log file and keeps it.
    ImportLog,
    /// Asks to delete a log, by its id.
    AskDeleteLog(String),
    /// Checks or unchecks an app of the list for the batch, by its id.
    ToggleMark(String),
    /// Unchecks every app of the list.
    ClearMarks,
    /// Asks to uninstall the checked apps one after the other: the user is asked to confirm.
    AskUninstallMarked,
    /// Closes the summary of the batch.
    CloseBatch,
    /// Asks to force the removal of an installed app, by its id.
    AskForceUninstall(String),
    /// Opens (or closes) the details of an app of the list, by its id.
    SelectApp(String),
    /// Opens the install folder of an app in Explorer.
    OpenFolder(String),
    /// Shows the registry key of an app (by its id) in the registry editor.
    OpenRegistryKey(String),
    /// Opens the web page an app registered.
    OpenWebsite(String),
    SetQuery(String),
    SelectSession(String),
    /// Asks to apply a fix: nothing changes yet, the user is asked to confirm.
    AskFix(String),
    /// Asks to undo a change of the history: same, it must be confirmed.
    AskUndo(u64),
    /// Carries out what is waiting for confirmation.
    Confirm,
    CancelConfirm,
}

impl Model {
    /// The apps the page lists, by name; the search filters by name and publisher.
    pub fn visible_apps(&self) -> Vec<&InstalledApp> {
        let query = self.query.trim().to_lowercase();
        let mut apps: Vec<&InstalledApp> = self
            .apps
            .iter()
            .flatten()
            .filter(|a| self.app_filter.allows(a, self.today))
            .filter(|a| {
                query.is_empty()
                    || a.name.to_lowercase().contains(&query)
                    || a.publisher
                        .as_deref()
                        .unwrap_or("")
                        .to_lowercase()
                        .contains(&query)
            })
            .collect();
        self.app_sort.order(&mut apps);
        apps
    }

    /// The proposal of the user's AI for a device, the newest one.
    pub fn candidate_for(&self, instance_id: &str) -> Option<&Candidate> {
        self.candidates
            .as_ref()?
            .iter()
            .rev()
            .find(|c| c.device == instance_id)
    }

    /// The rows of the drivers table that the filter and the search let through.
    pub fn visible_drivers(&self) -> Vec<&DriverRow> {
        let Some(view) = &self.drivers else {
            return Vec::new();
        };
        let query = self.query.trim().to_lowercase();
        view.rows
            .iter()
            .filter(|row| match self.filter {
                DriverFilter::All => true,
                DriverFilter::Attention => row.needs_attention(),
                DriverFilter::Category(category) => row.category == category,
            })
            .filter(|row| query.is_empty() || row.matches(&query))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::date::Date;
    use crate::device::{Device, InstalledDriver};
    use crate::report::{ScanReport, Scope};
    use crate::version::DriverVersion;

    fn device(id: &str, class: &str, name: &str, provider: &str, problem: Option<u32>) -> Device {
        Device {
            instance_id: id.into(),
            class: class.into(),
            name: name.into(),
            problem,
            driver: Some(InstalledDriver {
                provider: Some(provider.into()),
                version: DriverVersion::parse("1.0"),
                date: Date::new(2026, 1, 1),
                inf: None,
            }),
            ..Device::default()
        }
    }

    fn model() -> Model {
        let report = ScanReport::build(
            vec![
                device(
                    r"PCI\VEN_10DE&DEV_2182",
                    "Display",
                    "NVIDIA GeForce GTX 1660 Ti",
                    "NVIDIA",
                    None,
                ),
                device(
                    r"USB\VID_0BDA",
                    "Bluetooth",
                    "Realtek Bluetooth",
                    "Realtek",
                    Some(31),
                ),
                device(
                    r"HDAUDIO\FUNC_01",
                    "MEDIA",
                    "Realtek Audio",
                    "Realtek",
                    None,
                ),
            ],
            Scope::Hardware,
            Date::new(2026, 11, 18).unwrap(),
            5,
        );
        Model {
            drivers: Some(DriversView::build(&report)),
            ..Model::default()
        }
    }

    fn names(m: &Model) -> Vec<&str> {
        m.visible_drivers()
            .iter()
            .map(|r| r.name.as_str())
            .collect()
    }

    #[test]
    fn with_no_filter_every_row_shows_and_what_needs_attention_is_first() {
        assert_eq!(names(&model())[0], "Realtek Bluetooth");
        assert_eq!(names(&model()).len(), 3);
        assert!(
            Model::default().visible_drivers().is_empty(),
            "nothing read yet"
        );
    }

    #[test]
    fn a_filter_keeps_only_its_devices() {
        let mut m = model();
        m.filter = DriverFilter::Attention;
        assert_eq!(names(&m), ["Realtek Bluetooth"]);
        m.filter = DriverFilter::Category(Category::Gpu);
        assert_eq!(names(&m), ["NVIDIA GeForce GTX 1660 Ti"]);
        m.filter = DriverFilter::Category(Category::Storage);
        assert!(names(&m).is_empty());
    }

    #[test]
    fn the_search_looks_in_the_name_the_id_the_provider_and_the_class_whatever_the_case() {
        let mut m = model();
        for (query, expected) in [
            ("nvidia", 1),
            ("VEN_10DE", 1),
            ("realtek", 2),
            ("media", 1),
            ("  audio ", 1),
            ("zzz", 0),
        ] {
            m.query = query.into();
            assert_eq!(names(&m).len(), expected, "{query:?}");
        }
    }

    #[test]
    fn the_filter_and_the_search_work_together() {
        let mut m = model();
        m.filter = DriverFilter::Category(Category::Audio);
        m.query = "realtek".into();
        assert_eq!(names(&m), ["Realtek Audio"]);
    }
}
