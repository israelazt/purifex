//! The result of a scan: the devices with what can be said about each one
//! (its health, how old its driver is, whether it is the one Windows ships) and
//! a summary. These are **facts**: the report never says "update this"; whether
//! it is worth it is a later decision (a driver can be old and perfect).

use serde::Serialize;

use crate::date::Date;
use crate::device::{Device, Health};

/// One device and what is known about it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Entry {
    #[serde(flatten)]
    pub device: Device,
    pub health: Health,
    /// Days between the driver's date and the day of the scan.
    pub driver_age_days: Option<i64>,
    /// The driver is the one Windows ships.
    pub inbox_driver: bool,
}

impl Entry {
    pub fn of(device: Device, today: Date) -> Self {
        let health = device.health();
        let driver_age_days = device
            .driver
            .as_ref()
            .and_then(|d| d.date)
            .map(|date| date.days_until(today));
        let inbox_driver = device.has_inbox_driver();
        Self {
            device,
            health,
            driver_age_days,
            inbox_driver,
        }
    }
}

/// Counts over everything that was scanned (shown or not).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Summary {
    pub scanned: usize,
    pub physical: usize,
    pub shown: usize,
    pub missing_driver: usize,
    pub faults: usize,
    pub disabled: usize,
}

/// What a scan found.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ScanReport {
    pub scanned_on: Date,
    pub elapsed_ms: u64,
    pub summary: Summary,
    pub devices: Vec<Entry>,
}

/// Which devices a report lists.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Scope {
    /// Only hardware (the default).
    #[default]
    Hardware,
    /// Everything the system lists, made-up devices included.
    All,
}

impl ScanReport {
    /// Builds the report: filters by `scope`, puts what needs attention first and
    /// the rest by class and name.
    pub fn build(devices: Vec<Device>, scope: Scope, today: Date, elapsed_ms: u64) -> Self {
        let mut summary = Summary {
            scanned: devices.len(),
            ..Summary::default()
        };
        let mut entries = Vec::with_capacity(devices.len());
        for device in devices {
            let physical = device.is_physical();
            let entry = Entry::of(device, today);
            summary.physical += usize::from(physical);
            if physical {
                match entry.health {
                    Health::MissingDriver => summary.missing_driver += 1,
                    Health::Fault { .. } => summary.faults += 1,
                    Health::Disabled => summary.disabled += 1,
                    Health::Working => {}
                }
            }
            if physical || scope == Scope::All {
                entries.push(entry);
            }
        }
        entries.sort_by(|a, b| {
            (
                needs_attention(a.health),
                a.device.class.to_lowercase(),
                a.device.name.to_lowercase(),
            )
                .cmp(&(
                    needs_attention(b.health),
                    b.device.class.to_lowercase(),
                    b.device.name.to_lowercase(),
                ))
        });
        summary.shown = entries.len();
        Self {
            scanned_on: today,
            elapsed_ms,
            summary,
            devices: entries,
        }
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_else(|_| "{}".into())
    }
}

/// Sort key: what is wrong goes first (`false` sorts before `true`).
fn needs_attention(health: Health) -> bool {
    matches!(health, Health::Working | Health::Disabled)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::device::InstalledDriver;
    use crate::version::DriverVersion;

    fn today() -> Date {
        Date::new(2026, 11, 18).unwrap()
    }

    fn device(id: &str, class: &str, name: &str, problem: Option<u32>) -> Device {
        Device {
            instance_id: id.into(),
            class: class.into(),
            name: name.into(),
            problem,
            ..Device::default()
        }
    }

    #[test]
    fn the_age_of_a_driver_is_counted_from_its_date_to_the_scan() {
        let mut d = device("PCI\\A", "Net", "Wi-Fi", None);
        d.driver = Some(InstalledDriver {
            date: Date::new(2026, 11, 8),
            version: DriverVersion::parse("1.0"),
            ..Default::default()
        });
        assert_eq!(Entry::of(d, today()).driver_age_days, Some(10));
        assert_eq!(
            Entry::of(device("PCI\\B", "Net", "x", None), today()).driver_age_days,
            None
        );
    }

    #[test]
    fn by_default_only_hardware_is_listed_but_everything_is_counted() {
        let devices = vec![
            device("PCI\\A", "Display", "GPU", None),
            device("ROOT\\B", "System", "Virtual", None),
        ];
        let report = ScanReport::build(devices.clone(), Scope::Hardware, today(), 3);
        assert_eq!(
            (
                report.summary.scanned,
                report.summary.physical,
                report.summary.shown
            ),
            (2, 1, 1)
        );
        let all = ScanReport::build(devices, Scope::All, today(), 3);
        assert_eq!(all.summary.shown, 2);
    }

    #[test]
    fn what_needs_attention_goes_first_and_is_counted() {
        let devices = vec![
            device("PCI\\A", "Display", "GPU", None),
            device("USB\\B", "Net", "Dongle", Some(28)),
            device("PCI\\C", "Audio", "Sound", Some(43)),
            device("PCI\\D", "Audio", "Old", Some(22)),
        ];
        let report = ScanReport::build(devices, Scope::Hardware, today(), 1);
        let names: Vec<_> = report
            .devices
            .iter()
            .map(|e| e.device.name.as_str())
            .collect();
        assert_eq!(names, ["Sound", "Dongle", "Old", "GPU"]);
        assert_eq!(
            (
                report.summary.missing_driver,
                report.summary.faults,
                report.summary.disabled
            ),
            (1, 1, 1)
        );
    }

    #[test]
    fn the_json_is_flat_and_has_stable_names() {
        let mut d = device("PCI\\A", "Display", "GPU", None);
        d.driver = Some(InstalledDriver {
            provider: Some("NVIDIA".into()),
            version: DriverVersion::parse("31.0.101.5186"),
            date: Date::new(2025, 3, 4),
            inf: Some("oem1.inf".into()),
        });
        let json = ScanReport::build(vec![d], Scope::Hardware, today(), 2).to_json();
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        let first = &value["devices"][0];
        assert_eq!(first["name"], "GPU");
        assert_eq!(first["driver"]["version"], "31.0.101.5186");
        assert_eq!(first["driver"]["date"], "2025-03-04");
        assert_eq!(first["health"], "working");
        assert_eq!(first["inbox_driver"], false);
        assert_eq!(value["scanned_on"], "2026-11-18");
        assert_eq!(value["summary"]["shown"], 1);
    }
}
