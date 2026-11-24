//! What the rest of the program needs from the system, said by what it needs
//! and not by how Windows (or another system) gives it. The real adapters live
//! in `purifex-platform`; the tests use fakes.

use std::time::Duration;

use crate::date::Date;
use crate::device::Device;
use crate::draw::{Size, TextStyle};
use crate::fix::{HistoryEntry, SettingValue};
use crate::sample::Sample;

/// The devices present in the computer, with the driver each one runs.
pub trait SystemInventory {
    /// Every present device. Reading it changes nothing. A device whose data
    /// cannot be read is still listed, with what could be read.
    fn devices(&self) -> Result<Vec<Device>, String>;
}

/// What Windows Update offers for the drivers of this computer. Asking goes out to
/// the network and can take a minute, so it is only done when the user asks, and
/// off the thread of the interface.
pub trait DriverOffers: Send + Sync {
    fn search(&self) -> Result<Vec<crate::driver_offers::Offer>, String>;
}

/// The driver candidates that were proposed, kept between runs (the MCP server
/// writes them, the window reads them). Lines are only added.
pub trait CandidateStore {
    fn all(&self) -> Result<Vec<crate::driver_candidate::Candidate>, String>;
    fn add(&mut self, candidate: &crate::driver_candidate::Candidate) -> Result<(), String>;
}

/// Opens a web page in the user's browser. Nothing is downloaded by Purifex.
pub trait UrlOpener {
    fn open(&self, url: &str) -> Result<(), String>;
}

/// Opens a folder in the file explorer. Nothing is run: only a folder that exists is opened.
pub trait FolderOpener {
    fn open(&self, folder: &str) -> Result<(), String>;
}

/// Where the choices of the person for the list of apps are kept between runs.
pub trait AppListStore {
    /// What was saved (the defaults when nothing was).
    fn load(&self) -> crate::app_list::AppListSettings;
    fn save(&self, settings: &crate::app_list::AppListSettings) -> Result<(), String>;
}

/// Shows a key of the registry in the registry editor. Nothing is changed in the registry.
pub trait RegistryViewer {
    /// `key` is a full path: `HKEY_LOCAL_MACHINE\...` or `HKEY_CURRENT_USER\...`.
    fn show(&self, key: &str) -> Result<(), String>;
}

/// The programs running now. Listing changes nothing; closing ends one of them.
pub trait Processes {
    /// Every running program whose `.exe` can be read.
    fn running(&self) -> Result<Vec<crate::forced::RunningProcess>, String>;
    /// Ends one program. It may lose what it had not saved.
    fn close(&mut self, pid: u32) -> Result<(), String>;
}

/// The window under the pointer, for hunter mode. Looking changes nothing.
pub trait WindowPicker {
    /// The program that owns the window under the pointer, or `None` when there is none (or it
    /// cannot be read).
    fn under_pointer(&self) -> Result<Option<crate::forced::RunningProcess>, String>;
}

/// The apps installed in the computer. Reading it changes nothing.
pub trait AppInventory {
    fn apps(&self) -> Result<Vec<crate::apps::InstalledApp>, String>;
    /// Whether the app is still registered, opening only what is its own (reading the whole
    /// list takes half a second). `Err` when this inventory cannot say cheaply: read the list.
    fn still_registered(&self, id: &str) -> Result<bool, String>;
}

/// Looks for places that might hold the traces of an app. It only reads and
/// proposes; `app_traces::judge` decides.
pub trait TraceFinder: Send + Sync {
    fn candidates(
        &self,
        app: &crate::apps::InstalledApp,
    ) -> Result<Vec<crate::app_traces::Candidate>, String>;

    /// The candidates and the seeds they were looked for with, which can be more than
    /// the app's own (extensions found in the registry, programs seen in its traces):
    /// the rules must judge with the same seeds. By default, the app's own.
    fn search(&self, app: &crate::apps::InstalledApp) -> Result<Found, String> {
        Ok(Found {
            seeds: crate::seeds::seeds_of(app),
            candidates: self.candidates(app)?,
            ..Found::default()
        })
    }

    /// The search, saying when each part of it has finished (for a window that shows progress).
    /// By default, the whole search and then every part at once.
    fn search_reporting(
        &self,
        app: &crate::apps::InstalledApp,
        step: &dyn Fn(crate::search_step::SearchStep),
    ) -> Result<Found, String> {
        let found = self.search(app);
        crate::search_step::SearchStep::ALL
            .into_iter()
            .for_each(step);
        found
    }
}

/// What a search found, and the seeds it used.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Found {
    pub seeds: crate::seeds::Seeds,
    pub candidates: Vec<crate::app_traces::Candidate>,
    /// The search ran out of time and may have missed some candidates.
    pub incomplete: bool,
    /// Profiles of other people of this computer that could not be read (only an
    /// administrator can): the search did not look in them.
    pub other_users_unread: usize,
}

