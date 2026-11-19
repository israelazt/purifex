//! What Windows Update offers for the drivers of this computer, matched to the
//! devices it is for. Reading the offers changes nothing: they are facts for
//! the table of drivers (and, later, for the verdict of `driver_candidate`).

use serde::Serialize;

use crate::date::Date;
use crate::device::Device;
use crate::driver_candidate::{risk_of, Risk};
use crate::version::DriverVersion;

/// One driver offered by Windows Update, as it reports it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Offer {
    /// The hardware id of the device it is for.
    pub hardware_id: String,
    /// `NVIDIA - Display - 32.0.15.6094`
    pub title: String,
    pub provider: String,
    pub version: Option<DriverVersion>,
    pub date: Option<Date>,
}

/// An offer that is newer than the driver of the device it is for.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DeviceOffer {
    pub instance_id: String,
    pub name: String,
    pub class: String,
    pub title: String,
    pub provider: String,
    pub version: Option<DriverVersion>,
    pub date: Option<Date>,
    pub risk: Risk,
}

/// The state of the search for offers, as the interface shows it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum Offers {
    /// Not asked: nothing leaves the computer until the user asks.
    #[default]
    NotAsked,
    Searching,
    Found(Vec<DeviceOffer>),
    Failed(String),
}

impl Offers {
    /// The offer for a device row (identical devices share name and class).
    pub fn for_device(&self, name: &str, class: &str) -> Option<&DeviceOffer> {
        match self {
            Self::Found(found) => found.iter().find(|o| o.name == name && o.class == class),
            _ => None,
        }
    }
}

/// The version that the title of an update carries (`… - 32.0.15.6094`): the last
/// word that reads as a version with at least three numbers.
pub fn version_in_title(title: &str) -> Option<DriverVersion> {
    title.split_whitespace().rev().find_map(|word| {
        let word = word.trim_matches(|c: char| !c.is_ascii_digit());
        (word.matches('.').count() >= 2)
            .then(|| DriverVersion::parse(word))
            .flatten()
    })
}

/// Whether `offer` is better than what `device` runs. With no driver at all,
/// any offer is. With one, the versions are compared, or the dates when a
/// version is missing; if neither can be compared it is not claimed to be newer.
fn is_newer(device: &Device, offer: &Offer) -> bool {
    let Some(driver) = &device.driver else {
        return true;
    };
    match (offer.version, driver.version, offer.date, driver.date) {
        (Some(new), Some(old), _, _) => new > old,
        (_, _, Some(new), Some(old)) => new > old,
        (_, None, _, None) => device.driver.as_ref().is_some_and(|d| d.provider.is_none()),
        _ => false,
    }
}

/// The offers that apply to `devices` and are newer than what they run, the best
/// one per device, ordered by name.
pub fn match_offers(devices: &[Device], offers: &[Offer]) -> Vec<DeviceOffer> {
    let mut found: Vec<DeviceOffer> = Vec::new();
    for device in devices {
        let best = offers
            .iter()
            .filter(|o| {
                device
                    .hardware_ids
                    .iter()
                    .any(|id| id.eq_ignore_ascii_case(o.hardware_id.trim()))
            })
            .filter(|o| is_newer(device, o))
            .max_by_key(|o| (o.version, o.date));
        if let Some(o) = best {
            found.push(DeviceOffer {
                instance_id: device.instance_id.clone(),
                name: device.name.clone(),
                class: device.class.clone(),
                title: o.title.clone(),
                provider: o.provider.clone(),
                version: o.version,
                date: o.date,
                risk: risk_of(device),
            });
        }
    }
    found.sort_by(|a, b| a.name.cmp(&b.name).then(a.instance_id.cmp(&b.instance_id)));
    found
}

