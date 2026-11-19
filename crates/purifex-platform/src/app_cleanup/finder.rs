//! Looks for places that might hold the traces of an app: the folder it registered,
//! folders and shortcuts in the usual places named like the app, and keys of the
//! registry named like it. It only reads; the rules of the domain decide what is
//! really the app's.

use std::fs;
use std::os::windows::fs::MetadataExt;
use std::path::{Path, PathBuf};

use purifex_domain::app_traces::{names_of, simple, Candidate, TraceKind};
use purifex_domain::apps::InstalledApp;
use purifex_domain::ports::{Found, TraceFinder};
use purifex_domain::seeds::{seeds_of, Seeds};
use windows::Win32::System::Registry::{HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};

use super::find_files::{find_files, UserPlaces};
use super::find_registry::{
    discover_extensions, exes_in_values, find_associations, find_program_keys, find_values, Hives,
};
use crate::registry::{Key, View};

const REPARSE_POINT: u32 = 0x400;
/// How long reading shortcut targets may take before the search goes on without them.
const FILES_BUDGET: std::time::Duration = std::time::Duration::from_millis(1_500);
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
        roots.push(local.join("stalefiles"));
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
        ..Default::default()
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
        self.search(app).map(|f| f.candidates)
    }

    fn search(&self, app: &InstalledApp) -> Result<Found, String> {
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
        let hives = Hives::real();
        let mut seeds = seeds_of(app);
        seeds.extensions.extend(discover_extensions(&hives, &seeds));
        let values = find_values(&hives, &seeds);
        // Programs seen in what points into the folder (`...in\kdenlive.exe`) are the
        // programs whose crash dumps to look for, even for an app uninstalled elsewhere.
        let mut exes = seeds.exes.clone();
        exes.extend(exes_in_values(&values, seeds.folder.as_deref()));
        let seeds = Seeds::new(seeds.folder.as_deref(), &exes, &seeds.extensions);
        let (files, complete) = find_files(&UserPlaces::real(), &names, &seeds, FILES_BUDGET);
        if !complete {
            purifex_domain::log_warn!(
                "the search for shortcuts of {} ran out of time: the list may be incomplete",
                app.name
            );
        }
        found.extend(files);
        found.extend(values);
        found.extend(find_associations(&hives, &seeds));
        found.extend(find_program_keys(&hives, &names, &seeds));
        if names.is_empty() {
            return Ok(Found {
                seeds,
                candidates: found,
            });
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
                        ..Default::default()
                    });
                }
            }
        }
        Ok(Found {
            seeds,
            candidates: found,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use purifex_domain::apps::InstalledApp;

    use super::super::find_files::{find_files, UserPlaces};
    use super::super::find_registry::{
        discover_extensions, find_associations, find_program_keys, find_values, Hives,
    };
    use super::*;

    /// Not a test: `cargo test -p purifex-platform measure_the_real_search -- --ignored
    /// --nocapture` prints how long each part of the search takes on this computer.
    #[test]
    #[ignore]
    fn measure_the_real_search() {
        let app = InstalledApp {
            id: "x".into(),
            name: "Kdenlive".into(),
            install_location: Some(r"C:\Program Files\Kdenlive".into()),
            ..Default::default()
        };
        let names = names_of(&app);
        let hives = Hives::real();
        let t = Instant::now();
        let mut seeds = seeds_of(&app);
        println!(
            "seeds            {:>6.0} ms",
            t.elapsed().as_secs_f64() * 1e3
        );
        let t = Instant::now();
        seeds.extensions = discover_extensions(&hives, &seeds);
        println!(
            "discover (Classes) {:>4.0} ms  {} found",
            t.elapsed().as_secs_f64() * 1e3,
            seeds.extensions.len()
        );
        let t = Instant::now();
        let n = find_files(&UserPlaces::real(), &names, &seeds, FILES_BUDGET)
            .0
            .len();
        println!(
            "files            {:>6.0} ms  {n}",
            t.elapsed().as_secs_f64() * 1e3
        );
        let t = Instant::now();
        let n = find_values(&hives, &seeds).len();
        println!(
            "values           {:>6.0} ms  {n}",
            t.elapsed().as_secs_f64() * 1e3
        );
        let t = Instant::now();
        let n = find_associations(&hives, &seeds).len();
        println!(
            "associations     {:>6.0} ms  {n}",
            t.elapsed().as_secs_f64() * 1e3
        );
        let t = Instant::now();
        let n = find_program_keys(&hives, &names, &seeds).len();
        println!(
            "program keys     {:>6.0} ms  {n}",
            t.elapsed().as_secs_f64() * 1e3
        );
        let t = Instant::now();
        let n = WindowsTraceFinder.candidates(&app).unwrap().len();
        println!(
            "WHOLE SEARCH     {:>6.0} ms  {n}",
            t.elapsed().as_secs_f64() * 1e3
        );
    }
}
