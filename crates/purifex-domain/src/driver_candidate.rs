//! A driver someone proposes for a device, and the rules that say what Purifex
//! may do with it. The proposal can come from an AI that searched the web, so
//! nothing in it is believed: the verdict is a pure function of what the code
//! could check (the address, the version, the hardware ids, and later the hash and
//! the signature), never of how convincing the proposal is.

use serde::Serialize;

use crate::device::Device;
use crate::version::DriverVersion;

/// Where a candidate comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    /// Windows Update / the Microsoft catalog, found by hardware id.
    WindowsUpdate,
    /// A manufacturer's own catalog, read by code.
    Vendor,
    /// Found by an AI on the web (through the MCP server). Never installed from
    /// Purifex: at best, its official page is opened.
    Suggested,
}

/// A proposal for one device.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Candidate {
    /// `Device::instance_id` of the device it is for.
    pub device: String,
    pub source: Source,
    /// Where it is downloaded from, or the page where it is offered.
    pub url: String,
    pub version: DriverVersion,
}

/// What the code has checked about a candidate so far. `None` = not checked yet
/// (it needs the download); `Some(false)` = checked and it failed.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Checks {
    /// The hardware ids listed by the `.inf` of the package.
    pub inf_ids: Option<Vec<String>>,
    /// The hash of the file equals the one the manufacturer published.
    pub hash_matches: Option<bool>,
    /// The signature of the package (and its catalog) is valid.
    pub signature_valid: Option<bool>,
}

/// Why a candidate was turned down.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Reason {
    NotHttps,
    /// The address is odd: user info, a port, non-ASCII letters, no host.
    BadAddress,
    UnknownDomain,
    NotNewer,
    WrongHardware,
    HashMismatch,
    BadSignature,
}

/// What a candidate for a device is worth.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "verdict", rename_all = "snake_case")]
pub enum Verdict {
    /// Every check passed: it can be installed (with confirmation and a restore point).
    Installable,
    /// Nothing is wrong so far, but some checks need the download.
    Pending {
        missing: Vec<&'static str>,
    },
    /// The address is an official page: Purifex may open it, not install from it.
    OfficialPage,
    Rejected {
        reasons: Vec<Reason>,
    },
}

/// How careful to be with a device: a bad driver here can leave the PC without
/// screen, network or disk.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Risk {
    Normal,
    /// Not touched unless the user asks for it by name.
    Sensitive,
}

const SENSITIVE_CLASSES: [&str; 6] = [
    "display",
    "net",
    "hdc",
    "scsiadapter",
    "system",
    "processor",
];

pub fn risk_of(device: &Device) -> Risk {
    if SENSITIVE_CLASSES.contains(&device.class.to_ascii_lowercase().as_str()) {
        Risk::Sensitive
    } else {
        Risk::Normal
    }
}

/// Domains of the manufacturers a driver may be taken from. A host is allowed when
/// it is one of these or a subdomain of one.
const OFFICIAL_DOMAINS: [&str; 14] = [
    "nvidia.com",
    "amd.com",
    "intel.com",
    "realtek.com",
    "microsoft.com",
    "dell.com",
    "hp.com",
    "lenovo.com",
    "asus.com",
    "msi.com",
    "gigabyte.com",
    "mediatek.com",
    "qualcomm.com",
    "logitech.com",
];

/// The host of an `https` address, or why it is not acceptable. Strict on purpose:
/// `https://nvidia.com@evil.io`, `https://nvidia.com.evil.io` and look-alike
/// letters must not pass.
fn https_host(url: &str) -> Result<String, Reason> {
    let rest = url.strip_prefix("https://").ok_or(Reason::NotHttps)?;
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    let host = authority.to_ascii_lowercase();
    let valid = !host.is_empty()
        && host.is_ascii()
        && host
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'.')
        && !host.starts_with('.')
        && !host.ends_with('.')
        && !host.contains("..");
    if valid {
        Ok(host)
    } else {
        Err(Reason::BadAddress)
    }
}

fn is_official(host: &str) -> bool {
    OFFICIAL_DOMAINS
        .iter()
        .any(|d| host == *d || host.strip_suffix(d).is_some_and(|p| p.ends_with('.')))
}