/// The offers as text for the command line.
pub fn offers_to_text(offers: &[DeviceOffer]) -> String {
    use std::fmt::Write;
    if offers.is_empty() {
        return "Windows Update no ofrece drivers más nuevos que los instalados.".into();
    }
    let mut out = String::new();
    for o in offers {
        let version = o
            .version
            .map_or("versión sin indicar".to_string(), |v| v.to_string());
        let _ = writeln!(out, "• {} ({})", o.name, o.class);
        let _ = writeln!(out, "  ofrece: {} — {version}", o.title);
        if o.risk == Risk::Sensitive {
            let _ = writeln!(
                out,
                "  cuidado: pantalla, red, disco o sistema; no se toca sin pedirlo expresamente"
            );
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::device::InstalledDriver;

    fn v(text: &str) -> Option<DriverVersion> {
        DriverVersion::parse(text)
    }

    fn device(name: &str, hw: &str, class: &str, installed: Option<&str>) -> Device {
        Device {
            instance_id: format!(r"PCI\{hw}\1"),
            name: name.into(),
            class: class.into(),
            hardware_ids: vec![format!("PCI\\{hw}&SUBSYS_1"), format!("PCI\\{hw}")],
            driver: installed.map(|i| InstalledDriver {
                version: v(i),
                provider: Some("X".into()),
                ..InstalledDriver::default()
            }),
            ..Device::default()
        }
    }

    fn offer(hw: &str, version: &str) -> Offer {
        Offer {
            hardware_id: format!("PCI\\{hw}"),
            title: format!("Vendor - Class - {version}"),
            provider: "Vendor".into(),
            version: v(version),
            date: None,
        }
    }

    #[test]
    fn the_version_is_read_from_the_title_of_the_update() {
        assert_eq!(
            version_in_title("NVIDIA - Display - 32.0.15.6094"),
            v("32.0.15.6094")
        );
        assert_eq!(
            version_in_title("Realtek Semiconductor Corp. - MEDIA - 6.0.9.7000 (beta)"),
            v("6.0.9.7000")
        );
        assert_eq!(version_in_title("Intel - System - 10.1.1"), v("10.1.1"));
        assert_eq!(version_in_title("Intel Corporation - Bluetooth"), None);
        assert_eq!(
            version_in_title("Driver 2.0"),
            None,
            "two numbers are not a driver version"
        );
        assert_eq!(version_in_title(""), None);
    }

    #[test]
    fn only_a_newer_offer_for_the_same_hardware_is_kept() {
        let devices = [device(
            "GPU",
            "VEN_10DE&DEV_2182",
            "Display",
            Some("30.0.1.0"),
        )];
        let found = match_offers(
            &devices,
            &[
                offer("VEN_10DE&DEV_2182", "32.0.1.0"),
                offer("VEN_10DE&DEV_9999", "99.0.0.0"),
            ],
        );
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].version, v("32.0.1.0"));
        assert_eq!(found[0].risk, Risk::Sensitive);
        assert!(
            match_offers(&devices, &[offer("VEN_10DE&DEV_2182", "30.0.1.0")]).is_empty(),
            "equal is not newer"
        );
        assert!(match_offers(&devices, &[offer("VEN_10DE&DEV_2182", "29.0.0.0")]).is_empty());
    }

    #[test]
    fn the_best_of_several_offers_wins_and_ids_match_without_regard_to_case() {
        let devices = [device(
            "Audio",
            "VEN_10EC&DEV_0887",
            "MEDIA",
            Some("1.0.0.0"),
        )];
        let mut low = offer("VEN_10EC&DEV_0887", "2.0.0.0");
        low.hardware_id = low.hardware_id.to_lowercase();
        let found = match_offers(&devices, &[low, offer("VEN_10EC&DEV_0887", "3.0.0.0")]);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].version, v("3.0.0.0"));
        assert_eq!(found[0].risk, Risk::Normal);
    }

    #[test]
    fn a_device_without_driver_takes_any_offer_and_the_results_are_ordered() {
        let devices = [
            device("Zeta", "VEN_1&DEV_1", "Net", None),
            device("Alfa", "VEN_2&DEV_2", "Bluetooth", None),
        ];
        let found = match_offers(
            &devices,
            &[
                offer("VEN_1&DEV_1", "1.0.0.0"),
                offer("VEN_2&DEV_2", "1.0.0.0"),
            ],
        );
        assert_eq!(
            found.iter().map(|o| o.name.as_str()).collect::<Vec<_>>(),
            ["Alfa", "Zeta"]
        );
    }

    #[test]
    fn with_no_version_the_dates_decide_and_with_nothing_to_compare_nothing_is_claimed() {
        let mut d = device("GPU", "VEN_1&DEV_1", "MEDIA", Some("1.0.0.0"));
        d.driver.as_mut().unwrap().version = None;
        d.driver.as_mut().unwrap().date = Date::new(2024, 1, 1);
        let mut o = offer("VEN_1&DEV_1", "1.0.0.0");
        o.version = None;
        assert!(
            match_offers(&[d.clone()], &[o.clone()]).is_empty(),
            "no dates on the offer"
        );
        o.date = Date::new(2025, 1, 1);
        assert_eq!(match_offers(&[d.clone()], &[o.clone()]).len(), 1);
        o.date = Date::new(2023, 1, 1);
        assert!(match_offers(&[d], &[o]).is_empty());
    }

    #[test]
    fn nothing_is_asked_until_the_user_asks() {
        assert_eq!(Offers::default(), Offers::NotAsked);
    }
}
