//! A device of the computer and the driver it runs: plain facts, as the
//! system reports them. Judging them is the job of [`crate::report`].

use serde::Serialize;

use crate::date::Date;
use crate::version::DriverVersion;

/// The driver installed for a device. Every field may be unknown.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct InstalledDriver {
    pub provider: Option<String>,
    pub version: Option<DriverVersion>,
    pub date: Option<Date>,
    /// The `.inf` it was installed from (`oem42.inf`).
    pub inf: Option<String>,
}

/// A device present in the computer.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Device {
    /// `PCI\VEN_10DE&DEV_2504&SUBSYS_...\4&...`
    pub instance_id: String,
    pub name: String,
    /// The device class (`Display`, `Net`, `MEDIA`…), as the system names it.
    pub class: String,
    pub manufacturer: Option<String>,
    /// Most specific first, as the system lists them.
    pub hardware_ids: Vec<String>,
    pub driver: Option<InstalledDriver>,
    /// The problem code of the device manager (0 or `None` = none).
    pub problem: Option<u32>,
}

/// How a device is doing, from its problem code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Health {
    Working,
    /// Codes 1 and 28: the device has no driver.
    MissingDriver,
    /// Code 22: switched off by the user.
    Disabled,
    /// Any other code: the device manager reports a fault.
    Fault {
        code: u32,
    },
}

const CM_PROB_NOT_CONFIGURED: u32 = 1;
const CM_PROB_DISABLED: u32 = 22;
const CM_PROB_FAILED_INSTALL: u32 = 28;

impl Device {
    /// The bus it hangs from: the start of its instance id (`PCI`, `USB`,
    /// `ROOT`, `SWD`…), in capitals.
    pub fn bus(&self) -> String {
        self.instance_id
            .split('\\')
            .next()
            .unwrap_or("")
            .to_ascii_uppercase()
    }

    /// Whether it is a piece of hardware, and not something the system makes up
    /// (root-enumerated and software devices, volumes, other virtual things).
    pub fn is_physical(&self) -> bool {
        !matches!(
            self.bus().as_str(),
            "" | "ROOT" | "SWD" | "STORAGE" | "UMB" | "HTREE" | "TERMINPUT_BUS"
        )
    }

    pub fn health(&self) -> Health {
        match self.problem {
            None | Some(0) => Health::Working,
            Some(CM_PROB_NOT_CONFIGURED | CM_PROB_FAILED_INSTALL) => Health::MissingDriver,
            Some(CM_PROB_DISABLED) => Health::Disabled,
            Some(code) => Health::Fault { code },
        }
    }

    /// The driver comes with Windows itself (its provider is Microsoft).
    pub fn has_inbox_driver(&self) -> bool {
        self.driver
            .as_ref()
            .and_then(|d| d.provider.as_deref())
            .is_some_and(|p| p.eq_ignore_ascii_case("Microsoft"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn device(id: &str, problem: Option<u32>) -> Device {
        Device {
            instance_id: id.into(),
            problem,
            ..Device::default()
        }
    }

    #[test]
    fn the_bus_is_the_start_of_the_instance_id() {
        assert_eq!(device(r"pci\ven_10de&dev_2504\4&1", None).bus(), "PCI");
        assert_eq!(device("", None).bus(), "");
    }

    #[test]
    fn made_up_devices_are_not_physical() {
        for id in [
            r"ROOT\VMBUS\0000",
            r"SWD\MMDEVAPI\x",
            r"STORAGE\Volume\x",
            "",
        ] {
            assert!(!device(id, None).is_physical(), "{id}");
        }
        for id in [
            r"PCI\VEN_1",
            r"USB\VID_1",
            r"HDAUDIO\FUNC_01",
            r"ACPI\PNP0C0A",
        ] {
            assert!(device(id, None).is_physical(), "{id}");
        }
    }

    #[test]
    fn the_problem_code_says_how_it_is_doing() {
        assert_eq!(device("PCI\\A", None).health(), Health::Working);
        assert_eq!(device("PCI\\A", Some(0)).health(), Health::Working);
        assert_eq!(device("PCI\\A", Some(28)).health(), Health::MissingDriver);
        assert_eq!(device("PCI\\A", Some(1)).health(), Health::MissingDriver);
        assert_eq!(device("PCI\\A", Some(22)).health(), Health::Disabled);
        assert_eq!(
            device("PCI\\A", Some(43)).health(),
            Health::Fault { code: 43 }
        );
    }

    #[test]
    fn it_knows_a_driver_that_comes_with_windows() {
        let mut d = device("PCI\\A", None);
        assert!(!d.has_inbox_driver(), "no driver at all");
        d.driver = Some(InstalledDriver {
            provider: Some("microsoft".into()),
            ..Default::default()
        });
        assert!(d.has_inbox_driver());
        d.driver.as_mut().unwrap().provider = Some("NVIDIA".into());
        assert!(!d.has_inbox_driver());
    }
}
