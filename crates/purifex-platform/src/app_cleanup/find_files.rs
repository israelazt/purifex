//! The files that an app leaves in the user's folders and that are not named like a
//! folder of the app: settings with its name plus a suffix, crash dumps of its
//! programs, recent-file shortcuts of its extensions, and shortcuts (desktop, start
//! menu) whose target is inside its folder. It only reads and proposes; the rules of
//! the domain decide.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use purifex_domain::app_traces::{simple, Candidate};
use purifex_domain::reference::path_refers_to;
use purifex_domain::seeds::Seeds;

use super::finder::candidate_at;
use super::lnk;

/// The most entries read from one folder.
const MAX_PER_FOLDER: usize = 10_000;
/// The most shortcuts whose target is read in one folder.
const MAX_SHORTCUTS: usize = 5_000;

/// The user's folders that are looked at; replaceable to test against a temporary tree.
pub struct UserPlaces {
    pub local: Option<PathBuf>,
    pub roaming: Option<PathBuf>,
    /// Folders with shortcuts: the desktops and the start menus (one level of subfolders).
    pub shortcut_folders: Vec<PathBuf>,
}

impl UserPlaces {
    pub fn real() -> UserPlaces {
        let var = |n: &str| std::env::var_os(n).map(PathBuf::from);
        let roaming = var("APPDATA");
        let mut shortcut_folders = Vec::new();
        for home in [var("USERPROFILE"), var("PUBLIC")].into_iter().flatten() {
            shortcut_folders.push(home.join("Desktop"));
        }
        for base in [&roaming, &var("PROGRAMDATA")].into_iter().flatten() {
            shortcut_folders.push(base.join(r"Microsoft\Windows\Start Menu\Programs"));
        }
        UserPlaces {
            local: var("LOCALAPPDATA"),
            roaming,
            shortcut_folders,
        }
    }
}

fn files_of(dir: &Path) -> impl Iterator<Item = (String, PathBuf)> {
    fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .take(MAX_PER_FOLDER)
        .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
        .map(|e| (e.file_name().to_string_lossy().into_owned(), e.path()))
}

fn shortcut_target(path: &Path) -> Option<String> {
    let meta = fs::metadata(path).ok()?;
    if meta.len() > lnk::MAX_LNK_BYTES {
        return None;
    }
    lnk::target(&fs::read(path).ok()?)
}

fn with_text(path: &Path, text: Option<String>) -> Option<Candidate> {
    let mut c = candidate_at(path, false)?;
    c.text = text;
    Some(c)
}

/// Looks for the files. Reading shortcut targets is the slow part (a cold disk, a
/// virus scanner): after `budget` it stops reading them and says so (`false` = it did
/// not finish), keeping what it had found.
pub fn find_files(
    places: &UserPlaces,
    names: &[String],
    seeds: &Seeds,
    budget: Duration,
) -> (Vec<Candidate>, bool) {
    let until = Instant::now() + budget;
    let mut complete = true;
    let mut found = Vec::new();
    // Settings: `<name>rc`, `<name>.ini`... next to the others of the user.
    let data_dirs = [
        places.local.clone(),
        places.roaming.clone(),
        places.local.as_ref().map(|l| l.join("State")),
    ];
    if !names.is_empty() {
        for dir in data_dirs.iter().flatten() {
            for (name, path) in files_of(dir) {
                let s = simple(&name);
                if names.iter().any(|n| s.starts_with(n.as_str())) {
                    found.extend(with_text(&path, None));
                }
            }
        }
    }
    // Crash dumps: `<program>.exe.<pid>.dmp`.
    if let Some(local) = &places.local {
        for (name, path) in files_of(&local.join("CrashDumps")) {
            let lower = name.to_lowercase();
            if lower.ends_with(".dmp") && seeds.exes.iter().any(|e| lower.starts_with(e.as_str())) {
                found.extend(with_text(&path, None));
            }
        }
    }
    // Recent files of a type the app registered, or recent shortcuts into its folder.
    if let Some(roaming) = &places.roaming {
        shortcuts_in(
            &roaming.join(r"Microsoft\Windows\Recent"),
            seeds,
            Reading {
                by_extension: true,
                until,
            },
            &mut (&mut found, &mut complete),
        );
    }
    for folder in &places.shortcut_folders {
        let plain = Reading {
            by_extension: false,
            until,
        };
        shortcuts_in(folder, seeds, plain, &mut (&mut found, &mut complete));
        for sub in fs::read_dir(folder)
            .into_iter()
            .flatten()
            .flatten()
            .take(2_000)
        {
            if sub.file_type().is_ok_and(|t| t.is_dir()) {
                shortcuts_in(&sub.path(), seeds, plain, &mut (&mut found, &mut complete));
            }
        }
    }
    (found, complete)
}

