//! Reading what starts with Windows. Only reads.

use std::fs;
use std::path::PathBuf;

use purifex_domain::startup::{StartupItem, StartupKind};
use windows::Win32::System::Registry::{HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};

use crate::app_cleanup::registry_write::read_value_raw;
use crate::app_cleanup::{expand, lnk_target, startup_tasks};
use crate::registry::{Key, View};

const CURRENT: &str = r"Microsoft\Windows\CurrentVersion";

/// One `Run` key: its hive, its path, whether Windows keeps an on/off switch for it.
struct RunKey {
    label: &'static str,
    root: HKEY,
    path: String,
    toggle: bool,
}

fn run_keys() -> Vec<RunKey> {
    let key = |label, root, base: &str, leaf: &str, toggle| RunKey {
        label,
        root,
        path: format!(r"{base}\{CURRENT}\{leaf}"),
        toggle,
    };
    vec![
        key("HKCU", HKEY_CURRENT_USER, "Software", "Run", true),
        key("HKCU", HKEY_CURRENT_USER, "Software", "RunOnce", false),
        key("HKLM", HKEY_LOCAL_MACHINE, "SOFTWARE", "Run", true),
        key("HKLM", HKEY_LOCAL_MACHINE, "SOFTWARE", "RunOnce", false),
        key(
            "HKLM",
            HKEY_LOCAL_MACHINE,
            r"SOFTWARE\WOW6432Node",
            "Run",
            true,
        ),
    ]
}

/// Whether Windows has this entry turned on: 2 (or 6) is on, 3 is off, nothing is on.
fn approved(key: &str, name: &str) -> bool {
    read_value_raw(key, name)
        .ok()
        .and_then(|v| v.data.first().copied())
        .is_none_or(|first| first & 1 == 0)
}

fn registry_items() -> Vec<StartupItem> {
    let mut items = Vec::new();
    for run in run_keys() {
        let Some(key) = Key::open(run.root, &run.path, View::Bits64) else {
            continue;
        };
        let place = format!(r"{}\{}", run.label, run.path);
        let leaf = if run.path.contains("WOW6432Node") {
            "Run32"
        } else {
            "Run"
        };
        let switch = format!(
            r"{}\Software\{CURRENT}\Explorer\StartupApproved\{leaf}",
            run.label
        );
        for (name, text) in key.values() {
            let command = expand(&text.unwrap_or_default());
            items.push(StartupItem {
                id: format!("registro|{place}|{name}"),
                enabled: !run.toggle || approved(&switch, &name),
                can_toggle: run.toggle,
                needs_admin: run.label == "HKLM",
                name,
                command,
                kind: StartupKind::Registry,
                place: place.clone(),
                app: None,
            });
        }
    }
    items
}

fn startup_folders() -> Vec<(PathBuf, &'static str)> {
    let tail = r"Microsoft\Windows\Start Menu\Programs\Startup";
    let from = |var: &str, label| {
        std::env::var_os(var).map(|base| (PathBuf::from(base).join(tail), label))
    };
    [from("APPDATA", "HKCU"), from("PROGRAMDATA", "HKLM")]
        .into_iter()
        .flatten()
        .collect()
}

fn folder_items() -> Vec<StartupItem> {
    let mut items = Vec::new();
    for (folder, hive) in startup_folders() {
        let Ok(read) = fs::read_dir(&folder) else {
            continue;
        };
        let switch = format!(r"{hive}\Software\{CURRENT}\Explorer\StartupApproved\StartupFolder");
        for entry in read.flatten() {
            let file = entry.file_name().to_string_lossy().into_owned();
            if file.eq_ignore_ascii_case("desktop.ini") || !entry.path().is_file() {
                continue;
            }
            let path = entry.path();
            let command = if file.to_lowercase().ends_with(".lnk") {
                fs::read(&path).ok().and_then(|b| lnk_target(&b))
            } else {
                None
            }
            .unwrap_or_else(|| path.to_string_lossy().into_owned());
            items.push(StartupItem {
                id: format!("carpeta|{}", path.display()),
                name: file
                    .rsplit_once('.')
                    .map_or(file.clone(), |(stem, _)| stem.to_string()),
                command,
                kind: StartupKind::Folder,
                place: path.to_string_lossy().into_owned(),
                enabled: approved(&switch, &file),
                can_toggle: true,
                needs_admin: hive == "HKLM",
                app: None,
            });
        }
    }
    items
}

/// The services that start by themselves and are not Windows': listed so the person knows
/// they run, never changed from here (a wrong change can stop the PC from starting).
fn service_items() -> Vec<StartupItem> {
    let path = r"SYSTEM\CurrentControlSet\Services";
    let Some(parent) = Key::open(HKEY_LOCAL_MACHINE, path, View::Bits64) else {
        return Vec::new();
    };
    parent
        .subkeys()
        .into_iter()
        .filter_map(|name| {
            let key = Key::open(HKEY_LOCAL_MACHINE, &format!(r"{path}\{name}"), View::Bits64)?;
            let (start, kind) = (key.number("Start")?, key.number("Type")?);
            let image = expand(&key.text("ImagePath")?);
            let windows = image.to_lowercase().replace('"', "");
            let own = kind & 0x30 != 0;
            if start != 2
                || !own
                || windows.starts_with(r"c:\windows\")
                || windows.starts_with(r"\systemroot")
            {
                return None;
            }
            let shown = key.text("DisplayName").filter(|d| !d.starts_with('@'));
            Some(StartupItem {
                id: format!("servicio|{name}"),
                name: shown.unwrap_or_else(|| name.clone()),
                command: image,
                kind: StartupKind::Service,
                place: format!(r"HKLM\{path}\{name}"),
                enabled: true,
                can_toggle: false,
                needs_admin: true,
                app: None,
            })
        })
        .collect()
}

fn task_items() -> Vec<StartupItem> {
    startup_tasks()
        .into_iter()
        .map(|(path, command, enabled)| StartupItem {
            id: format!("tarea|{path}"),
            name: path.rsplit('\\').next().unwrap_or(&path).to_string(),
            command,
            kind: StartupKind::Task,
            place: path,
            enabled,
            can_toggle: true,
            needs_admin: false,
            app: None,
        })
        .collect()
}

pub(super) fn all() -> Vec<StartupItem> {
    let mut items = registry_items();
    items.extend(folder_items());
    items.extend(service_items());
    items.extend(task_items());
    items
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Reads the real machine: whatever is there, every entry is well formed and unique.
    #[test]
    fn the_real_list_is_well_formed() {
        let items = all();
        let mut ids: Vec<&str> = items.iter().map(|i| i.id.as_str()).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), items.len(), "ids are unique");
        for item in &items {
            assert!(!item.name.is_empty() && !item.place.is_empty(), "{item:?}");
            assert!(item.kind != StartupKind::Service || !item.can_toggle);
        }
    }
}
