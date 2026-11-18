//! One measurement of the computer's performance, taken once a second while
//! a game runs, and its line in the CSV file of the session.

use serde::Serialize;

/// What one process is using, as a share of the whole CPU.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ProcLoad {
    pub name: String,
    pub cpu: f32,
}

/// The game's own use of the computer.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct GameLoad {
    /// Percent of the whole CPU.
    pub cpu: f32,
    pub memory_mb: u32,
}

/// The graphics card, as its driver reports it.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct GpuSample {
    pub util: f32,
    pub memory_util: f32,
    pub temp_c: u32,
    pub clock_mhz: u32,
    pub max_clock_mhz: u32,
    pub power_w: f32,
    /// Why the clock is held back (NVML's bits; see [`throttle`]).
    pub throttle: u64,
}

/// The bits of `GpuSample::throttle` that matter here (NVML's numbering).
pub mod throttle {
    pub const POWER_CAP: u64 = 0x4;
    pub const HW_SLOWDOWN: u64 = 0x8;
    pub const SW_THERMAL: u64 = 0x20;
    pub const HW_THERMAL: u64 = 0x40;
    pub const HW_POWER_BRAKE: u64 = 0x80;
    /// Any of the heat or power-brake reasons: the card is slowing itself down.
    pub const HEAT: u64 = HW_SLOWDOWN | SW_THERMAL | HW_THERMAL | HW_POWER_BRAKE;
}

/// A second of the session.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Sample {
    /// Milliseconds since the recording began.
    pub at_ms: u64,
    pub cpu_total: f32,
    /// The busiest logical core: a game's main thread is limited by this one,
    /// not by the total.
    pub cpu_max_core: f32,
    /// Time in interrupts and deferred calls, percent of the CPU (drivers).
    pub dpc_interrupt: f32,
    pub cpu_mhz: u32,
    pub cpu_max_mhz: u32,
    pub ram_used: f32,
    pub game: Option<GameLoad>,
    pub gpu: Option<GpuSample>,
    /// The busiest processes besides the game, most first.
    pub top: Vec<ProcLoad>,
    /// What taking this sample cost, in microseconds.
    pub cost_us: u32,
}

pub const CSV_HEADER: &str = "t_s,cpu_total,cpu_max_core,dpc_interrupt,cpu_mhz,cpu_max_mhz,ram_used,\
game_cpu,game_mb,gpu_util,gpu_mem_util,gpu_temp_c,gpu_clock_mhz,gpu_max_clock_mhz,gpu_power_w,gpu_throttle,top,cost_us";

impl Sample {
    /// The line of this sample in the CSV (no trailing newline).
    pub fn csv_row(&self) -> String {
        let game = self
            .game
            .map_or(",".to_string(), |g| format!("{:.1},{}", g.cpu, g.memory_mb));
        let gpu = self.gpu.map_or(",,,,,,".to_string(), |g| {
            format!(
                "{:.0},{:.0},{},{},{},{:.1},0x{:x}",
                g.util,
                g.memory_util,
                g.temp_c,
                g.clock_mhz,
                g.max_clock_mhz,
                g.power_w,
                g.throttle
            )
        });
        let top: Vec<String> = self
            .top
            .iter()
            .map(|p| format!("{}={:.1}", p.name.replace([',', ';', '='], " "), p.cpu))
            .collect();
        format!(
            "{:.1},{:.1},{:.1},{:.1},{},{},{:.1},{},{},{},{}",
            self.at_ms as f64 / 1000.0,
            self.cpu_total,
            self.cpu_max_core,
            self.dpc_interrupt,
            self.cpu_mhz,
            self.cpu_max_mhz,
            self.ram_used,
            game,
            gpu,
            top.join(";"),
            self.cost_us
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    pub fn sample() -> Sample {
        Sample {
            at_ms: 2500,
            cpu_total: 41.26,
            cpu_max_core: 97.0,
            dpc_interrupt: 1.5,
            cpu_mhz: 3600,
            cpu_max_mhz: 4200,
            ram_used: 63.0,
            game: Some(GameLoad {
                cpu: 12.5,
                memory_mb: 1800,
            }),
            gpu: Some(GpuSample {
                util: 88.0,
                memory_util: 40.0,
                temp_c: 71,
                clock_mhz: 1800,
                max_clock_mhz: 2100,
                power_w: 95.25,
                throttle: throttle::POWER_CAP,
            }),
            top: vec![ProcLoad {
                name: "chrome.exe".into(),
                cpu: 3.0,
            }],
            cost_us: 900,
        }
    }

    #[test]
    fn the_row_has_as_many_fields_as_the_header() {
        let header = CSV_HEADER.split(',').count();
        assert_eq!(sample().csv_row().split(',').count(), header);
        let mut bare = sample();
        (bare.game, bare.gpu, bare.top) = (None, None, vec![]);
        assert_eq!(bare.csv_row().split(',').count(), header);
    }

    #[test]
    fn the_row_prints_the_values() {
        let row = sample().csv_row();
        assert!(row.starts_with(
            "2.5,41.3,97.0,1.5,3600,4200,63.0,12.5,1800,88,40,71,1800,2100,95.2,0x4,"
        ));
        assert!(row.ends_with("chrome.exe=3.0,900"));
    }

    #[test]
    fn heat_reasons_are_told_from_the_power_cap() {
        assert_eq!(throttle::POWER_CAP & throttle::HEAT, 0);
        assert_ne!(throttle::SW_THERMAL & throttle::HEAT, 0);
    }
}