#[derive(Clone, Copy)]
struct Reading {
    by_extension: bool,
    until: Instant,
}

fn shortcuts_in(
    dir: &Path,
    seeds: &Seeds,
    reading: Reading,
    out: &mut (&mut Vec<Candidate>, &mut bool),
) {
    let (found, complete) = out;
    let shortcuts: Vec<(String, PathBuf)> = files_of(dir)
        .filter_map(|(name, path)| {
            let lower = name.to_lowercase();
            lower
                .strip_suffix(".lnk")
                .map(|stem| (stem.to_string(), path))
        })
        .collect();
    // First by name, which costs nothing, over the whole list...
    let mut unmatched = Vec::new();
    for (stem, path) in shortcuts {
        if reading.by_extension && seeds.extensions.iter().any(|e| stem.ends_with(e.as_str())) {
            found.extend(with_text(&path, None));
        } else {
            unmatched.push(path);
        }
    }
    // ...then by target, which means reading each file: bounded in number and in time.
    // Not for the recent files: they point to the user's documents, and the list is long.
    if reading.by_extension {
        return;
    }
    let Some(folder) = &seeds.folder else {
        return;
    };
    for (read, path) in unmatched.iter().enumerate() {
        if read >= MAX_SHORTCUTS || Instant::now() > reading.until {
            **complete = false;
            break;
        }
        if let Some(target) = shortcut_target(path) {
            if path_refers_to(&target, folder) {
                found.extend(with_text(path, Some(target)));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::lnk::shortcut;
    use super::*;

    const LONG: Duration = Duration::from_secs(60);
    const APP: &str = r"C:\Program Files\Kdenlive";

    fn tree(name: &str) -> UserPlaces {
        let root = std::env::temp_dir().join(format!("purifex-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        for d in [
            r"Local\CrashDumps",
            r"Local\State",
            r"Roaming\Microsoft\Windows\Recent",
            r"Desktop",
            r"Menu\Editores",
        ] {
            fs::create_dir_all(root.join(d)).unwrap();
        }
        UserPlaces {
            local: Some(root.join("Local")),
            roaming: Some(root.join("Roaming")),
            shortcut_folders: vec![root.join("Desktop"), root.join("Menu")],
        }
    }

    fn put(places: &UserPlaces, rel: &str, bytes: &[u8]) {
        let base = places.local.as_ref().unwrap().parent().unwrap();
        fs::write(base.join(rel), bytes).unwrap();
    }

    fn seeds() -> Seeds {
        Seeds::new(Some(APP), &["kdenlive.exe".into()], &[".kdenlive".into()])
    }

    fn names_of(found: &[Candidate]) -> Vec<String> {
        let mut v: Vec<String> = found
            .iter()
            .map(|c| c.location.rsplit('\\').next().unwrap().to_string())
            .collect();
        v.sort();
        v
    }

    #[test]
    fn settings_dumps_and_shortcuts_of_the_app_are_found_and_the_rest_is_not() {
        let p = tree("files");
        let kdenlive_exe = format!(r"{APP}\bin\kdenlive.exe");
        for f in [
            r"Local\kdenliverc",
            r"Local\State\kdenlivestaterc",
            r"Roaming\Kdenlive.json",
            r"Local\otherrc",
            r"Local\CrashDumps\kdenlive.exe.33876.dmp",
            r"Local\CrashDumps\other.exe.1.dmp",
        ] {
            put(&p, f, b"x");
        }
        put(
            &p,
            r"Roaming\Microsoft\Windows\Recent\Video.kdenlive.lnk",
            b"x",
        );
        put(&p, r"Roaming\Microsoft\Windows\Recent\Notes.txt.lnk", b"x");
        put(
            &p,
            r"Desktop\Editor.lnk",
            &shortcut(&kdenlive_exe, true, true),
        );
        put(
            &p,
            r"Desktop\Other.lnk",
            &shortcut(r"C:\Program Files\Kdenlive2\k.exe", true, true),
        );
        put(
            &p,
            r"Menu\Editores\Kdenlive.lnk",
            &shortcut(&kdenlive_exe, false, true),
        );
        put(&p, r"Desktop\broken.lnk", b"not a shortcut");

        let (found, complete) = find_files(&p, &["kdenlive".into()], &seeds(), LONG);
        assert!(complete);
        assert_eq!(
            names_of(&found),
            [
                "Editor.lnk",
                "Kdenlive.json",
                "Kdenlive.lnk",
                "Video.kdenlive.lnk",
                "kdenlive.exe.33876.dmp",
                "kdenliverc",
                "kdenlivestaterc",
            ]
        );
        let editor = found
            .iter()
            .find(|c| c.location.ends_with("Editor.lnk"))
            .unwrap();
        assert_eq!(editor.text.as_deref(), Some(kdenlive_exe.as_str()));
        let _ = fs::remove_dir_all(p.local.unwrap().parent().unwrap());
    }

    #[test]
    fn a_long_list_of_recent_shortcuts_does_not_hide_the_ones_of_the_apps_extension() {
        let p = tree("files-recent");
        let recent = p
            .roaming
            .clone()
            .unwrap()
            .join("Microsoft")
            .join("Windows")
            .join("Recent");
        for i in 0..1_700 {
            fs::write(recent.join(format!("Doc {i}.docx.lnk")), b"x").unwrap();
        }
        put(
            &p,
            r"Roaming\Microsoft\Windows\Recent\Video.kdenlive.lnk",
            b"x",
        );
        let (found, complete) = find_files(&p, &[], &seeds(), LONG);
        assert_eq!(names_of(&found), ["Video.kdenlive.lnk"]);
        assert!(complete);
        let _ = fs::remove_dir_all(p.local.unwrap().parent().unwrap());
    }

    #[test]
    fn past_its_budget_it_stops_reading_shortcuts_says_so_and_keeps_what_it_found() {
        let p = tree("files-late");
        put(&p, r"Local\kdenliverc", b"x");
        put(
            &p,
            r"Desktop\Editor.lnk",
            &shortcut(&format!(r"{APP}\k.exe"), true, true),
        );
        let (found, complete) = find_files(&p, &["kdenlive".into()], &seeds(), Duration::ZERO);
        assert!(!complete);
        assert_eq!(names_of(&found), ["kdenliverc"]);
        let _ = fs::remove_dir_all(p.local.unwrap().parent().unwrap());
    }

    #[test]
    fn without_seeds_or_names_nothing_is_found() {
        let p = tree("files-none");
        put(&p, r"Local\kdenliverc", b"x");
        put(
            &p,
            r"Desktop\Editor.lnk",
            &shortcut(&format!(r"{APP}\k.exe"), true, true),
        );
        assert!(find_files(&p, &[], &Seeds::default(), LONG).0.is_empty());
        let _ = fs::remove_dir_all(p.local.unwrap().parent().unwrap());
    }

    #[test]
    fn folders_that_do_not_exist_are_skipped() {
        let p = UserPlaces {
            local: Some(PathBuf::from(r"C:\no\existe\jamas")),
            roaming: None,
            shortcut_folders: vec![PathBuf::from(r"C:\tampoco\existe")],
        };
        let (found, complete) = find_files(&p, &["kdenlive".into()], &seeds(), LONG);
        assert!(found.is_empty() && complete);
    }
}