/// What may be done with `candidate` for `device`, given what has been checked.
pub fn judge(device: &Device, candidate: &Candidate, checks: &Checks) -> Verdict {
    let mut reasons = Vec::new();
    match https_host(&candidate.url) {
        Ok(host) if !is_official(&host) => reasons.push(Reason::UnknownDomain),
        Ok(_) => {}
        Err(reason) => reasons.push(reason),
    }
    let installed = device.driver.as_ref().and_then(|d| d.version);
    if installed.is_some_and(|v| candidate.version <= v) {
        reasons.push(Reason::NotNewer);
    }
    if let Some(ids) = &checks.inf_ids {
        let fits = ids.iter().any(|id| {
            device
                .hardware_ids
                .iter()
                .any(|own| own.eq_ignore_ascii_case(id.trim()))
        });
        if !fits {
            reasons.push(Reason::WrongHardware);
        }
    }
    if checks.hash_matches == Some(false) {
        reasons.push(Reason::HashMismatch);
    }
    if checks.signature_valid == Some(false) {
        reasons.push(Reason::BadSignature);
    }
    if !reasons.is_empty() {
        return Verdict::Rejected { reasons };
    }
    if candidate.source == Source::Suggested {
        return Verdict::OfficialPage;
    }
    let missing: Vec<&'static str> = [
        ("hardware id", checks.inf_ids.is_none()),
        ("hash", checks.hash_matches.is_none()),
        ("signature", checks.signature_valid.is_none()),
    ]
    .into_iter()
    .filter_map(|(name, absent)| absent.then_some(name))
    .collect();
    if missing.is_empty() {
        Verdict::Installable
    } else {
        Verdict::Pending { missing }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::device::InstalledDriver;

    const GPU_ID: &str = r"PCI\VEN_10DE&DEV_2182&SUBSYS_12345678";

    fn gpu(installed: Option<&str>) -> Device {
        Device {
            instance_id: r"PCI\VEN_10DE&DEV_2182&SUBSYS_12345678\4&1".into(),
            name: "NVIDIA GeForce GTX 1660 Ti".into(),
            class: "Display".into(),
            hardware_ids: vec![GPU_ID.into(), r"PCI\VEN_10DE&DEV_2182".into()],
            driver: Some(InstalledDriver {
                version: installed.and_then(DriverVersion::parse),
                ..InstalledDriver::default()
            }),
            ..Device::default()
        }
    }

    fn candidate(source: Source, url: &str, version: &str) -> Candidate {
        Candidate {
            device: gpu(None).instance_id,
            source,
            url: url.into(),
            version: DriverVersion::parse(version).unwrap(),
        }
    }

    fn all_checked() -> Checks {
        Checks {
            inf_ids: Some(vec![GPU_ID.to_lowercase()]),
            hash_matches: Some(true),
            signature_valid: Some(true),
        }
    }

    fn rejected(v: Verdict) -> Vec<Reason> {
        match v {
            Verdict::Rejected { reasons } => reasons,
            other => panic!("expected a rejection, got {other:?}"),
        }
    }

    #[test]
    fn a_vendor_driver_that_passes_every_check_is_installable() {
        let c = candidate(
            Source::Vendor,
            "https://www.nvidia.com/d/566.zip",
            "32.0.15.6614",
        );
        assert_eq!(
            judge(&gpu(Some("32.0.16.2124")), &c, &all_checked()),
            Verdict::Rejected {
                reasons: vec![Reason::NotNewer]
            }
        );
        let newer = candidate(
            Source::Vendor,
            "https://us.download.nvidia.com/x.exe",
            "32.0.16.9000",
        );
        assert_eq!(
            judge(&gpu(Some("32.0.16.2124")), &newer, &all_checked()),
            Verdict::Installable
        );
    }

    #[test]
    fn what_is_not_checked_yet_is_listed_and_nothing_is_installable_meanwhile() {
        let c = candidate(Source::Vendor, "https://www.nvidia.com/x", "99.0");
        assert_eq!(
            judge(&gpu(Some("1.0")), &c, &Checks::default()),
            Verdict::Pending {
                missing: vec!["hardware id", "hash", "signature"]
            }
        );
        let partly = Checks {
            hash_matches: Some(true),
            ..Checks::default()
        };
        assert_eq!(
            judge(&gpu(Some("1.0")), &c, &partly),
            Verdict::Pending {
                missing: vec!["hardware id", "signature"]
            }
        );
    }

    #[test]
    fn a_suggestion_from_the_web_never_gets_past_opening_the_official_page() {
        let c = candidate(
            Source::Suggested,
            "https://www.realtek.com/Download",
            "6.0.9",
        );
        assert_eq!(
            judge(&gpu(Some("1.0")), &c, &all_checked()),
            Verdict::OfficialPage
        );
        assert_eq!(
            judge(&gpu(Some("1.0")), &c, &Checks::default()),
            Verdict::OfficialPage
        );
    }

    #[test]
    fn look_alike_addresses_are_turned_down() {
        let d = gpu(Some("1.0"));
        for (url, why) in [
            ("http://www.nvidia.com/x", Reason::NotHttps),
            ("https://nvidia.com@evil.io/x", Reason::BadAddress),
            ("https://nvidia.com:8443/x", Reason::BadAddress),
            ("https://nvidia.com.evil.io/x", Reason::UnknownDomain),
            ("https://evilnvidia.com/x", Reason::UnknownDomain),
            (
                "https://nvidia.com.evil.io/x?u=nvidia.com",
                Reason::UnknownDomain,
            ),
            ("https://nvidiа.com/x", Reason::BadAddress), // Cyrillic а
            ("https://./x", Reason::BadAddress),
            ("https:///x", Reason::BadAddress),
            ("ftp://nvidia.com/x", Reason::NotHttps),
            ("javascript:alert(1)", Reason::NotHttps),
        ] {
            let c = candidate(Source::Suggested, url, "9.0");
            assert_eq!(
                rejected(judge(&d, &c, &Checks::default())),
                vec![why],
                "{url}"
            );
        }
    }

    #[test]
    fn a_domain_and_its_subdomains_are_official_and_case_does_not_matter() {
        let d = gpu(Some("1.0"));
        for url in [
            "https://NVIDIA.com",
            "https://www.nvidia.com/a",
            "https://us.download.nvidia.com/a#x",
            "https://support.hp.com?q=1",
        ] {
            let c = candidate(Source::Suggested, url, "9.0");
            assert_eq!(
                judge(&d, &c, &Checks::default()),
                Verdict::OfficialPage,
                "{url}"
            );
        }
    }

    #[test]
    fn an_older_or_equal_version_is_never_offered_but_an_unknown_installed_one_does_not_block() {
        let same = candidate(Source::Vendor, "https://www.nvidia.com/x", "5.0");
        assert_eq!(
            rejected(judge(&gpu(Some("5.0")), &same, &all_checked())),
            vec![Reason::NotNewer]
        );
        let mut unknown = gpu(None);
        unknown.driver = None;
        assert!(matches!(
            judge(&unknown, &same, &all_checked()),
            Verdict::Installable
        ));
    }

    #[test]
    fn a_package_whose_inf_lacks_the_hardware_id_is_rejected() {
        let c = candidate(Source::Vendor, "https://www.nvidia.com/x", "9.0");
        let other = Checks {
            inf_ids: Some(vec![r"PCI\VEN_10DE&DEV_9999".into()]),
            ..all_checked()
        };
        assert_eq!(
            rejected(judge(&gpu(Some("1.0")), &c, &other)),
            vec![Reason::WrongHardware]
        );
        let short = Checks {
            inf_ids: Some(vec![r"pci\ven_10de&dev_2182".into()]),
            ..all_checked()
        };
        assert_eq!(
            judge(&gpu(Some("1.0")), &c, &short),
            Verdict::Installable,
            "case and the less specific id both fit"
        );
    }

    #[test]
    fn a_failed_hash_or_signature_rejects_even_an_official_page_and_reasons_add_up() {
        let c = candidate(Source::Suggested, "https://www.nvidia.com/x", "9.0");
        let bad = Checks {
            hash_matches: Some(false),
            signature_valid: Some(false),
            inf_ids: None,
        };
        assert_eq!(
            rejected(judge(&gpu(Some("1.0")), &c, &bad)),
            vec![Reason::HashMismatch, Reason::BadSignature]
        );
        let worst = candidate(Source::Vendor, "https://evil.io/x", "1.0");
        assert_eq!(
            rejected(judge(&gpu(Some("1.0")), &worst, &bad)),
            vec![
                Reason::UnknownDomain,
                Reason::NotNewer,
                Reason::HashMismatch,
                Reason::BadSignature
            ]
        );
    }

    #[test]
    fn screen_network_disk_and_system_devices_are_sensitive() {
        assert_eq!(risk_of(&gpu(None)), Risk::Sensitive);
        for class in ["Net", "HDC", "SCSIAdapter", "System", "Processor", "net"] {
            let d = Device {
                class: class.into(),
                ..Device::default()
            };
            assert_eq!(risk_of(&d), Risk::Sensitive, "{class}");
        }
        for class in ["MEDIA", "Camera", "Bluetooth", "HIDClass", "Mouse"] {
            let d = Device {
                class: class.into(),
                ..Device::default()
            };
            assert_eq!(risk_of(&d), Risk::Normal, "{class}");
        }
    }
}