/// Looks at an app that is still installed and says what its traces will be
/// recognised by once it is gone: its programs and the extensions it registered.
pub trait SeedProbe: Send + Sync {
    fn observe(&self, app: &crate::apps::InstalledApp) -> crate::seeds::Seeds;
}

/// Starts the uninstaller that the app registered (and does not wait for it).
pub trait Uninstaller {
    fn launch(&self, app: &crate::apps::InstalledApp) -> Result<(), String>;
}

/// Takes traces out of their place into a quarantine that can give them back, and
/// keeps the log of what it did.
pub trait CleanupStore {
    /// Moves the trace to the quarantine; returns where it is kept.
    fn remove(&mut self, trace: &crate::app_traces::Trace, seq: u64) -> Result<String, String>;
    /// Whether a program has the file, or a file inside the folder, open right now. It is what
    /// tells "in use" (it can wait for the next restart) from the other reasons not to remove.
    fn is_locked(&self, trace: &crate::app_traces::Trace) -> bool;
    /// Asks Windows to delete the file or folder at the next restart, for what a program has
    /// open and cannot be moved. Nothing is kept: it cannot be put back. Needs an administrator.
    fn delete_at_reboot(&mut self, trace: &crate::app_traces::Trace) -> Result<(), String>;
    /// Puts the removed thing back where it was (it must not exist there now).
    fn restore(&mut self, entry: &crate::cleanup_log::CleanupEntry) -> Result<(), String>;
    fn entries(&self) -> Result<Vec<crate::cleanup_log::CleanupEntry>, String>;
    fn append(&mut self, entry: &crate::cleanup_log::CleanupEntry) -> Result<(), String>;
    /// Keeps what was known of an app when its uninstaller was started (afterwards
    /// the system no longer lists it, and its traces are looked for from this).
    fn remember(&mut self, app: &crate::apps::InstalledApp) -> Result<(), String>;
    fn remembered(&self) -> Result<Vec<crate::apps::InstalledApp>, String>;
    /// Deletes for good the quarantine copy of a removal and returns the bytes freed. It only
    /// ever deletes inside the quarantine folder: a copy that is not there is refused.
    fn purge(&mut self, entry: &crate::cleanup_log::CleanupEntry) -> Result<u64, String>;
    /// What the quarantine takes on disk.
    fn quarantine_bytes(&self) -> Result<u64, String>;
}

/// A copy of what an app registered, kept before its uninstaller runs: the system forgets it
/// afterwards, and with the copy it can be registered again.
pub trait Backups {
    /// Saves the registration of the app and says where; `None` when it has none to save.
    fn save(&self, app: &crate::apps::InstalledApp) -> Result<Option<String>, String>;
    /// Writes down that a copy was taken (the log of copies only ever grows).
    fn record(&self, entry: &crate::safeguard::BackupEntry) -> Result<(), String>;
    /// The copies that were taken, oldest first.
    fn entries(&self) -> Result<Vec<crate::safeguard::BackupEntry>, String>;
    /// Registers the app again from a copy of the log (`reg import`). It refuses when the key
    /// exists: the app is registered, and importing would merge the old values into it.
    fn restore(&self, entry: &crate::safeguard::BackupEntry) -> Result<(), String>;
    /// Deletes a copy of the log: its file and its line. The registration of the app can no
    /// longer be put back with it.
    fn delete(&self, entry: &crate::safeguard::BackupEntry) -> Result<(), String>;
}

/// The restore points of Windows: the way back for the whole system, taken before an
/// uninstaller starts.
pub trait RestorePoints: Send + Sync {
    /// Asks Windows for a restore point. `Err` says why not (it needs an administrator).
    fn create(&self, description: &str) -> Result<(), String>;
}

/// What the program may do about its own permissions. Removing some traces (the registry
/// of the machine, Program Files) needs an administrator; the program runs without one and
/// only asks to start again with it when the person agrees.
pub trait Elevation {
    /// Whether the program is running as an administrator.
    fn is_elevated(&self) -> bool;
    /// Starts the program again as an administrator (Windows asks the person) opening on the
    /// apps page and looking for the traces of `resume`. `Err` if the person said no.
    fn restart_elevated(&self, resume: Option<&str>) -> Result<(), String>;
}

/// The clock: today's day, and a monotonic time to measure how long things take.
pub trait Clock {
    fn today(&self) -> Date;
    /// Time since an arbitrary start; only differences mean something.
    fn monotonic(&self) -> Duration;
    /// Waits.
    fn sleep(&self, time: Duration);
}

/// What a sampler measures, for the header of a recording.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SamplerInfo {
    pub game: String,
    pub cores: u32,
}

/// Measures the computer's performance. It keeps what it needs from one
/// sample to the next (usage is a difference between two readings), so the
/// first sample after creating it has no usage figures worth trusting.
pub trait PerformanceSampler {
    /// What is being measured: the game's program and the logical cores.
    fn describe(&self) -> SamplerInfo;
    /// Whether the game is running now (a cheap check, to wait for it).
    fn game_running(&mut self) -> Result<bool, String>;
    /// One measurement. `at_ms` is left at zero: the recorder sets it.
    fn sample(&mut self) -> Result<Sample, String>;
}

