//! Fakes of the ports, for the tests of this crate.

use std::cell::{Cell, RefCell};
use std::time::Duration;

use purifex_domain::date::Date;
use purifex_domain::device::Device;
use purifex_domain::ports::{Clock, PerformanceSampler, SessionFile, SystemInventory};
use purifex_domain::sample::{GameLoad, Sample};

pub struct FakeInventory(Result<Vec<Device>, String>);

impl FakeInventory {
    pub fn with(devices: Vec<Device>) -> Self {
        Self(Ok(devices))
    }

    pub fn failing(reason: &str) -> Self {
        Self(Err(reason.into()))
    }
}

impl SystemInventory for FakeInventory {
    fn devices(&self) -> Result<Vec<Device>, String> {
        self.0.clone()
    }
}

/// A clock that moves `step_ms` every time the monotonic time is read.
pub struct FakeClock {
    step_ms: u64,
    now_ms: Cell<u64>,
}

impl FakeClock {
    pub const DAY: Date = Date {
        year: 2026,
        month: 11,
        day: 18,
    };

    pub fn ticking(step_ms: u64) -> Self {
        Self {
            step_ms,
            now_ms: Cell::new(0),
        }
    }
}

impl Clock for FakeClock {
    fn today(&self) -> Date {
        Self::DAY
    }

    fn monotonic(&self) -> Duration {
        let now = self.now_ms.get();
        self.now_ms.set(now + self.step_ms);
        Duration::from_millis(now)
    }

    fn sleep(&self, time: Duration) {
        self.now_ms.set(self.now_ms.get() + time.as_millis() as u64);
    }
}

/// A sampler that says the game is missing for `absent_for` checks, then gives
/// `with_game` seconds with the game, then ones without it.
pub struct FakeSampler {
    pub waits: u32,
    absent_for: u32,
    with_game: u32,
    given: u32,
    keep_game: bool,
    pub fail_after: Option<u32>,
}

impl FakeSampler {
    pub fn new(absent_for: u32, with_game: u32, stops_with_game: bool) -> Self {
        Self {
            waits: 0,
            absent_for,
            with_game,
            given: 0,
            keep_game: !stops_with_game,
            fail_after: None,
        }
    }
}

impl PerformanceSampler for FakeSampler {
    fn game_running(&mut self) -> Result<bool, String> {
        self.waits += 1;
        Ok(self.waits > self.absent_for)
    }

    fn sample(&mut self) -> Result<Sample, String> {
        if self.fail_after.is_some_and(|n| self.given >= n) {
            return Err("sensor lost".into());
        }
        self.given += 1;
        // The first call is the baseline that the recorder discards.
        let in_game = self.keep_game || self.given <= self.with_game + 1;
        Ok(Sample {
            at_ms: 0,
            cpu_total: 30.0,
            cpu_max_core: 60.0,
            dpc_interrupt: 1.0,
            cpu_mhz: 3000,
            cpu_max_mhz: 4000,
            ram_used: 50.0,
            game: in_game.then_some(GameLoad {
                cpu: 10.0,
                memory_mb: 1500,
            }),
            gpu: None,
            top: vec![],
            cost_us: 400,
        })
    }
}

#[derive(Default)]
pub struct FakeFile {
    pub lines: RefCell<Vec<String>>,
    pub fail_appends: bool,
}

impl SessionFile for FakeFile {
    fn create(&mut self, name: &str) -> Result<String, String> {
        Ok(format!("fake/{name}.csv"))
    }

    fn append(&mut self, line: &str) -> Result<(), String> {
        if self.fail_appends {
            return Err("disk full".into());
        }
        self.lines.borrow_mut().push(line.to_string());
        Ok(())
    }
}
