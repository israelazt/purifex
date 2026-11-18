//! The general state of the computer as the home page shows it: four readings,
//! what limits the games, and the lines of the event console. Every figure comes
//! from a scan or from a recording; with no recording there are no figures about
//! games, and the page says so.

use serde::Serialize;

use crate::analysis::Analysis;
use crate::date::Date;
use crate::device::Health;
use crate::device_category::{category_of, Category};
use crate::diagnosis::Finding;
use crate::process_group::Group;
use crate::report::{Entry, ScanReport};
use crate::session_id::{stamp_of, SessionStamp};
use crate::version::DriverVersion;

/// What the scan found, in four numbers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DeviceKpi {
    pub hardware: usize,
    pub healthy: usize,
    pub missing_driver: usize,
    pub faults: usize,
    pub disabled: usize,
}

/// The graphics card and its driver.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GpuKpi {
    pub name: String,
    pub provider: Option<String>,
    pub version: Option<DriverVersion>,
    pub date: Option<Date>,
    pub age_days: Option<i64>,
}

/// The last recording, in the readings that matter.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SessionKpi {
    pub id: String,
    pub started: Option<SessionStamp>,
    pub seconds: u64,
    /// Time in interrupts and deferred calls, percent of the CPU (p95).
    pub dpc_p95: f32,
    pub core_p95: f32,
    pub core_pinned_pct: f32,
    pub gpu_util_avg: Option<f32>,
    pub gpu_temp_max: Option<f32>,
    pub game_cpu_avg: f32,
    pub league_client_avg: Option<f32>,
    pub findings: Vec<Finding>,
}

/// What limits the games, in a title and a line.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Limiter {
    pub title: String,
    pub detail: String,
}

/// A line of the event console.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ConsoleLine {
    /// `SCAN`, `DRIVERS`, `PARTIDA`, `PISTA`.
    pub tag: String,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Overview {
    pub scanned_on: Date,
    pub scan_ms: u64,
    pub devices: DeviceKpi,
    pub gpu: Option<GpuKpi>,
    pub session: Option<SessionKpi>,
    pub limiter: Limiter,
    pub console: Vec<ConsoleLine>,
}

impl Overview {
    pub fn build(scan: &ScanReport, last: Option<&Analysis>) -> Self {
        let devices = device_kpi(scan);
        let session = last.map(session_kpi);
        let limiter = limiter_of(session.as_ref());
        let console = console_of(scan, &devices, session.as_ref());
        Self {
            scanned_on: scan.scanned_on,
            scan_ms: scan.elapsed_ms,
            gpu: gpu_kpi(scan),
            devices,
            session,
            limiter,
            console,
        }
    }
}

fn device_kpi(scan: &ScanReport) -> DeviceKpi {
    let s = &scan.summary;
    DeviceKpi {
        hardware: s.physical,
        healthy: s
            .physical
            .saturating_sub(s.missing_driver + s.faults + s.disabled),
        missing_driver: s.missing_driver,
        faults: s.faults,
        disabled: s.disabled,
    }
}

/// The first graphics device that runs a driver from its maker (not the basic
/// one of Windows), or else the first one.
fn gpu_kpi(scan: &ScanReport) -> Option<GpuKpi> {
    let gpus: Vec<&Entry> = scan
        .devices
        .iter()
        .filter(|e| category_of(&e.device.class) == Category::Gpu && e.health != Health::Disabled)
        .collect();
    let chosen = gpus.iter().find(|e| !e.inbox_driver).or(gpus.first())?;
    let driver = chosen.device.driver.as_ref();
    Some(GpuKpi {
        name: chosen.device.name.clone(),
        provider: driver.and_then(|d| d.provider.clone()),
        version: driver.and_then(|d| d.version),
        date: driver.and_then(|d| d.date),
        age_days: chosen.driver_age_days,
    })
}

fn session_kpi(analysis: &Analysis) -> SessionKpi {
    let s = &analysis.summary;
    SessionKpi {
        id: analysis.id.clone(),
        started: stamp_of(&analysis.id),
        seconds: s.seconds,
        dpc_p95: s.dpc_interrupt.p95,
        core_p95: s.cpu_max_core.p95,
        core_pinned_pct: s.core_pinned_pct,
        gpu_util_avg: s.gpu.as_ref().map(|g| g.util.avg),
        gpu_temp_max: s.gpu.as_ref().map(|g| g.temp_c.max),
        game_cpu_avg: s.game_cpu.avg,
        league_client_avg: s
            .groups
            .iter()
            .find(|g| g.group == Group::LeagueClient)
            .map(|g| g.cpu.avg),
        findings: analysis.findings.clone(),
    }
}

fn limiter_of(session: Option<&SessionKpi>) -> Limiter {
    match session {
        None => Limiter {
            title: "Sin datos de partida".into(),
            detail: "Graba una partida con `purifex watch` para saber qué la limita.".into(),
        },
        Some(s) => match s.findings.first() {
            Some(finding) => Limiter {
                title: finding.title.clone(),
                detail: finding.evidence.clone(),
            },
            None => Limiter {
                title: "Nada destacable".into(),
                detail: "Ni el CPU, ni la GPU, ni los drivers, ni el calor limitaron esa partida."
                    .into(),
            },
        },
    }
}

