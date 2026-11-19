//! The sampler of performance for Windows. One sample costs a few system
//! calls (the table of processes, the times of the cores, the clock speeds,
//! the memory and, for NVIDIA cards, NVML): about a millisecond, once a
//! second, and no process stays resident.

mod cpu_clock;
mod load;
mod nvml;

use std::collections::HashMap;
use std::time::Instant;

use purifex_domain::ports::{PerformanceSampler, SamplerInfo};
use purifex_domain::sample::{effective_mhz, Sample};
use windows::Wdk::System::SystemInformation::SYSTEM_INFORMATION_CLASS;
use windows::Win32::System::Power::{
    CallNtPowerInformation, POWER_INFORMATION_LEVEL, PROCESSOR_POWER_INFORMATION,
};
use windows::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};

use crate::nt_layout::{parse_cores, parse_processes, CoreTimes, RawProcess};
use crate::nt_query::{
    self, CORES as SYSTEM_PROCESSOR_PERFORMANCE, CORE_ENTRY,
    PROCESSES as SYSTEM_PROCESS_INFORMATION,
};
use cpu_clock::CpuClock;
use load::{cpu_usage, process_loads, Identity};
use nvml::Nvml;

const PROCESSOR_INFORMATION: POWER_INFORMATION_LEVEL = POWER_INFORMATION_LEVEL(11);
pub struct WindowsSampler {
    /// Names of the game's programs, lowercase.
    game: Vec<String>,
    /// The first program of the game as it was given (for the header of a recording).
    game_name: String,
    /// Logical cores, read once when the sampler is created.
    cores: u32,
    /// The graphics card and the clock counter are opened on the first sample:
    /// `scan` and the window never measure, and NVML alone takes ~35 MB.
    sources_opened: bool,
    nvml: Option<Nvml>,
    clock: Option<CpuClock>,
    own_pid: u64,
    buffer: Vec<u8>,
    last_at: Option<Instant>,
    last_processes: HashMap<u64, u64>,
    last_cores: Vec<CoreTimes>,
}

impl WindowsSampler {
    pub fn new(game_programs: &[&str]) -> Self {
        let mut sampler = Self {
            game: game_programs.iter().map(|g| g.to_lowercase()).collect(),
            game_name: game_programs
                .first()
                .copied()
                .unwrap_or_default()
                .to_string(),
            cores: 1,
            sources_opened: false,
            nvml: None,
            clock: None,
            own_pid: u64::from(std::process::id()),
            buffer: Vec::new(),
            last_at: None,
            last_processes: HashMap::new(),
            last_cores: Vec::new(),
        };
        sampler.cores = sampler
            .query(SYSTEM_PROCESSOR_PERFORMANCE, CORE_ENTRY)
            .map(|used| parse_cores(&sampler.buffer[..used.min(sampler.buffer.len())]).len())
            .map_or(1, |n| n.max(1) as u32);
        sampler
    }

    pub fn has_gpu_readings(&self) -> bool {
        self.nvml.is_some()
    }

    /// Fills `self.buffer` with a system table (see `nt_query`).
    fn query(&mut self, class: SYSTEM_INFORMATION_CLASS, unit: usize) -> Result<usize, String> {
        nt_query::query(&mut self.buffer, class, unit)
    }

    fn processes(&mut self) -> Result<Vec<RawProcess>, String> {
        let used = self.query(SYSTEM_PROCESS_INFORMATION, 1)?;
        Ok(parse_processes(&self.buffer[..used.min(self.buffer.len())]))
    }

    /// The graphics card and the clock counter are opened on the first sample only.
    fn open_sources(&mut self) {
        if self.sources_opened {
            return;
        }
        self.sources_opened = true;
        self.nvml = Nvml::open();
        self.clock = CpuClock::open();
    }

    /// The ticks (100 ns) all the cores could have run since the last sample: the 100 %
    /// of a share. Zero for the first sample.
    fn capacity_since_last(&mut self, now: Instant, cores: usize) -> f64 {
        let wall = self
            .last_at
            .map_or(0, |t| now.duration_since(t).as_nanos() / 100) as f64;
        self.last_at = Some(now);
        wall * cores.max(1) as f64
    }

    fn is_game(&self, name: &str) -> bool {
        let name = name.to_lowercase();
        self.game.contains(&name)
    }
}

/// Clock speeds of the cores: (average now, highest maximum).
fn clock_speeds(cores: usize) -> (u32, u32) {
    let mut info = vec![PROCESSOR_POWER_INFORMATION::default(); cores.max(1)];
    // SAFETY: the output is as long as the length passed.
    let status = unsafe {
        CallNtPowerInformation(
            PROCESSOR_INFORMATION,
            None,
            0,
            Some(info.as_mut_ptr().cast()),
            (info.len() * std::mem::size_of::<PROCESSOR_POWER_INFORMATION>()) as u32,
        )
    };
    if status.0 < 0 {
        return (0, 0);
    }
    let current = info.iter().map(|i| u64::from(i.CurrentMhz)).sum::<u64>() / info.len() as u64;
    let max = info.iter().map(|i| i.MaxMhz).max().unwrap_or(0);
    (current as u32, max)
}

fn memory_used_percent() -> f32 {
    let mut status = MEMORYSTATUSEX {
        dwLength: std::mem::size_of::<MEMORYSTATUSEX>() as u32,
        ..MEMORYSTATUSEX::default()
    };
    // SAFETY: `dwLength` is set, as the function requires.
    match unsafe { GlobalMemoryStatusEx(&mut status) } {
        Ok(()) => status.dwMemoryLoad as f32,
        Err(_) => 0.0,
    }
}

impl PerformanceSampler for WindowsSampler {
    fn describe(&self) -> SamplerInfo {
        SamplerInfo {
            game: self.game_name.clone(),
            cores: self.cores,
        }
    }

    fn game_running(&mut self) -> Result<bool, String> {
        Ok(self.processes()?.iter().any(|p| self.is_game(&p.name)))
    }

    fn sample(&mut self) -> Result<Sample, String> {
        self.open_sources();
        let started = Instant::now();
        let processes = self.processes()?;
        let used = self.query(SYSTEM_PROCESSOR_PERFORMANCE, CORE_ENTRY)?;
        let cores = parse_cores(&self.buffer[..used.min(self.buffer.len())]);
        let capacity = self.capacity_since_last(started, cores.len());
        let usage = cpu_usage(&cores, &self.last_cores);
        let core_count = cores.len();
        self.last_cores = cores;

        let is_game = |name: &str| self.is_game(name);
        let who = Identity {
            is_game: &is_game,
            own_pid: self.own_pid,
        };
        let loads = process_loads(&processes, &self.last_processes, capacity, &who);
        self.last_processes = loads.ticks;

        let (reported_mhz, cpu_max_mhz) = clock_speeds(core_count);
        // The real clock, when the counter answers; if not, what Windows reports.
        let cpu_mhz = self
            .clock
            .as_mut()
            .and_then(CpuClock::read)
            .map_or(reported_mhz, |percent| effective_mhz(cpu_max_mhz, percent));
        Ok(Sample {
            at_ms: 0,
            cpu_total: usage.total,
            cpu_max_core: usage.max_core,
            dpc_interrupt: usage.dpc_interrupt,
            cpu_mhz,
            cpu_max_mhz,
            ram_used: memory_used_percent(),
            game: loads.game,
            gpu: self.nvml.as_mut().and_then(Nvml::read),
            groups: loads.groups,
            top: loads.top,
            cost_us: started.elapsed().as_micros().min(u128::from(u32::MAX)) as u32,
        })
    }
}
