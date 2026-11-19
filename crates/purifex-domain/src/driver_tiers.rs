//! Who can resolve the driver of each device: Windows Update (when it offers one),
//! a manufacturer's own catalog, or a search on the web by the user's AI. The
//! split is decided here, by rules, so the AI is only asked about the devices
//! nobody else covers and that really need something.

use serde::Serialize;

use crate::date::Date;
use crate::device::{Device, Health};
use crate::driver_offers::{match_offers, DeviceOffer, Offer, OfferReport};
use crate::version::DriverVersion;

/// A driver this old (days), from a maker other than Microsoft, is worth looking at.
const OLD_DRIVER_DAYS: i64 = 730;

/// Manufacturers with a catalog that code can read (phase 6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Vendor {
    Nvidia,
    Amd,
    Intel,
}

/// The manufacturer of the chip, from the vendor id in its hardware ids.
pub fn vendor_of(device: &Device) -> Option<Vendor> {
    device.hardware_ids.iter().find_map(|id| {
        let id = id.to_ascii_uppercase();
        let ven = id.strip_prefix("PCI\\VEN_")?.get(..4)?.to_string();
        match ven.as_str() {
            "10DE" => Some(Vendor::Nvidia),
            "1002" | "1022" => Some(Vendor::Amd),
            "8086" => Some(Vendor::Intel),
            _ => None,
        }
    })
}

/// Why a device is worth searching a driver for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "reason", rename_all = "snake_case")]
pub enum Need {
    MissingDriver,
    Fault { code: u32 },
    OldDriver { days: i64 },
}

