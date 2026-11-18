//! What the rest of the program needs from the system, said by what it needs
//! and not by how Windows (or another system) gives it. The real adapters live
//! in `purifex-platform`; the tests use fakes.

use std::time::Duration;

use crate::date::Date;
use crate::device::Device;
use crate::sample::Sample;

/// The devices present in the computer, with the driver each one runs.
pub trait SystemInventory {
    /// Every present device. Reading it changes nothing. A device whose data
    /// cannot be read is still listed, with what could be read.
    fn devices(&self) -> Result<Vec<Device>, String>;
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
