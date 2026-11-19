//! The classic apps installed in the computer: what each one registered in
//! `Uninstall` (the list that Windows' own "Installed apps" shows). Read only.

use purifex_domain::apps::{AppKind, InstalledApp};
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
                    kind: AppKind::Desktop,
                    hidden: is_hidden(&key, &sub),
                });
            }
        }
        Ok(apps)
    }
}
