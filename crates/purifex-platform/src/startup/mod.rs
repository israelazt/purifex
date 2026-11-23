//! What starts with Windows, on Windows: reading `Run`, the Startup folders, the services and
//! the logon tasks, and turning an entry on or off the way Windows itself does
//! (`StartupApproved` for the first two, `schtasks` for tasks). Nothing is deleted here.

mod read;

use purifex_domain::ports::StartupSource;
use purifex_domain::startup::{StartupItem, StartupKind};

use crate::app_cleanup::registry_write::{set_value_raw, RawValue};
use crate::app_cleanup::set_task_enabled;

pub struct WindowsStartup;

const APPROVED: &str = r"Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved";
/// Seconds between 1601 (what a FILETIME counts from) and 1970.
const EPOCH_GAP: u64 = 11_644_473_600;

/// The `StartupApproved` key that says whether this entry runs, and the name of its value.
fn approved_for(item: &StartupItem) -> Option<(String, String)> {
    let place = item.place.to_lowercase();
    let hive = if place.starts_with("hkcu") {
        "HKCU"
    } else {
        "HKLM"
    };
    let leaf = match item.kind {
        StartupKind::Registry if place.ends_with(r"\run") && place.contains("wow6432node") => {
            "Run32"
        }
        StartupKind::Registry if place.ends_with(r"\run") => "Run",
        StartupKind::Folder => "StartupFolder",
        _ => return None,
    };
    let name = match item.kind {
        StartupKind::Folder => item.place.rsplit('\\').next()?.to_string(),
        _ => item.name.clone(),
    };
    Some((format!(r"{hive}\{APPROVED}\{leaf}"), name))
}

/// What Windows writes: 2 means it runs, 3 means it was turned off (and when).
fn approval(on: bool) -> RawValue {
    let mut data = vec![0u8; 12];
    if on {
        data[0] = 2;
    } else {
        data[0] = 3;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        let ticks = (now + EPOCH_GAP) * 10_000_000;
        data[4..12].copy_from_slice(&ticks.to_le_bytes());
    }
    RawValue { kind: 3, data }
}

impl StartupSource for WindowsStartup {
    fn read(&self) -> Result<Vec<StartupItem>, String> {
        Ok(read::all(false))
    }

    fn read_quick(&self) -> Result<Vec<StartupItem>, String> {
        Ok(read::all(true))
    }

    fn set_enabled(&self, item: &StartupItem, on: bool) -> Result<(), String> {
        if !item.can_toggle {
            return Err("esta entrada no se puede activar ni desactivar desde aquí".into());
        }
        if item.kind == StartupKind::Task {
            return set_task_enabled(&item.place, on).map_err(|e| {
                format!("{e} (si es una tarea del sistema hace falta ser administrador)")
            });
        }
        let (key, name) = approved_for(item).ok_or("esta entrada no se puede cambiar")?;
        set_value_raw(&key, &name, &approval(on)).map_err(|e| {
            if item.needs_admin {
                format!(
                    "{} (es de todo el equipo: usa «Reiniciar como administrador»)",
                    e.message()
                )
            } else {
                e.message()
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn registry_item(place: &str) -> StartupItem {
        StartupItem {
            id: "x".into(),
            name: "Zed".into(),
            command: "z.exe".into(),
            kind: StartupKind::Registry,
            place: place.into(),
            enabled: true,
            can_toggle: true,
            needs_admin: false,
            app: None,
        }
    }

    #[test]
    fn each_place_has_its_own_approval_key_and_run_once_has_none() {
        let run = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";
        let (key, name) = approved_for(&registry_item(run)).unwrap();
        assert!(key.ends_with(r"StartupApproved\Run") && key.starts_with("HKCU") && name == "Zed");
        let wow = r"HKLM\SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Run";
        assert!(approved_for(&registry_item(wow))
            .unwrap()
            .0
            .ends_with("Run32"));
        assert!(approved_for(&registry_item(&format!("{run}Once"))).is_none());
        let folder = StartupItem {
            kind: StartupKind::Folder,
            place: r"C:\Users\u\Start Menu\Startup\z.lnk".into(),
            ..registry_item("")
        };
        let (key, name) = approved_for(&folder).unwrap();
        assert!(key.ends_with("StartupFolder") && name == "z.lnk");
    }

    #[test]
    fn the_approval_bytes_are_what_windows_writes() {
        assert_eq!(approval(true).data, [2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        let off = approval(false).data;
        assert_eq!(off[0], 3);
        assert!(
            off[4..].iter().any(|b| *b != 0),
            "the time it was turned off"
        );
    }
}

/// The whole way of an entry on this computer: made for the test in the user's own `Run` key,
/// seen by the reader, turned off and on the way Windows does, taken out to a quarantine and put
/// back. Everything is removed at the end, even when an assertion fails halfway.
#[cfg(test)]
mod real {
    use purifex_domain::cleanup_log::{CleanupEntry, CleanupState};
    use purifex_domain::ports::CleanupStore;

    use super::*;
    use crate::app_cleanup::registry_write::{delete_value, read_value_raw};
    use crate::app_cleanup::LocalCleanup;

    const RUN: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";
    const SWITCH: &str =
        r"HKCU\Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run";

    struct Entry(String);

    impl Drop for Entry {
        fn drop(&mut self) {
            let _ = delete_value(RUN, &self.0);
            let _ = delete_value(SWITCH, &self.0);
        }
    }

    fn find(name: &str) -> Option<StartupItem> {
        WindowsStartup
            .read()
            .unwrap()
            .into_iter()
            .find(|i| i.name == name && i.kind == StartupKind::Registry)
    }

    #[test]
    fn an_entry_is_seen_turned_off_and_on_removed_and_put_back_for_real() {
        let name = format!("PurifexStartupE2E{}", std::process::id());
        let entry = Entry(name.clone());
        let command = r"C:\Nada\purifex-e2e.exe /x"
            .encode_utf16()
            .chain([0])
            .flat_map(u16::to_le_bytes)
            .collect();
        set_value_raw(
            RUN,
            &name,
            &RawValue {
                kind: 1,
                data: command,
            },
        )
        .unwrap();
        let item = find(&name).expect("the reader lists it");
        assert!(item.enabled && item.can_toggle && !item.needs_admin);
        assert_eq!(item.program(), r"C:\Nada\purifex-e2e.exe");

        WindowsStartup.set_enabled(&item, false).unwrap();
        assert!(!find(&name).unwrap().enabled, "Windows records it as off");
        assert_eq!(read_value_raw(SWITCH, &name).unwrap().data[0], 3);
        WindowsStartup.set_enabled(&item, true).unwrap();
        assert!(find(&name).unwrap().enabled);

        let work = std::env::temp_dir().join(format!("purifex-startup-e2e-{}", std::process::id()));
        let mut store = LocalCleanup::in_dir(work.join("Purifex"));
        let trace = item.trace().unwrap();
        let kept = store.remove(&trace, 1).unwrap();
        assert!(find(&name).is_none(), "taken out of Run");
        let record = CleanupEntry {
            seq: 1,
            app: format!("Arranque: {name}"),
            trace,
            kept,
            state: CleanupState::Removed,
            ..Default::default()
        };
        store.restore(&record).unwrap();
        let back = find(&name).expect("put back");
        assert_eq!(back.command, item.command, "same command as before");
        drop(entry);
        let _ = std::fs::remove_dir_all(work);
    }
}