fn need_of(device: &Device, today: Date) -> Option<Need> {
    match device.health() {
        Health::MissingDriver => Some(Need::MissingDriver),
        Health::Fault { code } => Some(Need::Fault { code }),
        Health::Disabled => None,
        Health::Working => {
            let date = device.driver.as_ref()?.date?;
            let days = date.days_until(today);
            (!device.has_inbox_driver() && days >= OLD_DRIVER_DAYS)
                .then_some(Need::OldDriver { days })
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tier {
    /// Windows Update offers a newer driver.
    WindowsOffers,
    /// A manufacturer catalog covers it.
    VendorCatalog(Vendor),
    /// Needs a search on the web (by the user's AI, through MCP).
    NeedsSearch(Need),
    /// Nothing to do.
    Fine,
}

pub fn tier_of(device: &Device, has_offer: bool, today: Date) -> Tier {
    if has_offer {
        return Tier::WindowsOffers;
    }
    if let Some(vendor) = vendor_of(device) {
        return Tier::VendorCatalog(vendor);
    }
    need_of(device, today).map_or(Tier::Fine, Tier::NeedsSearch)
}

/// A device that nobody else covers and that needs a driver or a newer one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Unresolved {
    pub instance_id: String,
    pub name: String,
    pub class: String,
    pub hardware_ids: Vec<String>,
    pub installed: Option<DriverVersion>,
    pub need: Need,
}

/// The offers for the physical `devices`, and who covers the rest.
pub fn report(devices: &[Device], offers: &[Offer], today: Date) -> OfferReport {
    let found: Vec<DeviceOffer> = match_offers(devices, offers);
    let mut vendor_covered = 0;
    let mut unresolved = Vec::new();
    for device in devices {
        let has_offer = found.iter().any(|o| o.instance_id == device.instance_id);
        match tier_of(device, has_offer, today) {
            Tier::VendorCatalog(_) => vendor_covered += 1,
            Tier::NeedsSearch(need) => unresolved.push(Unresolved {
                instance_id: device.instance_id.clone(),
                name: device.name.clone(),
                class: device.class.clone(),
                hardware_ids: device.hardware_ids.clone(),
                installed: device.driver.as_ref().and_then(|d| d.version),
                need,
            }),
            Tier::WindowsOffers | Tier::Fine => {}
        }
    }
    unresolved.sort_by(|a, b| a.name.cmp(&b.name));
    OfferReport {
        offers: found,
        unresolved,
        vendor_covered,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::device::InstalledDriver;

    fn today() -> Date {
        Date::new(2026, 11, 18).unwrap()
    }

    fn dev(
        name: &str,
        hw: &str,
        provider: &str,
        date: Option<Date>,
        problem: Option<u32>,
    ) -> Device {
        Device {
            instance_id: format!("{hw}\\1"),
            name: name.into(),
            class: "Net".into(),
            hardware_ids: vec![hw.into()],
            driver: Some(InstalledDriver {
                provider: Some(provider.into()),
                version: DriverVersion::parse("1.0.0.0"),
                date,
                ..InstalledDriver::default()
            }),
            problem,
            ..Device::default()
        }
    }

    #[test]
    fn the_vendor_comes_from_the_vendor_id_of_the_chip() {
        let nv = dev("g", r"PCI\VEN_10DE&DEV_1", "NVIDIA", None, None);
        assert_eq!(vendor_of(&nv), Some(Vendor::Nvidia));
        assert_eq!(
            vendor_of(&dev("a", r"pci\ven_1002&dev_1", "AMD", None, None)),
            Some(Vendor::Amd)
        );
        assert_eq!(
            vendor_of(&dev("i", r"PCI\VEN_8086&DEV_1", "Intel", None, None)),
            Some(Vendor::Intel)
        );
        assert_eq!(
            vendor_of(&dev("r", r"USB\VID_0BDA&PID_A728", "Realtek", None, None)),
            None
        );
        assert_eq!(
            vendor_of(&dev("r", r"PCI\VEN_10EC&DEV_8168", "Realtek", None, None)),
            None
        );
    }

    #[test]
    fn a_fault_or_a_missing_driver_with_nobody_covering_it_goes_to_the_search() {
        let bt = dev(
            "Realtek BT",
            r"USB\VID_0BDA&PID_A728",
            "Realtek",
            Date::new(2024, 3, 20),
            Some(31),
        );
        assert_eq!(
            tier_of(&bt, false, today()),
            Tier::NeedsSearch(Need::Fault { code: 31 })
        );
        let none = dev("X", r"USB\VID_1&PID_2", "Acme", None, Some(28));
        assert_eq!(
            tier_of(&none, false, today()),
            Tier::NeedsSearch(Need::MissingDriver)
        );
    }

    #[test]
    fn an_old_driver_of_a_maker_is_searched_but_a_young_one_or_the_one_of_windows_is_not() {
        let old = dev(
            "Audio",
            r"PCI\VEN_10EC&DEV_1",
            "Realtek",
            Date::new(2022, 1, 1),
            None,
        );
        assert!(matches!(
            tier_of(&old, false, today()),
            Tier::NeedsSearch(Need::OldDriver { .. })
        ));
        let young = dev(
            "Audio",
            r"PCI\VEN_10EC&DEV_1",
            "Realtek",
            Date::new(2026, 1, 1),
            None,
        );
        assert_eq!(tier_of(&young, false, today()), Tier::Fine);
        let inbox = dev(
            "Audio",
            r"PCI\VEN_10EC&DEV_1",
            "Microsoft",
            Date::new(2006, 6, 21),
            None,
        );
        assert_eq!(
            tier_of(&inbox, false, today()),
            Tier::Fine,
            "Windows keeps its own up to date"
        );
        let no_data = Device {
            driver: None,
            ..dev("Mouse", r"BTH\X", "", None, None)
        };
        assert_eq!(
            tier_of(&no_data, false, today()),
            Tier::Fine,
            "no driver data is not a driver problem"
        );
        let off = dev("Off", r"USB\X", "Acme", Date::new(2010, 1, 1), Some(22));
        assert_eq!(
            tier_of(&off, false, today()),
            Tier::Fine,
            "switched off by the user"
        );
    }

    #[test]
    fn windows_goes_first_then_the_vendor_catalog() {
        let gpu = dev(
            "GPU",
            r"PCI\VEN_10DE&DEV_1",
            "NVIDIA",
            Date::new(2020, 1, 1),
            None,
        );
        assert_eq!(tier_of(&gpu, true, today()), Tier::WindowsOffers);
        assert_eq!(
            tier_of(&gpu, false, today()),
            Tier::VendorCatalog(Vendor::Nvidia)
        );
    }

    #[test]
    fn the_report_splits_the_devices_and_orders_the_unresolved() {
        let devices = vec![
            dev(
                "Zeta BT",
                r"USB\VID_0BDA&PID_A728",
                "Realtek",
                None,
                Some(31),
            ),
            dev("GPU", r"PCI\VEN_10DE&DEV_1", "NVIDIA", None, None),
            dev("Alfa", r"USB\VID_9&PID_9", "Acme", None, Some(28)),
            dev(
                "Fine",
                r"USB\VID_8&PID_8",
                "Acme",
                Date::new(2026, 5, 1),
                None,
            ),
        ];
        let r = report(&devices, &[], today());
        assert_eq!(r.vendor_covered, 1);
        assert_eq!(
            r.unresolved
                .iter()
                .map(|u| u.name.as_str())
                .collect::<Vec<_>>(),
            ["Alfa", "Zeta BT"]
        );
        assert!(r.offers.is_empty());
        assert_eq!(
            r.unresolved[1].hardware_ids,
            vec![r"USB\VID_0BDA&PID_A728".to_string()]
        );
    }
}
