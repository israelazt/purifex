//! The sampler of performance for Windows. One sample costs a few system
//! calls (the table of processes, the times of the cores, the clock speeds,
//! the memory and, for NVIDIA cards, NVML): about a millisecond, once a
//! second, and no process stays resident.

mod cpu_clock;
mod nvml;

use std::collections::HashMap;
use std::time::Instant;

use purifex_domain::ports::PerformanceSampler;
use purifex_domain::sample::{effective_mhz, GameLoad, ProcLoad, Sample};
use windows::Wdk::System::SystemInformation::{NtQuerySystemInformation, SYSTEM_INFORMATION_CLASS};
use windows::Win32::System::Power::{
    CallNtPowerInformation, POWER_INFORMATION_LEVEL, PROCESSOR_POWER_INFORMATION,
};
use windows::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};

use crate::nt_layout::{parse_cores, parse_processes, CoreTimes, RawProcess};
use cpu_clock::CpuClock;
use nvml::Nvml;

const SYSTEM_PROCESS_INFORMATION: SYSTEM_INFORMATION_CLASS = SYSTEM_INFORMATION_CLASS(5);
const SYSTEM_PROCESSOR_PERFORMANCE: SYSTEM_INFORMATION_CLASS = SYSTEM_INFORMATION_CLASS(8);
const PROCESSOR_INFORMATION: POWER_INFORMATION_LEVEL = POWER_INFORMATION_LEVEL(11);
const STATUS_INFO_LENGTH_MISMATCH: i32 = 0xC000_0004_u32 as i32;
/// How many other processes each sample lists.
/// The most memory a system table may take.
const MAX_TABLE: usize = 64 * 1024 * 1024;
/// Size of one core's entry in the performance table.
const CORE_ENTRY: usize = 48;
const TOP_PROCESSES: usize = 5;
/// Below this share of the CPU a process is not worth listing.
const MIN_LISTED: f32 = 0.2;

