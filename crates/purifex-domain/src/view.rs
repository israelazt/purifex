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
    /// How much of what the search finds is proposed (the window switches it).
    pub scan_mode: crate::scan_mode::ScanMode,
    /// Whether the program runs as an administrator (read on the apps page).
    pub elevated: Option<bool>,
    /// The window with the list of traces is open.
    pub traces_open: bool,
    /// Traces of the list the user unchecked (all start checked), by `Trace::key`.
    pub unchecked: Vec<String>,
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
    /// Change how much of what the search finds is proposed, and look again.
    SetScanMode(crate::scan_mode::ScanMode),
    SetFilter(DriverFilter),
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
        self.apps
            .iter()
            .flatten()
            .filter(|a| {
                query.is_empty()
                    || a.name.to_lowercase().contains(&query)
                    || a.publisher
                        .as_deref()
                        .unwrap_or("")
                        .to_lowercase()
                        .contains(&query)
            })
            .collect()
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
