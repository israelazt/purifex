//! The table of devices and drivers as the interface shows it: identical devices
//! joined in one row, a count for each filter, and what needs attention first.

use serde::Serialize;

use crate::date::Date;
use crate::device::Health;
use crate::device_category::{category_of, Category, ALL_CATEGORIES};
use crate::report::{Entry, ScanReport};
use crate::text::grouped;
use crate::version::DriverVersion;

/// One row of the table: a device, or several identical ones.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DriverRow {
    pub name: String,
    pub class: String,
    pub category: Category,
    pub instance_id: String,
    /// How many identical devices the row stands for.
    pub count: usize,
    pub provider: Option<String>,
    pub version: Option<DriverVersion>,
    pub date: Option<Date>,
    pub age_days: Option<i64>,
    /// The driver is the one Windows ships.
    pub inbox: bool,
    pub health: Health,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CategoryCount {
    pub category: Category,
    pub label: &'static str,
    /// Devices (not rows) of the category.
    pub count: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DriversView {
    /// Devices of hardware, counted one by one.
    pub total: usize,
    /// Devices that need attention (no driver, a fault).
    pub attention: usize,
    /// The categories that have devices, in a fixed order.
    pub categories: Vec<CategoryCount>,
    pub rows: Vec<DriverRow>,
}

impl DriversView {
    pub fn build(report: &ScanReport) -> Self {
        let rows: Vec<DriverRow> = grouped(&report.devices)
            .into_iter()
            .map(|(entry, count)| row_of(entry, count))
            .collect();
        let categories = ALL_CATEGORIES
            .into_iter()
            .filter_map(|category| {
                let count: usize = rows
                    .iter()
                    .filter(|r| r.category == category)
                    .map(|r| r.count)
                    .sum();
                (count > 0).then_some(CategoryCount {
                    category,
                    label: category.label(),
                    count,
                })
            })
            .collect();
        Self {
            total: rows.iter().map(|r| r.count).sum(),
            attention: rows
                .iter()
                .filter(|r| matches!(r.health, Health::MissingDriver | Health::Fault { .. }))
                .map(|r| r.count)
                .sum(),
            categories,
            rows,
        }
    }
}

fn row_of(entry: &Entry, count: usize) -> DriverRow {
    let driver = entry.device.driver.as_ref();
    DriverRow {
        name: entry.device.name.clone(),
        class: entry.device.class.clone(),
        category: category_of(&entry.device.class),
        instance_id: entry.device.instance_id.clone(),
        count,
        provider: driver.and_then(|d| d.provider.clone()),
        version: driver.and_then(|d| d.version),
        date: driver.and_then(|d| d.date),
        age_days: entry.driver_age_days,
        inbox: entry.inbox_driver,
        health: entry.health,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::device::{Device, InstalledDriver};
    use crate::report::Scope;

    fn device(id: &str, class: &str, name: &str, problem: Option<u32>) -> Device {
        Device {
            instance_id: id.into(),
            class: class.into(),
            name: name.into(),
            problem,
            driver: Some(InstalledDriver {
                provider: Some("Microsoft".into()),
                version: DriverVersion::parse("10.0"),
                date: Date::new(2006, 6, 21),
                inf: None,
            }),
            ..Device::default()
        }
    }

    fn view(devices: Vec<Device>) -> DriversView {
        DriversView::build(&ScanReport::build(
            devices,
            Scope::Hardware,
            Date::new(2026, 11, 18).unwrap(),
            5,
        ))
    }

    #[test]
    fn identical_devices_are_one_row_that_counts_them() {
        let v = view(vec![
            device(r"USB\A", "HIDClass", "Entrada USB", None),
            device(r"USB\B", "HIDClass", "Entrada USB", None),
            device(r"PCI\C", "Display", "GPU", None),
        ]);
        assert_eq!(v.rows.len(), 2);
        assert_eq!(v.total, 3);
        assert_eq!(
            v.rows
                .iter()
                .find(|r| r.name == "Entrada USB")
                .unwrap()
                .count,
            2
        );
    }

    #[test]
    fn each_category_counts_devices_not_rows_and_only_those_with_devices_appear() {
        let v = view(vec![
            device(r"USB\A", "HIDClass", "Entrada USB", None),
            device(r"USB\B", "HIDClass", "Entrada USB", None),
            device(r"PCI\C", "Display", "GPU", None),
        ]);
        let counts: Vec<(Category, usize)> =
            v.categories.iter().map(|c| (c.category, c.count)).collect();
        assert_eq!(counts, [(Category::Gpu, 1), (Category::Input, 2)]);
        assert_eq!(v.categories[0].label, "GPU");
    }

    #[test]
    fn what_needs_attention_is_counted_and_comes_first() {
        let v = view(vec![
            device(r"PCI\A", "Display", "GPU", None),
            device(r"USB\B", "Net", "Dongle", Some(28)),
            device(r"PCI\C", "Audio", "Sonido", Some(22)),
        ]);
        assert_eq!(v.attention, 1, "a disabled device is not a fault");
        assert_eq!(v.rows[0].name, "Dongle");
    }

    #[test]
    fn the_json_is_flat_with_stable_names() {
        let v = view(vec![device(r"PCI\A", "Display", "GPU", None)]);
        let value = serde_json::to_value(&v).unwrap();
        assert_eq!(value["rows"][0]["category"], "gpu");
        assert_eq!(value["rows"][0]["version"], "10.0.0.0");
        assert_eq!(value["rows"][0]["health"], "working");
        assert_eq!(value["total"], 1);
    }
}
