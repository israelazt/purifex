//! The summary of a recording: for each measure, its average, its 95th
//! percentile and its maximum. Only the percentiles tell the truth about a
//! game: an average of 40 % hides a core pinned at 100 % for minutes.

use std::collections::BTreeMap;

use serde::Serialize;

use crate::process_group::{Group, ALL_GROUPS};
use crate::sample::{throttle, Sample};

/// Average, 95th percentile and maximum of a measure.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize)]
pub struct Stat {
    pub avg: f32,
    pub p95: f32,
    pub max: f32,
}

impl Stat {
    pub fn of(mut values: Vec<f32>) -> Self {
        if values.is_empty() {
            return Self::default();
        }
        values.sort_by(|a, b| a.total_cmp(b));
        let rank = ((values.len() as f32 * 0.95).ceil() as usize).clamp(1, values.len());
        Self {
            avg: values.iter().sum::<f32>() / values.len() as f32,
            p95: values[rank - 1],
            max: values[values.len() - 1],
        }
    }
}

/// What the graphics card did over the session.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct GpuSummary {
    pub util: Stat,
    pub temp_c: Stat,
    pub power_w: Stat,
    /// Clock as a share of its maximum.
    pub clock_pct: Stat,
    /// Share of the seconds with a heat or power-brake reason (0–100).
    pub heat_throttled_pct: f32,
    /// Share of the seconds held at the power limit (0–100).
    pub power_capped_pct: f32,
}

/// What a family of programs used over the session, per second.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct GroupSummary {
    pub group: Group,
    /// Percent of the whole CPU; a second without the family counts as zero.
    pub cpu: Stat,
}

/// A process that used CPU besides the game.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Background {
    pub name: String,
    /// Average percent of the whole CPU over the whole session.
    pub avg_cpu: f32,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct SessionSummary {
    pub seconds: u64,
    pub samples: usize,
    pub cpu_total: Stat,
    pub cpu_max_core: Stat,
    /// Share of the seconds with some core at 95 % or more (0–100).
    pub core_pinned_pct: f32,
    pub dpc_interrupt: Stat,
    /// Average clock as a share of the maximum.
    pub cpu_clock_pct: Stat,
    pub ram_used: Stat,
    pub game_cpu: Stat,
    pub game_memory_mb: Stat,
    pub gpu: Option<GpuSummary>,
    /// The families of programs that used CPU, the busiest first.
    pub groups: Vec<GroupSummary>,
    /// Busiest other processes, most first.
    pub background: Vec<Background>,
    /// What one sample cost, in microseconds.
    pub sample_cost_us: Stat,
}

const PINNED: f32 = 95.0;
/// How many background processes the summary keeps.
const BACKGROUND_KEPT: usize = 5;

fn share(count: usize, of: usize) -> f32 {
    if of == 0 {
        0.0
    } else {
        count as f32 * 100.0 / of as f32
    }
}

