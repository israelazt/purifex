//! The arithmetic of a sample: how much of the CPU the cores and each process used
//! between two readings. Pure functions over the system's counters (100 ns ticks), so
//! they are tested without measuring anything.

use std::collections::HashMap;

use purifex_domain::process_group::{group_of, Group};
use purifex_domain::sample::{GameLoad, GroupLoad, ProcLoad};

use crate::nt_layout::{CoreTimes, RawProcess};

/// How many other processes each sample lists.
const TOP_PROCESSES: usize = 5;
/// Below this share of the CPU a process is not worth listing.
const MIN_LISTED: f32 = 0.2;

/// What the cores did between two readings.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct CpuUsage {
    /// Percent of all the cores.
    pub total: f32,
    /// Percent of the busiest core.
    pub max_core: f32,
    /// Percent of all the cores spent in interrupts and deferred calls (drivers).
    pub dpc_interrupt: f32,
}

/// The usage between `before` and `now`. Without a reading to compare to (the first
/// sample, or a different number of cores) there is no difference to measure: zeros.
pub fn cpu_usage(now: &[CoreTimes], before: &[CoreTimes]) -> CpuUsage {
    if now.is_empty() || now.len() != before.len() {
        return CpuUsage::default();
    }
    let (mut busy, mut all, mut dpc) = (0u64, 0u64, 0u64);
    let mut max_core = 0f32;
    for (now, before) in now.iter().zip(before) {
        let total = now.total.saturating_sub(before.total);
        let idle = now.idle.saturating_sub(before.idle);
        let work = total.saturating_sub(idle);
        busy += work;
        all += total;
        dpc += now.dpc_interrupt.saturating_sub(before.dpc_interrupt);
        if total > 0 {
            max_core = max_core.max(work as f32 * 100.0 / total as f32);
        }
    }
    if all == 0 {
        return CpuUsage {
            max_core,
            ..CpuUsage::default()
        };
    }
    CpuUsage {
        total: busy as f32 * 100.0 / all as f32,
        max_core,
        dpc_interrupt: dpc as f32 * 100.0 / all as f32,
    }
}

/// What the processes did between two readings.
#[derive(Debug, Default, PartialEq)]
pub struct ProcessLoads {
    pub game: Option<GameLoad>,
    /// The busiest other processes.
    pub top: Vec<ProcLoad>,
    /// The families of programs (the League client, browsers...), the busiest first.
    pub groups: Vec<GroupLoad>,
    /// The CPU ticks of every process now, to compare with at the next sample.
    pub ticks: HashMap<u64, u64>,
}

/// Which process is which: the game's, this program's own (left out), the rest.
pub struct Identity<'a> {
    pub is_game: &'a dyn Fn(&str) -> bool,
    pub own_pid: u64,
}

/// The load of each process since `before` (their ticks then). `capacity` is the ticks
/// all the cores could have run in the meantime: the 100 % of a share.
pub fn process_loads(
    processes: &[RawProcess],
    before: &HashMap<u64, u64>,
    capacity: f64,
    who: &Identity,
) -> ProcessLoads {
    let share = |delta: u64| {
        if capacity > 0.0 {
            (delta as f64 / capacity * 100.0) as f32
        } else {
            0.0
        }
    };
    let mut game: Option<GameLoad> = None;
    let mut others: HashMap<&str, f32> = HashMap::new();
    let mut families: HashMap<Group, f32> = HashMap::new();
    let mut ticks = HashMap::with_capacity(processes.len());
    for p in processes {
        ticks.insert(p.pid, p.cpu_ticks);
        let Some(previous) = before.get(&p.pid) else {
            continue;
        };
        let cpu = share(p.cpu_ticks.saturating_sub(*previous));
        if (who.is_game)(&p.name) {
            let g = game.get_or_insert_with(GameLoad::default);
            g.cpu += cpu;
            g.memory_mb += (p.working_set / (1024 * 1024)) as u32;
        } else if p.pid != 0 && p.pid != who.own_pid {
            *others.entry(p.name.as_str()).or_default() += cpu;
            if let Some(group) = group_of(&p.name) {
                *families.entry(group).or_default() += cpu;
            }
        }
    }
    // The game may have started between two samples: it is there, with no difference
    // to measure yet.
    if game.is_none() && processes.iter().any(|p| (who.is_game)(&p.name)) {
        game = Some(GameLoad::default());
    }
    ProcessLoads {
        game,
        top: busiest(others),
        groups: busiest_families(families),
        ticks,
    }
}

