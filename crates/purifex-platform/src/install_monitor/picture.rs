//! The picture of the system: the folders where apps put their files (two levels deep) and
//! the keys where they register (two levels below `Software`). Only names, sizes, times: no
//! contents are read, so it takes a moment.

use std::fs;
use std::os::windows::fs::MetadataExt;
use std::path::Path;
use std::time::UNIX_EPOCH;

use purifex_domain::app_traces::TraceKind;
use purifex_domain::monitor::Mark;
use purifex_domain::ports::SystemPicture;
use windows::Win32::System::Registry::{HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};

use crate::app_cleanup::roots;
use crate::registry::{Key, View};

const REPARSE_POINT: u32 = 0x400;
/// The most things in one picture: a bound for a very full disk.
const MAX_MARKS: usize = 400_000;
/// Keys of `Software` that belong to Windows or to the machine, not to an installer.
const SKIP_KEYS: [&str; 4] = ["microsoft", "classes", "policies", "wow6432node"];

pub struct WindowsPicture;

fn seconds(meta: &fs::Metadata) -> u64 {
    meta.modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_secs())
}

fn files_in(folder: &Path, depth: u8, marks: &mut Vec<Mark>) {
    let Ok(read) = fs::read_dir(folder) else {
        return;
    };
    for entry in read.flatten() {
        if marks.len() >= MAX_MARKS {
            return;
        }
        let Ok(meta) = entry.metadata() else {
            continue;
        };
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_lowercase();
        if meta.file_attributes() & REPARSE_POINT != 0 || name == "windowsapps" {
            continue;
        }
        let is_dir = meta.is_dir();
        marks.push(Mark {
            kind: if is_dir {
                TraceKind::Folder
            } else {
                TraceKind::File
            },
            location: path.to_string_lossy().into_owned(),
            value: None,
            stamp: format!("{}-{}", meta.len(), seconds(&meta)),
            bytes: if is_dir { 0 } else { meta.len() },
        });
        if is_dir && depth > 1 {
            files_in(&path, depth - 1, marks);
        }
    }
}

fn keys_in(hive: (&str, HKEY), base: &str, view: View, depth: u8, marks: &mut Vec<Mark>) {
    let Some(key) = Key::open(hive.1, base, view) else {
        return;
    };
    for name in key.subkeys() {
        if marks.len() >= MAX_MARKS {
            return;
        }
        let top = depth == 2 && SKIP_KEYS.contains(&name.to_lowercase().as_str());
        if top {
            continue;
        }
        let path = format!(r"{base}\{name}");
        let shown = match view {
            View::Bits32 if base.eq_ignore_ascii_case("SOFTWARE") => {
                format!(r"{}\SOFTWARE\WOW6432Node\{name}", hive.0)
            }
            _ => format!(r"{}\{path}", hive.0),
        };
        marks.push(Mark {
            kind: TraceKind::RegistryKey,
            location: shown,
            value: None,
            stamp: String::new(),
            bytes: 0,
        });
        if depth > 1 {
            keys_in(hive, &path, view, depth - 1, marks);
        }
    }
}

impl SystemPicture for WindowsPicture {
    fn measure(&self, marks: &mut [Mark]) {
        for mark in marks.iter_mut().filter(|m| m.kind == TraceKind::Folder) {
            mark.bytes = crate::app_cleanup::folder_size(Path::new(&mark.location));
        }
    }

    fn take(&self) -> Result<Vec<Mark>, String> {
        let mut marks = Vec::new();
        for root in roots() {
            files_in(&root, 2, &mut marks);
        }
        keys_in(
            ("HKCU", HKEY_CURRENT_USER),
            "Software",
            View::Bits64,
            2,
            &mut marks,
        );
        keys_in(
            ("HKLM", HKEY_LOCAL_MACHINE),
            "SOFTWARE",
            View::Bits64,
            2,
            &mut marks,
        );
        keys_in(
            ("HKLM", HKEY_LOCAL_MACHINE),
            "SOFTWARE",
            View::Bits32,
            2,
            &mut marks,
        );
        if marks.is_empty() {
            return Err("no se pudo leer ninguna ubicación del sistema".into());
        }
        Ok(marks)
    }
}

#[cfg(test)]
mod tests {
    use purifex_domain::monitor::added;

    use super::*;
    use crate::app_cleanup::registry_write::{delete_key_tree, set_value_raw, RawValue};

    /// A real folder and a real key made between two pictures are what the difference holds,
    /// and what was already there is not.
    #[test]
    fn a_folder_and_a_key_made_between_two_pictures_are_the_difference() {
        let name = format!("PurifexMonitorTest{}", std::process::id());
        let local = std::env::var_os("LOCALAPPDATA")
            .map(std::path::PathBuf::from)
            .unwrap();
        let folder = local.join(&name);
        let key = format!(r"HKCU\Software\{name}");
        let before = WindowsPicture.take().unwrap();
        fs::create_dir_all(folder.join("data")).unwrap();
        fs::write(folder.join("data").join("a.bin"), b"abc").unwrap();
        let data = "x\0".encode_utf16().flat_map(u16::to_le_bytes).collect();
        set_value_raw(&format!(r"{key}\Sub"), "v", &RawValue { kind: 1, data }).unwrap();
        let after = WindowsPicture.take().unwrap();
        let mut new = added(&before, &after);
        let _ = fs::remove_dir_all(&folder);
        let _ = delete_key_tree(&key);
        WindowsPicture.measure(&mut new);
        let places: Vec<&str> = new.iter().map(|m| m.location.as_str()).collect();
        let folder_text = folder.to_string_lossy().to_lowercase();
        assert!(
            places.iter().any(|p| p.to_lowercase() == folder_text),
            "{places:?}"
        );
        assert!(
            places.iter().any(|p| p
                .to_lowercase()
                .ends_with(&format!(r"{}\sub", name.to_lowercase()))),
            "{places:?}"
        );
        assert!(new.len() < 20, "only what is new: {places:?}");
    }
}
