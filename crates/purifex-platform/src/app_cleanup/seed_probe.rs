//! Looks at an app that is still installed, before its uninstaller starts, to keep what
//! its traces will be recognised by afterwards: the programs in its folder and the file
//! extensions it registered. Read only.

use std::fs;
use std::path::Path;

use purifex_domain::apps::InstalledApp;
use purifex_domain::ports::SeedProbe;
use purifex_domain::seeds::{extensions_into, seeds_of, Seeds};
use windows::Win32::System::Registry::{HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};

use crate::registry::{Key, View};

/// Looks at no more than this many entries of the folder, and this many levels down.
const MAX_ENTRIES: usize = 400;
const MAX_DEPTH: usize = 2;

pub struct WindowsSeedProbe;

/// Where the file types are registered: the machine's and the user's.
const CLASSES: [(HKEY, &str, View); 2] = [
    (HKEY_LOCAL_MACHINE, r"SOFTWARE\Classes", View::Bits64),
    (HKEY_CURRENT_USER, r"Software\Classes", View::Bits64),
];

impl SeedProbe for WindowsSeedProbe {
    fn observe(&self, app: &InstalledApp) -> Seeds {
        let seeds = seeds_of(app);
        let Some(folder) = seeds.folder.clone() else {
            return seeds;
        };
        let mut exes = programs_in(Path::new(&folder));
        if let Some(icon) = &app.display_icon {
            exes.extend(exe_of_icon(icon));
        }
        let extensions = extensions_into(&folder, &registered_extensions(), commands_of);
        Seeds::new(Some(&folder), &exes, &extensions)
    }
}

/// The `.exe` files in the folder and a level or two below it (`bin`).
fn programs_in(folder: &Path) -> Vec<String> {
    let mut found = Vec::new();
    let mut seen = 0usize;
    let mut level = vec![folder.to_path_buf()];
    for _ in 0..=MAX_DEPTH {
        let mut next = Vec::new();
        for dir in &level {
            let Ok(entries) = fs::read_dir(dir) else {
                continue;
            };
            for entry in entries.flatten() {
                seen += 1;
                if seen > MAX_ENTRIES {
                    return found;
                }
                let path = entry.path();
                if path.is_dir() {
                    next.push(path);
                } else if path
                    .extension()
                    .is_some_and(|e| e.eq_ignore_ascii_case("exe"))
                {
                    if let Some(name) = path.file_name() {
                        found.push(name.to_string_lossy().into_owned());
                    }
                }
            }
        }
        level = next;
    }
    found
}

/// `"C:\x\x.exe",0` → `x.exe`.
fn exe_of_icon(icon: &str) -> Option<String> {
    let path = icon.trim().trim_start_matches('"');
    let end = path.to_lowercase().find(".exe")? + 4;
    path[..end].rsplit(['\\', '/']).next().map(String::from)
}

/// Every extension registered in `Classes`, with the ProgId it names (or itself when
/// the extension carries its own commands).
fn registered_extensions() -> Vec<(String, String)> {
    let mut pairs = Vec::new();
    for (root, path, view) in CLASSES {
        let Some(classes) = Key::open(root, path, view) else {
            continue;
        };
        for name in classes.subkeys().into_iter().filter(|n| n.starts_with('.')) {
            let progid = Key::open(root, &format!(r"{path}\{name}"), view)
                .and_then(|k| k.text(""))
                .filter(|p| !p.is_empty())
                .unwrap_or_else(|| name.clone());
            pairs.push((name, progid));
        }
    }
    pairs
}

/// What a ProgId runs and shows: its open command and its icon (user's and machine's).
fn commands_of(progid: &str) -> Vec<String> {
    let mut texts = Vec::new();
    for (root, path, view) in CLASSES {
        for sub in [r"shell\open\command", "DefaultIcon"] {
            if let Some(text) =
                Key::open(root, &format!(r"{path}\{progid}\{sub}"), view).and_then(|k| k.text(""))
            {
                texts.push(text);
            }
        }
    }
    texts
}

#[cfg(test)]
mod tests {
    use purifex_domain::apps::AppKind;

    use super::super::registry_write::{delete_key_tree, set_value_raw, RawValue};
    use super::*;

    fn sz(text: &str) -> RawValue {
        let mut data: Vec<u8> = text.encode_utf16().flat_map(u16::to_le_bytes).collect();
        data.extend([0, 0]);
        RawValue { kind: 1, data }
    }

    #[test]
    fn the_programs_and_the_extensions_of_an_app_are_seen_before_it_is_gone() {
        let id = std::process::id();
        let folder = std::env::temp_dir()
            .join(format!("purifex-seed-{id}"))
            .join("Fake App");
        let _ = fs::remove_dir_all(folder.parent().unwrap());
        fs::create_dir_all(folder.join("bin")).unwrap();
        fs::write(folder.join("bin").join("Fake.EXE"), b"x").unwrap();
        fs::write(folder.join("readme.txt"), b"x").unwrap();
        let ext = format!(".pt{}", id % 100_000);
        let progid = format!("PurifexTest.File{id}");
        let classes = r"HKCU\Software\Classes";
        let command = format!(
            r#""{}" "%1""#,
            folder.join("bin").join("Fake.EXE").display()
        );
        set_value_raw(&format!(r"{classes}\{ext}"), "", &sz(&progid)).unwrap();
        set_value_raw(
            &format!(r"{classes}\{progid}\shell\open\command"),
            "",
            &sz(&command),
        )
        .unwrap();
        // Another type that points somewhere else must not be taken.
        let other = format!(".po{}", id % 100_000);
        set_value_raw(&format!(r"{classes}\{other}"), "", &sz("txtfile")).unwrap();

        let app = InstalledApp {
            id: "x".into(),
            name: "Fake App".into(),
            publisher: None,
            version: None,
            install_location: Some(folder.to_string_lossy().into_owned()),
            uninstall: None,
            quiet_uninstall: None,
            size_kb: None,
            kind: AppKind::Desktop,
            hidden: false,
            display_icon: Some(format!(r#""{}",0"#, folder.join("Icon.exe").display())),
            observed: Default::default(),
        };
        let seeds = WindowsSeedProbe.observe(&app);

        for key in [&ext, &progid, &other] {
            let _ = delete_key_tree(&format!(r"{classes}\{key}"));
        }
        let _ = fs::remove_dir_all(folder.parent().unwrap());

        assert_eq!(
            seeds.folder.as_deref(),
            Some(folder.to_string_lossy().as_ref())
        );
        assert_eq!(seeds.exes, vec!["fake.exe", "icon.exe"]);
        assert_eq!(seeds.extensions, vec![ext.to_lowercase()]);
    }

    #[test]
    fn an_app_without_a_usable_folder_gives_no_seeds() {
        let app = InstalledApp {
            id: "x".into(),
            name: "Nada".into(),
            publisher: None,
            version: None,
            install_location: Some(r"C:\Program Files".into()),
            uninstall: None,
            quiet_uninstall: None,
            size_kb: None,
            kind: AppKind::Desktop,
            hidden: false,
            display_icon: None,
            observed: Default::default(),
        };
        assert!(WindowsSeedProbe.observe(&app).is_empty());
    }

    #[test]
    fn an_icon_gives_the_program_name() {
        assert_eq!(
            exe_of_icon(r#""C:\x\App.exe",0"#).as_deref(),
            Some("App.exe")
        );
        assert_eq!(exe_of_icon(r"C:\x\app.ico"), None);
    }
}