fn busiest(others: HashMap<&str, f32>) -> Vec<ProcLoad> {
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
    top
}

fn busiest_families(families: HashMap<Group, f32>) -> Vec<GroupLoad> {
    let mut groups: Vec<GroupLoad> = families
        .into_iter()
        .map(|(group, cpu)| GroupLoad { group, cpu })
        .collect();
    groups.sort_by(|a, b| b.cpu.total_cmp(&a.cpu));
    groups
}

#[cfg(test)]
mod tests {
    use super::*;

    fn core(idle: u64, total: u64, dpc: u64) -> CoreTimes {
        CoreTimes {
            idle,
            total,
            dpc_interrupt: dpc,
        }
    }

    fn process(pid: u64, name: &str, ticks: u64) -> RawProcess {
        RawProcess {
            pid,
            name: name.into(),
            cpu_ticks: ticks,
            working_set: 512 * 1024 * 1024,
        }
    }

    #[test]
    fn the_cores_are_measured_between_two_readings() {
        let before = [core(0, 0, 0), core(0, 0, 0)];
        let now = [core(50, 100, 10), core(100, 100, 0)];
        let usage = cpu_usage(&now, &before);
        assert_eq!(
            usage.max_core, 50.0,
            "the first core worked half of the time"
        );
        assert_eq!(usage.total, 25.0, "50 of 200 ticks");
        assert_eq!(usage.dpc_interrupt, 5.0);
    }

    #[test]
    fn without_a_reading_to_compare_there_is_nothing_to_measure() {
        let now = [core(1, 2, 3)];
        assert_eq!(cpu_usage(&now, &[]), CpuUsage::default());
        assert_eq!(
            cpu_usage(&now, &[core(0, 0, 0), core(0, 0, 0)]),
            CpuUsage::default()
        );
        assert_eq!(cpu_usage(&[], &[]), CpuUsage::default());
        // Cores that did not tick: no division by zero.
        assert_eq!(cpu_usage(&now, &now), CpuUsage::default());
    }

    #[test]
    fn the_game_is_told_from_the_rest_and_this_program_is_left_out() {
        let before: HashMap<u64, u64> = [(1, 0), (2, 0), (3, 0), (7, 0)].into();
        let now = [
            process(1, "League of Legends.exe", 500),
            process(2, "chrome.exe", 300),
            process(3, "purifex.exe", 900),
            process(9, "new.exe", 100),
        ];
        let is_game = |n: &str| n.starts_with("League");
        let who = Identity {
            is_game: &is_game,
            own_pid: 3,
        };
        let loads = process_loads(&now, &before, 1000.0, &who);
        let game = loads.game.unwrap();
        assert_eq!((game.cpu, game.memory_mb), (50.0, 512));
        assert_eq!(
            loads.top.len(),
            1,
            "only chrome: purifex is us, new.exe has no before"
        );
        assert_eq!(
            (loads.top[0].name.as_str(), loads.top[0].cpu),
            ("chrome.exe", 30.0)
        );
        assert_eq!(
            loads.ticks.len(),
            4,
            "every process is kept for the next sample"
        );
    }

    #[test]
    fn a_game_that_just_started_is_there_with_nothing_to_measure() {
        let now = [process(5, "League of Legends.exe", 10)];
        let is_game = |n: &str| n.starts_with("League");
        let who = Identity {
            is_game: &is_game,
            own_pid: 0,
        };
        let loads = process_loads(&now, &HashMap::new(), 1000.0, &who);
        assert_eq!(loads.game, Some(GameLoad::default()));
    }

    #[test]
    fn the_list_keeps_the_busiest_few_above_the_minimum() {
        let before: HashMap<u64, u64> = (1..=8).map(|pid| (pid, 0)).collect();
        let now: Vec<RawProcess> = (1..=8)
            .map(|pid| process(pid, &format!("p{pid}.exe"), pid * 100))
            .collect();
        let none = |_: &str| false;
        let who = Identity {
            is_game: &none,
            own_pid: 0,
        };
        let loads = process_loads(&now, &before, 1000.0, &who);
        assert_eq!(loads.top.len(), TOP_PROCESSES);
        assert_eq!(loads.top[0].name, "p8.exe");
        let quiet = [process(1, "idle.exe", 1)];
        let before_quiet: HashMap<u64, u64> = [(1, 0)].into();
        assert!(process_loads(&quiet, &before_quiet, 1_000_000.0, &who)
            .top
            .is_empty());
    }
}
