//! What the application can do, as one trait: the command line (and the MCP
//! server) ask for things through it, without knowing which ports are behind.
//! [`App`] is the real implementation, built from the services of this crate.

use purifex_domain::ports::{Clock, PerformanceSampler, SessionFile, SystemInventory};
use purifex_domain::report::{ScanReport, Scope};

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
}

/// The services of the application over real (or fake) ports.
pub struct App<I, C, S, C2, F> {
    scanner: Scanner<I, C>,
    recorder: Recorder<S, C2, F>,
}

impl<I, C, S, C2, F> App<I, C, S, C2, F> {
    pub fn new(scanner: Scanner<I, C>, recorder: Recorder<S, C2, F>) -> Self {
        Self { scanner, recorder }
    }
}

impl<I, C, S, C2, F> Services for App<I, C, S, C2, F>
where
    I: SystemInventory,
    C: Clock,
    S: PerformanceSampler,
    C2: Clock,
    F: SessionFile,
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
}
