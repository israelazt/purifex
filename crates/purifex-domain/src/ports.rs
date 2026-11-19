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

/// The apps installed in the computer. Reading it changes nothing.
pub trait AppInventory {
    fn apps(&self) -> Result<Vec<crate::apps::InstalledApp>, String>;
}

/// Looks for places that might hold the traces of an app. It only reads and
/// proposes; `app_traces::judge` decides.
pub trait TraceFinder {
    fn candidates(
        &self,
        app: &crate::apps::InstalledApp,
    ) -> Result<Vec<crate::app_traces::Candidate>, String>;
}

/// Looks at an app that is still installed and says what its traces will be
/// recognised by once it is gone: its programs and the extensions it registered.
pub trait SeedProbe {
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
    /// Puts the removed thing back where it was (it must not exist there now).
    fn restore(&mut self, entry: &crate::cleanup_log::CleanupEntry) -> Result<(), String>;
    fn entries(&self) -> Result<Vec<crate::cleanup_log::CleanupEntry>, String>;
    fn append(&mut self, entry: &crate::cleanup_log::CleanupEntry) -> Result<(), String>;
    /// Keeps what was known of an app when its uninstaller was started (afterwards
    /// the system no longer lists it, and its traces are looked for from this).
    fn remember(&mut self, app: &crate::apps::InstalledApp) -> Result<(), String>;
    fn remembered(&self) -> Result<Vec<crate::apps::InstalledApp>, String>;
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