pub fn summarize(samples: &[Sample]) -> SessionSummary {
    let stat =
        |f: &dyn Fn(&Sample) -> Option<f32>| Stat::of(samples.iter().filter_map(f).collect());
    let gpus: Vec<_> = samples.iter().filter_map(|s| s.gpu).collect();
    let gpu = (!gpus.is_empty()).then(|| GpuSummary {
        util: Stat::of(gpus.iter().map(|g| g.util).collect()),
        temp_c: Stat::of(gpus.iter().map(|g| g.temp_c as f32).collect()),
        power_w: Stat::of(gpus.iter().map(|g| g.power_w).collect()),
        clock_pct: Stat::of(
            gpus.iter()
                .filter(|g| g.max_clock_mhz > 0)
                .map(|g| g.clock_mhz as f32 * 100.0 / g.max_clock_mhz as f32)
                .collect(),
        ),
        heat_throttled_pct: share(
            gpus.iter()
                .filter(|g| g.throttle & throttle::HEAT != 0)
                .count(),
            gpus.len(),
        ),
        power_capped_pct: share(
            gpus.iter()
                .filter(|g| g.throttle & throttle::POWER_CAP != 0)
                .count(),
            gpus.len(),
        ),
    });

    // A process missing from a second's list counts as zero in that second.
    let mut load: BTreeMap<&str, f32> = BTreeMap::new();
    for process in samples.iter().flat_map(|s| &s.top) {
        *load.entry(process.name.as_str()).or_default() += process.cpu;
    }
    let mut background: Vec<Background> = load
        .into_iter()
        .map(|(name, sum)| Background {
            name: name.to_string(),
            avg_cpu: sum / samples.len().max(1) as f32,
        })
        .collect();
    background.sort_by(|a, b| b.avg_cpu.total_cmp(&a.avg_cpu));
    background.truncate(BACKGROUND_KEPT);

    let mut groups: Vec<GroupSummary> = ALL_GROUPS
        .into_iter()
        .filter(|g| {
            samples
                .iter()
                .any(|s| s.groups.iter().any(|l| l.group == *g))
        })
        .map(|group| GroupSummary {
            group,
            cpu: Stat::of(
                samples
                    .iter()
                    .map(|s| {
                        s.groups
                            .iter()
                            .filter(|l| l.group == group)
                            .map(|l| l.cpu)
                            .sum()
                    })
                    .collect(),
            ),
        })
        .collect();
    groups.sort_by(|a, b| b.cpu.avg.total_cmp(&a.cpu.avg));

    SessionSummary {
        seconds: samples.last().map_or(0, |s| s.at_ms / 1000),
        samples: samples.len(),
        cpu_total: stat(&|s| Some(s.cpu_total)),
        cpu_max_core: stat(&|s| Some(s.cpu_max_core)),
        core_pinned_pct: share(
            samples.iter().filter(|s| s.cpu_max_core >= PINNED).count(),
            samples.len(),
        ),
        dpc_interrupt: stat(&|s| Some(s.dpc_interrupt)),
        cpu_clock_pct: stat(&|s| {
            (s.cpu_max_mhz > 0).then(|| s.cpu_mhz as f32 * 100.0 / s.cpu_max_mhz as f32)
        }),
        ram_used: stat(&|s| Some(s.ram_used)),
        game_cpu: stat(&|s| s.game.map(|g| g.cpu)),
        game_memory_mb: stat(&|s| s.game.map(|g| g.memory_mb as f32)),
        gpu,
        groups,
        background,
        sample_cost_us: stat(&|s| Some(s.cost_us as f32)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sample::{GpuSample, ProcLoad};

    pub fn at(ms: u64, core: f32) -> Sample {
        Sample {
            at_ms: ms,
            cpu_total: 30.0,
            cpu_max_core: core,
            dpc_interrupt: 1.0,
            cpu_mhz: 3000,
            cpu_max_mhz: 4000,
            ram_used: 60.0,
            game: None,
            gpu: None,
            groups: vec![],
            top: vec![],
            cost_us: 500,
        }
    }

    #[test]
    fn the_percentile_shows_what_the_average_hides() {
        let mut values = vec![10.0; 90];
        values.extend(vec![100.0; 10]);
        let stat = Stat::of(values);
        assert_eq!(stat.avg, 19.0);
        assert_eq!(stat.p95, 100.0);
        assert_eq!(stat.max, 100.0);
        assert_eq!(Stat::of(vec![]), Stat::default());
        assert_eq!(Stat::of(vec![7.0]).p95, 7.0);
    }

    #[test]
    fn it_counts_the_seconds_with_a_pinned_core() {
        let samples = [
            at(1000, 99.0),
            at(2000, 50.0),
            at(3000, 95.0),
            at(4000, 10.0),
        ];
        let summary = summarize(&samples);
        assert_eq!(summary.core_pinned_pct, 50.0);
        assert_eq!(summary.seconds, 4);
        assert_eq!(summary.cpu_clock_pct.avg, 75.0);
        assert_eq!(summary.gpu, None);
    }

    #[test]
    fn the_gpu_is_summarized_with_its_throttling() {
        let gpu = |throttle| {
            let mut s = at(1000, 50.0);
            s.gpu = Some(GpuSample {
                util: 90.0,
                memory_util: 30.0,
                temp_c: 80,
                clock_mhz: 1500,
                max_clock_mhz: 2000,
                power_w: 100.0,
                throttle,
            });
            s
        };
        let samples = [
            gpu(0),
            gpu(throttle::SW_THERMAL),
            gpu(throttle::POWER_CAP),
            gpu(0),
        ];
        let g = summarize(&samples).gpu.unwrap();
        assert_eq!(g.heat_throttled_pct, 25.0);
        assert_eq!(g.power_capped_pct, 25.0);
        assert_eq!(g.clock_pct.avg, 75.0);
    }

    #[test]
    fn a_family_is_summed_per_second_and_averaged_over_the_whole_session() {
        use crate::sample::GroupLoad;
        let with = |ms, cpu| {
            let mut s = at(ms, 10.0);
            s.groups = vec![GroupLoad {
                group: Group::LeagueClient,
                cpu,
            }];
            s
        };
        let samples = [
            with(1000, 16.0),
            with(2000, 12.0),
            at(3000, 10.0),
            at(4000, 10.0),
        ];
        let groups = summarize(&samples).groups;
        assert_eq!(groups.len(), 1, "only the families that were seen");
        assert_eq!(groups[0].group, Group::LeagueClient);
        assert_eq!(groups[0].cpu.avg, 7.0);
        assert_eq!(groups[0].cpu.max, 16.0);
    }

    #[test]
    fn background_load_is_averaged_over_the_whole_session() {
        let with = |cpu| {
            let mut s = at(1000, 10.0);
            s.top = vec![ProcLoad {
                name: "chrome.exe".into(),
                cpu,
            }];
            s
        };
        // Present in two of four seconds, 8 % each: 4 % on average.
        let samples = [with(8.0), with(8.0), at(3000, 10.0), at(4000, 10.0)];
        let background = summarize(&samples).background;
        assert_eq!(
            background,
            [Background {
                name: "chrome.exe".into(),
                avg_cpu: 4.0
            }]
        );
    }
}
