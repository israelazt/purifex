//! Fakes of the ports, for the tests of this crate.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

use purifex_domain::date::Date;
use purifex_domain::device::Device;
use purifex_domain::fix::{HistoryEntry, SettingValue};
use purifex_domain::ports::{
    Clock, PerformanceSampler, SamplerInfo, SessionArchive, SessionFile, SessionInfo,
    SystemInventory,
};
use purifex_domain::sample::{GameLoad, Sample};

/// Offers that arrive after a while (or an error), as Windows Update would give them.
pub struct FakeOffers(
    pub u64,
    pub Result<Vec<purifex_domain::driver_offers::Offer>, String>,
);

impl purifex_domain::ports::DriverOffers for FakeOffers {
    fn search(&self) -> Result<Vec<purifex_domain::driver_offers::Offer>, String> {
        std::thread::sleep(Duration::from_millis(self.0));
        self.1.clone()
    }
}

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
    fn describe(&self) -> SamplerInfo {
        SamplerInfo {
            game: "League of Legends.exe".into(),
            cores: 12,
        }
    }

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
            groups: vec![],
            top: vec![],
            cost_us: 400,
        })
    }
}

#[derive(Default)]
pub struct FakeFile {
    pub lines: RefCell<Vec<String>>,
    pub summary: RefCell<Option<String>>,
    pub fail_appends: bool,
    pub fail_summary: bool,
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

    fn write_summary(&mut self, json: &str) -> Result<(), String> {
        if self.fail_summary {
            return Err("read-only disk".into());
        }
        *self.summary.borrow_mut() = Some(json.to_string());
        Ok(())
    }
}

/// An archive of recordings held in memory: `(id, csv)`.
#[derive(Default)]
pub struct FakeArchive {
    pub sessions: Vec<(String, String)>,
    /// Make listing fail with this reason.
    pub fails: Option<String>,
}

impl FakeArchive {
    pub fn with(sessions: &[(&str, &str)]) -> Self {
        Self {
            sessions: sessions
                .iter()
                .map(|(i, c)| (i.to_string(), c.to_string()))
                .collect(),
            fails: None,
        }
    }
}

impl SessionArchive for FakeArchive {
    fn list(&self) -> Result<Vec<SessionInfo>, String> {
        if let Some(reason) = &self.fails {
            return Err(reason.clone());
        }
        Ok(self
            .sessions
            .iter()
            .map(|(id, csv)| SessionInfo {
                id: id.clone(),
                bytes: csv.len() as u64,
                has_summary: false,
            })
            .collect())
    }

    fn read_csv(&self, id: &str) -> Result<String, String> {
        self.sessions
            .iter()
            .find(|(i, _)| i == id)
            .map(|(_, csv)| csv.clone())
            .ok_or_else(|| format!("no existe la grabación «{id}»"))
    }
}

/// What the fake setting holds and what it was asked, kept where a test can see it.
#[derive(Default)]
pub struct FixState {
    pub value: Option<SettingValue>,
    pub blockers: Vec<String>,
    pub writes: Vec<SettingValue>,
    /// The write is accepted but the value does not change.
    pub does_not_stick: bool,
    pub history: Vec<HistoryEntry>,
    pub history_full: bool,
}

/// A setting and a history in memory: the test keeps a second handle to see them.
#[derive(Clone)]
pub struct FakeFixes(pub Rc<RefCell<FixState>>);

impl FakeFixes {
    pub fn with(value: Option<SettingValue>) -> (Self, Rc<RefCell<FixState>>) {
        let state = Rc::new(RefCell::new(FixState {
            value,
            ..FixState::default()
        }));
        (Self(state.clone()), state)
    }

    /// A `Fixer` over this fake.
    pub fn fixer(&self) -> crate::fixer::Fixer {
        crate::fixer::Fixer::new(Box::new(self.clone()), Box::new(self.clone()))
    }
}

impl purifex_domain::ports::FixBackend for FakeFixes {
    fn current(&self, _: &str) -> Result<SettingValue, String> {
        self.0
            .borrow()
            .value
            .clone()
            .ok_or_else(|| "no está instalado".to_string())
    }
    fn blockers(&self, _: &str) -> Result<Vec<String>, String> {
        Ok(self.0.borrow().blockers.clone())
    }
    fn set(&mut self, _: &str, value: &SettingValue) -> Result<(), String> {
        let mut state = self.0.borrow_mut();
        state.writes.push(value.clone());
        if !state.does_not_stick {
            state.value = Some(value.clone());
        }
        Ok(())
    }
}

impl purifex_domain::ports::HistoryStore for FakeFixes {
    fn entries(&self) -> Result<Vec<HistoryEntry>, String> {
        Ok(self.0.borrow().history.clone())
    }
    fn append(&mut self, entry: &HistoryEntry) -> Result<(), String> {
        if self.0.borrow().history_full {
            return Err("disco lleno".into());
        }
        self.0.borrow_mut().history.push(entry.clone());
        Ok(())
    }
}