/// Where a recording is written, line by line, so that what was measured
/// survives if the program is closed in the middle.
pub trait SessionFile {
    /// Creates the file for a new recording and returns where it is.
    fn create(&mut self, name: &str) -> Result<String, String>;
    fn append(&mut self, line: &str) -> Result<(), String>;
    /// Writes the summary of the recording next to its file (`<name>.summary.json`).
    fn write_summary(&mut self, json: &str) -> Result<(), String>;
}

/// A recording kept on disk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionInfo {
    /// Its name, without extension (`lol-20261118-165329`).
    pub id: String,
    pub bytes: u64,
    /// Whether its summary was saved beside it.
    pub has_summary: bool,
}

/// The recordings made so far.
pub trait SessionArchive {
    /// Every recording, in any order (see `session_id::sort_newest_first`).
    fn list(&self) -> Result<Vec<SessionInfo>, String>;
    /// The text of a recording's CSV. `id` must be a name, never a path.
    fn read_csv(&self, id: &str) -> Result<String, String>;
}

/// How big a piece of text is when drawn: the interface needs it to lay out
/// lines and wrap words, and only the graphics of the system know the fonts.
pub trait TextMetrics {
    /// The size of `text` in one line, in the units of the style.
    fn measure(&self, text: &str, style: &TextStyle) -> Size;
}

/// The settings that fixes change. It reads and writes each one by the id of its
/// fix; everything about *where* it lives (a file, the registry) is its own.
pub trait FixBackend {
    /// The value now. A setting that is not written is `Unset`, not an error.
    fn current(&self, id: &str) -> Result<SettingValue, String>;
    /// What stops the change right now, as things to do (`Cierra el cliente`).
    fn blockers(&self, id: &str) -> Result<Vec<String>, String>;
    /// Writes the value, keeping a copy of what it replaces.
    fn set(&mut self, id: &str, value: &SettingValue) -> Result<(), String>;
}

/// What has been changed, line by line. Lines are only added, never rewritten.
pub trait HistoryStore {
    fn entries(&self) -> Result<Vec<HistoryEntry>, String>;
    fn append(&mut self, entry: &HistoryEntry) -> Result<(), String>;
}

/// Takes a picture of the places where an installer leaves things (the folders where apps
/// live, the keys where they register). It only reads, and is bounded: no contents are read.
pub trait SystemPicture {
    fn take(&self) -> Result<Vec<crate::monitor::Mark>, String>;
    /// Fills in the size of the folders among `marks` (a picture leaves them empty: adding up
    /// every folder would be slow, and only the few the installer made are worth measuring).
    fn measure(&self, _marks: &mut [crate::monitor::Mark]) {}
}

/// Runs an installer the person chose and says when it is over.
pub trait InstallerRunner {
    /// Starts the installer (an `.exe` or `.msi`). It does not wait: installers start other
    /// programs and end, so the person says when it is done.
    fn run(&self, installer: &str) -> Result<(), String>;
}

/// The logs of installations kept on disk.
pub trait InstallLogs {
    fn all(&self) -> Result<Vec<crate::monitor::InstallLog>, String>;
    fn save(&self, log: &crate::monitor::InstallLog) -> Result<(), String>;
    /// Deletes the log with this id.
    fn delete(&self, id: &str) -> Result<(), String>;
    /// Writes the log to a file in the user's Documents; returns where.
    fn export(&self, id: &str) -> Result<String, String>;
    /// Reads a log from a file; it is kept as not made here.
    fn import(&self, file: &str) -> Result<crate::monitor::InstallLog, String>;
}

/// Asks the person for a file with the system's dialog (`None`: they cancelled).
pub trait FilePicker {
    fn pick(&self, title: &str, extensions: &[&str]) -> Result<Option<String>, String>;
}

/// What starts with Windows, and turning it on and off.
pub trait StartupSource {
    /// Everything that starts with Windows or at logon. Reads only.
    fn read(&self) -> Result<Vec<crate::startup::StartupItem>, String>;
    /// The same without what is slow to read (the tasks): for pages that only need to know
    /// whether something starts with Windows.
    fn read_quick(&self) -> Result<Vec<crate::startup::StartupItem>, String> {
        self.read()
    }
    /// Turns an entry on or off the way Windows itself does, without deleting it. `Err` says why
    /// not (an entry of the machine needs an administrator).
    fn set_enabled(&self, item: &crate::startup::StartupItem, on: bool) -> Result<(), String>;
}

/// Finds the junk of Windows: the direct children of the places that hold it. It only reads.
pub trait JunkFinder {
    fn scan(&self) -> Result<Vec<crate::junk::JunkItem>, String>;
}
