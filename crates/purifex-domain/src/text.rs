//! The report as text for a terminal. Pure: it returns the string, the program
//! prints it. (The texts are in Spanish for now; they move to a table of
//! strings with the interface, as in Luminifex's `i18n`.)

use std::fmt::Write;

use crate::device::Health;
use crate::report::{Entry, ScanReport};

const NAME_WIDTH: usize = 38;
const CLASS_WIDTH: usize = 12;
const PROVIDER_WIDTH: usize = 20;

impl ScanReport {
    pub fn to_text(&self) -> String {
        let s = &self.summary;
        let mut out = String::new();
        let _ = writeln!(
            out,
            "Purifex · análisis del {} · {} ms · {} dispositivos leídos, {} son hardware",
            self.scanned_on, self.elapsed_ms, s.scanned, s.physical
        );
        let _ = writeln!(
            out,
            "Sin driver: {} · con fallo: {} · desactivados: {}\n",
            s.missing_driver, s.faults, s.disabled
        );
        let _ = writeln!(
            out,
            "{:<3}{:<NAME_WIDTH$} {:<CLASS_WIDTH$} {:<15} {:<10} {:>8}  Proveedor",
            "", "Dispositivo", "Clase", "Versión", "Fecha", "Edad"
        );
        for (entry, count) in grouped(&self.devices) {
            let _ = writeln!(out, "{}", row(entry, count));
        }
        out
    }
}

/// What makes two entries the same line of the table.
fn same_line(a: &Entry, b: &Entry) -> bool {
    let (da, db) = (a.device.driver.as_ref(), b.device.driver.as_ref());
    a.device.name == b.device.name
        && a.device.class == b.device.class
        && a.health == b.health
        && da.map(|d| (&d.provider, d.version, d.date))
            == db.map(|d| (&d.provider, d.version, d.date))
}

/// The entries with the identical ones joined (eight "USB input device" are one
/// line, "×8"), in the order of the first of each. The report itself keeps them
/// all: the JSON lists every device.
pub(crate) fn grouped(entries: &[Entry]) -> Vec<(&Entry, usize)> {
    let mut lines: Vec<(&Entry, usize)> = Vec::new();
    for entry in entries {
        match lines.iter_mut().find(|(first, _)| same_line(first, entry)) {
            Some((_, count)) => *count += 1,
            None => lines.push((entry, 1)),
        }
    }
    lines
}

fn row(entry: &Entry, count: usize) -> String {
    let driver = entry.device.driver.as_ref();
    let version = driver
        .and_then(|d| d.version)
        .map_or_else(|| "-".to_string(), |v| v.to_string());
    let date = driver
        .and_then(|d| d.date)
        .map_or_else(|| "-".to_string(), |d| d.to_string());
    let provider = driver.and_then(|d| d.provider.as_deref()).unwrap_or("-");
    let marker = match entry.health {
        Health::Working => "",
        Health::MissingDriver => "!",
        Health::Disabled => "x",
        Health::Fault { .. } => "!",
    };
    let mut line = format!(
        "{:<3}{:<NAME_WIDTH$} {:<CLASS_WIDTH$} {:<15} {:<10} {:>8}  {}",
        marker,
        name_with_count(&entry.device.name, count),
        cut(&entry.device.class, CLASS_WIDTH),
        version,
        date,
        entry.driver_age_days.map_or_else(|| "-".to_string(), age),
        cut(provider, PROVIDER_WIDTH),
    );
    match entry.health {
        Health::MissingDriver => line.push_str("  [sin driver]"),
        Health::Fault { code } => {
            let _ = write!(line, "  [fallo, código {code}]");
        }
        Health::Disabled => line.push_str("  [desactivado]"),
        Health::Working => {}
    }
    line
}

/// The name, cut to its column, with `×N` when several devices share the line.
fn name_with_count(name: &str, count: usize) -> String {
    if count < 2 {
        return cut(name, NAME_WIDTH);
    }
    let suffix = format!(" ×{count}");
    let room = NAME_WIDTH.saturating_sub(suffix.chars().count());
    format!("{}{suffix}", cut(name, room))
}

