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
use super::find_firewall::find_firewall_rules;
use super::find_registry::{
    discover_extensions, exes_in_values, find_associations, find_program_keys, find_values, Hives,
};
use super::find_services::find_services;
use super::find_shell::find_shell;
use super::tasks::find_tasks;
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

/// The install folder the app registered, as a candidate (if it is still there).
fn registered_folder(app: &InstalledApp) -> Vec<Candidate> {
    app.install_location
        .as_deref()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .and_then(|location| candidate_at(Path::new(location), true))
        .into_iter()
        .collect()
}

/// The seeds to look with, and the values that point into the app's folder. Besides
/// the app's own seeds: the extensions it registered in `Classes` (even if they were
/// not seen before it was uninstalled), and the programs seen in those values
/// (`...\bin\kdenlive.exe`), whose crash dumps are looked for even when the app was
/// uninstalled elsewhere.
fn observed_seeds(app: &InstalledApp, hives: &Hives) -> (Seeds, Vec<Candidate>) {
    let mut seeds = seeds_of(app);
    seeds.extensions.extend(discover_extensions(hives, &seeds));
    let values = find_values(hives, &seeds);
    let mut exes = seeds.exes.clone();
    exes.extend(exes_in_values(&values, seeds.folder.as_deref()));
    let seeds = Seeds::new(seeds.folder.as_deref(), &exes, &seeds.extensions);
    (seeds, values)
}

/// Folders, files and shortcuts in the usual places named exactly like the app.
fn places_named(names: &[String]) -> Vec<Candidate> {
    if names.is_empty() {
        return Vec::new();
    }
    roots()
        .iter()
        .filter_map(|root| fs::read_dir(root).ok())
        .flat_map(|read| read.flatten().take(10_000))
        .filter(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            let stem = name.strip_suffix(".lnk").unwrap_or(&name).to_string();
            names.contains(&simple(&stem))
        })
        .filter_map(|entry| candidate_at(&entry.path(), false))
        .collect()
}

/// Registry keys named exactly like the app, under `Software` of each hive.
fn keys_named(names: &[String]) -> Vec<Candidate> {
    if names.is_empty() {
        return Vec::new();
    }
    registry_roots()
        .into_iter()
        .filter_map(|(label, root, path)| {
            Key::open(root, path, View::Bits64).map(|parent| (label, path, parent))
        })
        .flat_map(|(label, path, parent)| {
            parent
                .subkeys()
                .into_iter()
                .filter(|sub| names.contains(&simple(sub)))
                .map(move |sub| Candidate {
                    kind: TraceKind::RegistryKey,
                    location: format!(r"{label}\{path}\{sub}"),
                    ..Default::default()
                })
        })
        .collect()
}

/// What Windows runs or allows on its own (services, tasks, firewall, COM and the context
/// menu). Each step waits on something slow (the registry by thousands, a process), and none
/// needs another: they run side by side.
fn system_places(seeds: &Seeds) -> Vec<Candidate> {
    let steps: [fn(&Seeds) -> Vec<Candidate>; 4] =
        [find_services, find_tasks, find_firewall_rules, find_shell];
    std::thread::scope(|scope| {
        let running: Vec<_> = steps
            .iter()
            .map(|step| scope.spawn(|| step(seeds)))
            .collect();
        running
            .into_iter()
            .flat_map(|step| step.join().unwrap_or_default())
            .collect()
    })
}

impl TraceFinder for WindowsTraceFinder {
    fn candidates(&self, app: &InstalledApp) -> Result<Vec<Candidate>, String> {
        self.search(app).map(|f| f.candidates)
    }

    fn search(&self, app: &InstalledApp) -> Result<Found, String> {
        let names = names_of(app);
        let hives = Hives::real();
        let (seeds, values) = observed_seeds(app, &hives);
        let (files, complete) = find_files(&UserPlaces::real(), &names, &seeds, FILES_BUDGET);
        if !complete {
            purifex_domain::log_warn!(
                "the search for shortcuts of {} ran out of time: the list may be incomplete",
                app.name
            );
        }
        let candidates = [
            registered_folder(app),
            files,
            values,
            find_associations(&hives, &seeds),
            find_program_keys(&hives, &names, &seeds),
            places_named(&names),
            keys_named(&names),
            system_places(&seeds),
        ]
        .concat();
        Ok(Found {
            seeds,
            candidates,
            incomplete: !complete,
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

    /// Runs `step`, prints how long it took and how many things it found.
    fn timed<T>(label: &str, step: impl FnOnce() -> (T, usize)) -> T {
        let started = Instant::now();
        let (value, found) = step();
        let ms = started.elapsed().as_secs_f64() * 1e3;
        println!("{label:<18} {ms:>6.0} ms  {found}");
        value
    }

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
        let mut seeds = timed("seeds", || (seeds_of(&app), 0));
        seeds.extensions = timed("discover (Classes)", || {
            let found = discover_extensions(&hives, &seeds);
            let n = found.len();
            (found, n)
        });
        let places = UserPlaces::real();
        timed("files", || {
            let n = find_files(&places, &names, &seeds, FILES_BUDGET).0.len();
            ((), n)
        });
        timed("values", || ((), find_values(&hives, &seeds).len()));
        timed("associations", || {
            ((), find_associations(&hives, &seeds).len())
        });
        timed("program keys", || {
            ((), find_program_keys(&hives, &names, &seeds).len())
        });
        timed("services", || ((), find_services(&seeds).len()));
        timed("tasks", || ((), find_tasks(&seeds).len()));
        timed("firewall", || ((), find_firewall_rules(&seeds).len()));
        timed("com + menu", || ((), find_shell(&seeds).len()));
        timed("WHOLE SEARCH", || {
            ((), WindowsTraceFinder.candidates(&app).unwrap().len())
        });
    }
}
