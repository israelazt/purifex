//! Looks for places that might hold the traces of an app: the folder it registered,
//! folders and shortcuts in the usual places named like the app, and keys of the
//! registry named like it. It only reads; the rules of the domain decide what is
//! really the app's.

use std::fs;
use std::os::windows::fs::MetadataExt;
use std::path::{Path, PathBuf};

use purifex_domain::app_traces::{names_of, simple, Candidate, TraceKind};
use purifex_domain::apps::InstalledApp;
use purifex_domain::ports::TraceFinder;
use purifex_domain::seeds::seeds_of;
use windows::Win32::System::Registry::{HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};

use super::find_files::{find_files, UserPlaces};
use crate::registry::{Key, View};

const REPARSE_POINT: u32 = 0x400;
/// The most entries looked at when adding up the size of a folder.
const MAX_ENTRIES: usize = 200_000;

pub struct WindowsTraceFinder;

fn env_path(name: &str) -> Option<PathBuf> {
    std::env::var_os(name).map(PathBuf::from)
}

/// The folders whose direct children are looked at.
fn roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    let appdata = env_path("APPDATA");
    let local = env_path("LOCALAPPDATA");
    let programdata = env_path("PROGRAMDATA");
    for base in [&appdata, &local, &programdata].into_iter().flatten() {
        roots.push(base.clone());
    }
    if let Some(local) = &local {
        roots.push(local.join("Programs"));
    }
    for name in ["ProgramFiles", "ProgramFiles(x86)"] {
        roots.extend(env_path(name));
    }
    if let Some(profile) = env_path("USERPROFILE") {
        roots.push(profile.join("Documents"));
    }
    if let Some(appdata) = &appdata {
        roots.push(appdata.join(r"Microsoft\Windows\Start Menu\Programs"));
    }
    if let Some(programdata) = &programdata {
        roots.push(programdata.join(r"Microsoft\Windows\Start Menu\Programs"));
    }
    roots
}

/// The size of a folder, not following links, up to a limit of entries.
pub fn folder_size(path: &Path) -> u64 {
    let (mut total, mut seen) = (0u64, 0usize);
    let mut pending = vec![path.to_path_buf()];
    while let Some(folder) = pending.pop() {
        let Ok(read) = fs::read_dir(&folder) else {
            continue;
        };
        for entry in read.flatten() {
            seen += 1;
            if seen > MAX_ENTRIES {
                return total;
            }
            let Ok(meta) = entry.metadata() else { continue };
            if meta.file_attributes() & REPARSE_POINT != 0 {
                continue;
            }
            if meta.is_dir() {
                pending.push(entry.path());
            } else {
                total += meta.len();
            }
        }
    }
    total
}

pub(super) fn candidate_at(path: &Path, registered: bool) -> Option<Candidate> {
    let meta = fs::symlink_metadata(path).ok()?;
    if meta.file_attributes() & REPARSE_POINT != 0 {
        return None; // a link or junction: what it points to is not ours to move
    }
    let (kind, bytes) = if meta.is_dir() {
        (TraceKind::Folder, folder_size(path))
    } else {
        (TraceKind::File, meta.len())
    };
    Some(Candidate {
        kind,
        location: path.to_string_lossy().into_owned(),
        bytes,
        registered_install: registered,
        value: None,
        text: None,
    })
}

fn registry_roots() -> [(&'static str, HKEY, &'static str); 3] {
    [
        ("HKCU", HKEY_CURRENT_USER, "Software"),
        ("HKLM", HKEY_LOCAL_MACHINE, "SOFTWARE"),
        ("HKLM", HKEY_LOCAL_MACHINE, r"SOFTWARE\WOW6432Node"),
    ]
}

impl TraceFinder for WindowsTraceFinder {
    fn candidates(&self, app: &InstalledApp) -> Result<Vec<Candidate>, String> {
        let names = names_of(app);
        let mut found = Vec::new();
        if let Some(location) = app
            .install_location
            .as_deref()
            .map(str::trim)
            .filter(|l| !l.is_empty())
        {
            found.extend(candidate_at(Path::new(location), true));
        }
        // What is recognised by reference to the app's folder or programs needs no name.
        found.extend(find_files(&UserPlaces::real(), &names, &seeds_of(app)));
        if names.is_empty() {
            return Ok(found);
        }
        for root in roots() {
            let Ok(read) = fs::read_dir(&root) else {
                continue;
            };
            for entry in read.flatten().take(10_000) {
                let name = entry.file_name().to_string_lossy().into_owned();
                let stem = name.strip_suffix(".lnk").unwrap_or(&name);
                if names.contains(&simple(stem)) {
                    found.extend(candidate_at(&entry.path(), false));
                }
            }
        }
        for (label, root, path) in registry_roots() {
            let Some(parent) = Key::open(root, path, View::Bits64) else {
                continue;
            };
            for sub in parent.subkeys() {
                if names.contains(&simple(&sub)) {
                    found.push(Candidate {
                        kind: TraceKind::RegistryKey,
                        location: format!(r"{label}\{path}\{sub}"),
                        bytes: 0,
                        registered_install: false,
                        value: None,
                        text: None,
                    });
                }
            }
        }
        Ok(found)
    }
}