pub struct WindowsSampler {
    /// Names of the game's programs, lowercase.
    game: Vec<String>,
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
        Self {
            game: game_programs.iter().map(|g| g.to_lowercase()).collect(),
            nvml: Nvml::open(),
            clock: CpuClock::open(),
            own_pid: u64::from(std::process::id()),
            buffer: Vec::new(),
            last_at: None,
            last_processes: HashMap::new(),
            last_cores: Vec::new(),
        }
    }

    pub fn has_gpu_readings(&self) -> bool {
        self.nvml.is_some()
    }

    /// Fills `self.buffer` with a system table, growing it as the system asks.
    /// `unit` is the size the table must be a multiple of (its entry size, for
    /// the tables that insist on it).
    fn query(&mut self, class: SYSTEM_INFORMATION_CLASS, unit: usize) -> Result<usize, String> {
        let round = |n: usize| n.div_ceil(unit) * unit;
        if self.buffer.len() < 64 * 1024 {
            self.buffer.resize(round(256 * 1024), 0);
        }
        self.buffer
            .truncate(round(self.buffer.len()).min(self.buffer.len()));
        self.buffer.resize(round(self.buffer.len()), 0);
        for _ in 0..8 {
            let mut needed = 0u32;
            // SAFETY: the buffer is as long as the length passed.
            let status = unsafe {
                NtQuerySystemInformation(
                    class,
                    self.buffer.as_mut_ptr().cast(),
                    self.buffer.len() as u32,
                    &mut needed,
                )
            };
            if status.0 >= 0 {
                return Ok(if needed == 0 {
                    self.buffer.len()
                } else {
                    needed as usize
                });
            }
            if status.0 != STATUS_INFO_LENGTH_MISMATCH {
                return Err(format!(
                    "NtQuerySystemInformation falló: 0x{:08x}",
                    status.0 as u32
                ));
            }
            let size = round((needed as usize + 64 * 1024).max(self.buffer.len() * 2));
            if size > MAX_TABLE {
                return Err("la tabla del sistema es demasiado grande".into());
            }
            self.buffer.resize(size, 0);
        }
        Err("la tabla del sistema no deja de crecer".into())
    }

    fn processes(&mut self) -> Result<Vec<RawProcess>, String> {
        let used = self.query(SYSTEM_PROCESS_INFORMATION, 1)?;
        Ok(parse_processes(&self.buffer[..used.min(self.buffer.len())]))
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
    fn game_running(&mut self) -> Result<bool, String> {
        Ok(self.processes()?.iter().any(|p| self.is_game(&p.name)))
    }

    fn sample(&mut self) -> Result<Sample, String> {
        let started = Instant::now();
        let processes = self.processes()?;
        let used = self.query(SYSTEM_PROCESSOR_PERFORMANCE, CORE_ENTRY)?;
        let cores = parse_cores(&self.buffer[..used.min(self.buffer.len())]);

        // Wall time since the last sample, in 100 ns ticks, for the shares.
        let wall = self
            .last_at
            .map_or(0, |t| started.duration_since(t).as_nanos() / 100) as f64;
        self.last_at = Some(started);
        let capacity = wall * cores.len().max(1) as f64;
        let share = |delta: u64| {
            if capacity > 0.0 {
                (delta as f64 / capacity * 100.0) as f32
            } else {
                0.0
            }
        };

        let (mut cpu_total, mut cpu_max_core, mut dpc_interrupt) = (0f32, 0f32, 0f32);
        if self.last_cores.len() == cores.len() && !cores.is_empty() {
            let (mut busy, mut all, mut dpc) = (0u64, 0u64, 0u64);
            for (now, before) in cores.iter().zip(&self.last_cores) {
                let total = now.total.saturating_sub(before.total);
                let idle = now.idle.saturating_sub(before.idle);
                let work = total.saturating_sub(idle);
                busy += work;
                all += total;
                dpc += now.dpc_interrupt.saturating_sub(before.dpc_interrupt);
                if total > 0 {
                    cpu_max_core = cpu_max_core.max(work as f32 * 100.0 / total as f32);
                }
            }
            if all > 0 {
                cpu_total = busy as f32 * 100.0 / all as f32;
                dpc_interrupt = dpc as f32 * 100.0 / all as f32;
            }
        }
        let core_count = cores.len();
        self.last_cores = cores;

        let mut game: Option<GameLoad> = None;
        let mut others: HashMap<&str, f32> = HashMap::new();
        let mut ticks_now = HashMap::with_capacity(processes.len());
        for p in &processes {
            ticks_now.insert(p.pid, p.cpu_ticks);
            let Some(before) = self.last_processes.get(&p.pid) else {
                continue;
            };
            let cpu = share(p.cpu_ticks.saturating_sub(*before));
            if self.is_game(&p.name) {
                let g = game.get_or_insert(GameLoad {
                    cpu: 0.0,
                    memory_mb: 0,
                });
                g.cpu += cpu;
                g.memory_mb += (p.working_set / (1024 * 1024)) as u32;
            } else if p.pid != 0 && p.pid != self.own_pid {
                *others.entry(p.name.as_str()).or_default() += cpu;
            }
        }
        // The game may have started between two samples: it is there, with no
        // difference to measure yet.
        if game.is_none() && processes.iter().any(|p| self.is_game(&p.name)) {
            game = Some(GameLoad {
                cpu: 0.0,
                memory_mb: 0,
            });
        }
        let mut top: Vec<ProcLoad> = others
            .into_iter()
            .filter(|(_, cpu)| *cpu >= MIN_LISTED)
            .map(|(name, cpu)| ProcLoad {
                name: name.to_string(),
                cpu,
            })
            .collect();
        top.sort_by(|a, b| b.cpu.total_cmp(&a.cpu));
        top.truncate(TOP_PROCESSES);
        self.last_processes = ticks_now;

        let (reported_mhz, cpu_max_mhz) = clock_speeds(core_count);
        // The real clock, when the counter answers; if not, what Windows reports.
        let cpu_mhz = self
            .clock
            .as_mut()
            .and_then(CpuClock::read)
            .map_or(reported_mhz, |percent| effective_mhz(cpu_max_mhz, percent));
        Ok(Sample {
            at_ms: 0,
            cpu_total,
            cpu_max_core,
            dpc_interrupt,
            cpu_mhz,
            cpu_max_mhz,
            ram_used: memory_used_percent(),
            game,
            gpu: self.nvml.as_mut().and_then(Nvml::read),
            top,
            cost_us: started.elapsed().as_micros().min(u128::from(u32::MAX)) as u32,
        })
    }
}
