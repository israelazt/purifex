//! The classic apps installed in the computer: what each one registered in
//! `Uninstall` (the list that Windows' own "Installed apps" shows). Read only.

use purifex_domain::apps::InstalledApp;
use purifex_domain::date::Date;
use purifex_domain::ports::AppInventory;
use windows::Win32::System::Registry::{HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};

use crate::registry::{Key, View};

const UNINSTALL: &str = r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall";

pub struct RegistryApps;

/// Where the apps are registered: machine (64 and 32-bit views) and current user.
const PLACES: [(&str, HKEY, View); 3] = [
    ("HKLM64", HKEY_LOCAL_MACHINE, View::Bits64),
    ("HKLM32", HKEY_LOCAL_MACHINE, View::Bits32),
    ("HKCU", HKEY_CURRENT_USER, View::Bits64),
];

/// Updates and parts of other programs are not apps of their own.
fn is_hidden(key: &Key, sub: &str) -> bool {
    key.number("SystemComponent") == Some(1)
        || key.text("ParentKeyName").is_some()
        || key.text("ParentDisplayName").is_some()
        || key.text("ReleaseType").is_some_and(|r| {
            ["update", "hotfix", "security update", "service pack"]
                .contains(&r.to_lowercase().as_str())
        })
        || sub.to_uppercase().starts_with("KB")
}

/// `InstallDate` is text (`20261118`) in most apps and a number in a few.
fn install_date(key: &Key) -> Option<Date> {
    key.text("InstallDate")
        .or_else(|| key.number("InstallDate").map(|n| n.to_string()))
        .and_then(|text| Date::parse_compact(&text))
}

/// The page of the app: only a web address is kept (some apps write a path or an e-mail there).
fn website(key: &Key) -> Option<String> {
    ["URLInfoAbout", "HelpLink"]
        .into_iter()
        .filter_map(|name| key.text(name))
        .map(|url| url.trim().to_string())
        .find(|url| {
            let lower = url.to_lowercase();
            lower.starts_with("https://") || lower.starts_with("http://")
        })
}

impl AppInventory for RegistryApps {
    fn apps(&self) -> Result<Vec<InstalledApp>, String> {
        let mut apps = Vec::new();
        for (label, root, view) in PLACES {
            let Some(parent) = Key::open(root, UNINSTALL, view) else {
                continue;
            };
            for sub in parent.subkeys() {
                let Some(key) = Key::open(root, &format!(r"{UNINSTALL}\{sub}"), view) else {
                    continue;
                };
                let Some(name) = key.text("DisplayName") else {
                    continue;
                };
                apps.push(InstalledApp {
                    id: format!("{label}:{sub}"),
                    name,
                    publisher: key.text("Publisher"),
                    version: key.text("DisplayVersion"),
                    install_location: key.text("InstallLocation"),
                    uninstall: key.text("UninstallString"),
                    quiet_uninstall: key.text("QuietUninstallString"),
                    size_kb: key.number("EstimatedSize").map(u64::from),
                    hidden: is_hidden(&key, &sub),
                    display_icon: key.text("DisplayIcon"),
                    install_date: install_date(&key),
                    website: website(&key),
                    ..Default::default()
                });
            }
        }
        Ok(apps)
    }
}