/// `3.2a` / `5m` / `12d` for the age of a driver given in days.
fn age(days: i64) -> String {
    if days < 0 {
        return "-".into();
    }
    if days >= 365 {
        format!("{:.1}a", days as f64 / 365.25)
    } else if days >= 30 {
        format!("{}m", days / 30)
    } else {
        format!("{days}d")
    }
}

/// `text` cut to `width` characters, with `…` when something was left out.
fn cut(text: &str, width: usize) -> String {
    if text.chars().count() <= width {
        return text.to_string();
    }
    let mut short: String = text.chars().take(width - 1).collect();
    short.push('…');
    short
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::date::Date;
    use crate::device::{Device, InstalledDriver};
    use crate::report::Scope;
    use crate::version::DriverVersion;

    #[test]
    fn ages_read_in_the_unit_that_fits() {
        assert_eq!(age(12), "12d");
        assert_eq!(age(95), "3m");
        assert_eq!(age(730), "2.0a");
        assert_eq!(age(-4), "-");
    }

    #[test]
    fn long_names_are_cut_with_an_ellipsis() {
        assert_eq!(cut("short", 10), "short");
        assert_eq!(cut("abcdefghij", 5), "abcd…");
        assert_eq!(cut("ñandú-ñandú", 5).chars().count(), 5);
    }

    fn usb_input() -> Device {
        Device {
            instance_id: r"USB\X".into(),
            name: "Dispositivo de entrada USB".into(),
            class: "HIDClass".into(),
            driver: Some(InstalledDriver {
                provider: Some("Microsoft".into()),
                version: DriverVersion::parse("10.0.26100.9974"),
                date: Date::new(2006, 6, 21),
                inf: None,
            }),
            ..Device::default()
        }
    }

    #[test]
    fn identical_devices_are_one_line_with_their_count_but_the_json_keeps_all() {
        let mut broken = usb_input();
        broken.problem = Some(28);
        let devices = vec![usb_input(), usb_input(), usb_input(), broken];
        let report = ScanReport::build(
            devices,
            Scope::Hardware,
            Date::new(2026, 11, 18).unwrap(),
            1,
        );
        let text = report.to_text();
        assert_eq!(
            text.matches("Dispositivo de entrada USB").count(),
            2,
            "{text}"
        );
        assert!(text.contains("USB ×3"), "{text}");
        let json: serde_json::Value = serde_json::from_str(&report.to_json()).unwrap();
        assert_eq!(json["devices"].as_array().unwrap().len(), 4);
    }

    #[test]
    fn a_long_name_with_a_count_still_fits_its_column() {
        let name = name_with_count("Dispositivo definido por el proveedor de HID muy largo", 12);
        assert_eq!(name.chars().count(), NAME_WIDTH);
        assert!(name.ends_with("…") || name.ends_with("×12"));
        assert!(name.ends_with(" ×12"));
    }

    #[test]
    fn the_text_lists_each_device_and_flags_the_ones_with_problems() {
        let gpu = Device {
            instance_id: r"PCI\A".into(),
            name: "NVIDIA GeForce RTX 3060".into(),
            class: "Display".into(),
            driver: Some(InstalledDriver {
                provider: Some("NVIDIA".into()),
                version: DriverVersion::parse("31.0.101.5186"),
                date: Date::new(2025, 3, 4),
                inf: None,
            }),
            ..Device::default()
        };
        let broken = Device {
            instance_id: r"USB\B".into(),
            name: "Dongle".into(),
            class: "Net".into(),
            problem: Some(28),
            ..Device::default()
        };
        let report = ScanReport::build(
            vec![gpu, broken],
            Scope::Hardware,
            Date::new(2026, 11, 18).unwrap(),
            4,
        );
        let text = report.to_text();
        assert!(text.contains("4 ms"));
        assert!(text.contains("Sin driver: 1"));
        assert!(text.contains("31.0.101.5186"));
        assert!(text.contains("[sin driver]"));
        let broken_at = text.find("Dongle").unwrap();
        let gpu_at = text.find("RTX 3060").unwrap();
        assert!(broken_at < gpu_at, "the problem goes first");
    }
}