fn console_of(
    scan: &ScanReport,
    devices: &DeviceKpi,
    session: Option<&SessionKpi>,
) -> Vec<ConsoleLine> {
    let line = |tag: &str, text: String| ConsoleLine {
        tag: tag.into(),
        text,
    };
    let mut lines = vec![
        line(
            "SCAN",
            format!(
                "{} dispositivos leídos en {} ms; {} son hardware.",
                scan.summary.scanned, scan.elapsed_ms, devices.hardware
            ),
        ),
        line(
            "DRIVERS",
            format!(
                "{} sin problemas · {} sin driver · {} con fallo · {} desactivados.",
                devices.healthy, devices.missing_driver, devices.faults, devices.disabled
            ),
        ),
    ];
    match session {
        None => lines.push(line("PARTIDA", "Todavía no hay grabaciones.".into())),
        Some(s) => {
            lines.push(line(
                "PARTIDA",
                format!(
                    "Última grabación {} de {} s: DPC p95 {:.1} %, núcleo más cargado p95 {:.0} %.",
                    s.id, s.seconds, s.dpc_p95, s.core_p95
                ),
            ));
            for finding in &s.findings {
                lines.push(line(
                    "PISTA",
                    format!("{}: {}", finding.title, finding.evidence),
                ));
            }
        }
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::device::{Device, InstalledDriver};
    use crate::report::Scope;
    use crate::session_csv::parse;

    fn device(id: &str, class: &str, name: &str, provider: &str, problem: Option<u32>) -> Device {
        Device {
            instance_id: id.into(),
            class: class.into(),
            name: name.into(),
            problem,
            driver: Some(InstalledDriver {
                provider: Some(provider.into()),
                version: DriverVersion::parse("32.0.16.2124"),
                date: Date::new(2026, 11, 7),
                inf: None,
            }),
            ..Device::default()
        }
    }

    fn scan() -> ScanReport {
        ScanReport::build(
            vec![
                device(
                    r"PCI\A",
                    "Display",
                    "Microsoft Basic Display",
                    "Microsoft",
                    None,
                ),
                device(
                    r"PCI\B",
                    "Display",
                    "NVIDIA GeForce GTX 1660 Ti",
                    "NVIDIA",
                    None,
                ),
                device(r"USB\C", "Net", "Dongle", "X", Some(28)),
                device(r"PCI\D", "Audio", "Sonido", "Y", Some(43)),
            ],
            Scope::Hardware,
            Date::new(2026, 11, 18).unwrap(),
            200,
        )
    }

    const CSV: &str = "t_s,cpu_total,cpu_max_core,dpc_interrupt,groups\n1.0,40.0,99.0,2.0,league_client=16.0\n2.0,40.0,98.0,2.4,league_client=17.0\n";

    fn analysis() -> Analysis {
        Analysis::of("lol-20261118-165329", parse(CSV).unwrap()).unwrap()
    }

    #[test]
    fn the_devices_are_counted_by_how_they_are_doing() {
        let o = Overview::build(&scan(), None);
        assert_eq!(
            o.devices,
            DeviceKpi {
                hardware: 4,
                healthy: 2,
                missing_driver: 1,
                faults: 1,
                disabled: 0
            }
        );
    }

    #[test]
    fn the_gpu_is_the_one_with_its_makers_driver_not_the_basic_one() {
        let gpu = Overview::build(&scan(), None).gpu.unwrap();
        assert_eq!(gpu.name, "NVIDIA GeForce GTX 1660 Ti");
        assert_eq!(gpu.age_days, Some(11));
        assert_eq!(gpu.provider.as_deref(), Some("NVIDIA"));
    }

    #[test]
    fn with_no_recording_there_are_no_game_figures_and_it_says_so() {
        let o = Overview::build(&scan(), None);
        assert_eq!(o.session, None);
        assert_eq!(o.limiter.title, "Sin datos de partida");
        assert!(o
            .console
            .iter()
            .any(|l| l.tag == "PARTIDA" && l.text.contains("Todavía no")));
    }

    #[test]
    fn the_last_recording_gives_its_readings_and_the_first_lead_is_the_limiter() {
        let o = Overview::build(&scan(), Some(&analysis()));
        let s = o.session.as_ref().unwrap();
        assert_eq!(s.id, "lol-20261118-165329");
        assert_eq!(s.dpc_p95, 2.4);
        assert_eq!(s.league_client_avg, Some(16.5));
        assert!(
            o.limiter.title.contains("núcleo") || o.limiter.title.contains("cliente"),
            "{}",
            o.limiter.title
        );
        assert!(o.console.iter().any(|l| l.tag == "PISTA"));
    }

    #[test]
    fn a_calm_recording_says_nothing_limited_it() {
        let calm = Analysis::of(
            "lol-1",
            parse("t_s,cpu_total,cpu_max_core\n1.0,10.0,20.0\n").unwrap(),
        )
        .unwrap();
        let o = Overview::build(&scan(), Some(&calm));
        assert_eq!(o.limiter.title, "Nada destacable");
    }

    #[test]
    fn the_json_has_stable_names() {
        let value = serde_json::to_value(Overview::build(&scan(), Some(&analysis()))).unwrap();
        assert_eq!(value["devices"]["hardware"], 4);
        assert_eq!(value["session"]["started"], "2026-11-18 16:53:29");
        assert_eq!(value["scan_ms"], 200);
        assert!(value["console"][0]["tag"].is_string());
    }
}
