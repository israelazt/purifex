//! The registry places that can hold traces of an app, from a fixed list: values of
//! the cache and usage keys of Windows that point into the app's folder, the file types
//! it registered (and the keys of Explorer that hang from them), and the keys named
//! after its programs. It only reads and proposes; the rules of the domain decide.

use purifex_domain::app_traces::{Candidate, TraceKind};
use purifex_domain::reference::path_refers_to;
use purifex_domain::seeds::{extensions_into, Seeds};
use windows::Win32::System::Registry::{HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};

use crate::registry::{Key, View};

/// Where `Software` lives in each hive. Replaceable by a scratch branch of `HKCU` to
/// test; `hklm: None` leaves the machine alone.
#[derive(Debug, Clone)]
pub struct Hives {
    pub hkcu: String,
    pub hklm: Option<String>,
}

impl Hives {
    pub fn real() -> Hives {
        Hives {
            hkcu: "Software".into(),
            hklm: Some("SOFTWARE".into()),
        }
    }

    fn each(&self) -> Vec<(&'static str, HKEY, &str)> {
        let mut v = vec![("HKCU", HKEY_CURRENT_USER, self.hkcu.as_str())];
        if let Some(base) = &self.hklm {
            v.push(("HKLM", HKEY_LOCAL_MACHINE, base.as_str()));
        }
        v
    }
}

/// Keys of Windows that only hold caches and usage lists, with the hives they live in
/// (`true`: also machine-wide). One value of them may be the app's.
const VALUE_KEYS: [(&str, bool); 8] = [
    (
        r"Classes\Local Settings\Software\Microsoft\Windows\Shell\MuiCache",
        false,
    ),
    (
        r"Microsoft\Windows NT\CurrentVersion\AppCompatFlags\Compatibility Assistant\Store",
        false,
    ),
    (
        r"Microsoft\Windows NT\CurrentVersion\AppCompatFlags\Layers",
        true,
    ),
    (
        r"Microsoft\Windows\CurrentVersion\Explorer\FeatureUsage\AppSwitched",
        false,
    ),
    (
        r"Microsoft\Windows\CurrentVersion\Explorer\FeatureUsage\AppLaunch",
        false,
    ),
    (
        r"Microsoft\Windows\CurrentVersion\Explorer\FeatureUsage\ShowJumpView",
        false,
    ),
    (
        r"Microsoft\Windows\CurrentVersion\Search\JumplistData",
        false,
    ),
    (
        r"Microsoft\Windows\CurrentVersion\ApplicationAssociationToasts",
        false,
    ),
];

const EXPLORER: &str = r"Microsoft\Windows\CurrentVersion\Explorer";
const CURRENT_VERSION: &str = r"Microsoft\Windows\CurrentVersion";

fn exists(root: HKEY, path: &str) -> bool {
    Key::open(root, path, View::Bits64).is_some()
}

fn key_candidate(label: &str, path: &str, text: Option<String>) -> Candidate {
    Candidate {
        kind: TraceKind::RegistryKey,
        location: format!(r"{label}\{path}"),
        text,
        ..Default::default()
    }
}

/// The values of the fixed keys whose name or text points into the app's folder.
pub fn find_values(hives: &Hives, seeds: &Seeds) -> Vec<Candidate> {
    let Some(folder) = &seeds.folder else {
        return Vec::new();
    };
    let mut found = Vec::new();
    for (label, root, base) in hives.each() {
        for (rel, machine_too) in VALUE_KEYS {
            if label == "HKLM" && !machine_too {
                continue;
            }
            let path = format!(r"{base}\{rel}");
            let Some(key) = Key::open(root, &path, View::Bits64) else {
                continue;
            };
            for (name, text) in key.values() {
                let refers = path_refers_to(&name, folder)
                    || text.as_deref().is_some_and(|t| path_refers_to(t, folder));
                if refers {
                    found.push(Candidate {
                        kind: TraceKind::RegistryValue,
                        location: format!(r"{label}\{path}"),
                        value: Some(name),
                        text,
                        ..Default::default()
                    });
                }
            }
        }
    }
    found
}

/// The names of the programs (`kdenlive.exe`) that the found values point to inside the
/// app's folder.
pub fn exes_in_values(values: &[Candidate], folder: Option<&str>) -> Vec<String> {
    let Some(folder) = folder else {
        return Vec::new();
    };
    let mut exes = Vec::new();
    for c in values {
        for text in [c.value.as_deref(), c.text.as_deref()]
            .into_iter()
            .flatten()
        {
            if !path_refers_to(text, folder) {
                continue;
            }
            let lower = text.to_lowercase();
            for (at, _) in lower.match_indices(".exe") {
                let start = lower[..at].rfind(['\\', '/']).map_or(0, |i| i + 1);
                exes.push(lower[start..at + 4].to_string());
            }
        }
    }
    exes.sort();
    exes.dedup();
    exes
}

/// Every extension registered in `Classes`, with the ProgId it names (or itself when
/// the extension carries its own commands).
pub fn registered_extensions(hives: &Hives) -> Vec<(String, String)> {
    let mut pairs = Vec::new();
    for (_, root, base) in hives.each() {
        let path = format!(r"{base}\Classes");
        let Some(classes) = Key::open(root, &path, View::Bits64) else {
            continue;
        };
        for name in classes.subkeys().into_iter().filter(|n| n.starts_with('.')) {
            let progid = Key::open(root, &format!(r"{path}\{name}"), View::Bits64)
                .and_then(|k| k.text(""))
                .filter(|p| !p.is_empty())
                .unwrap_or_else(|| name.clone());
            pairs.push((name, progid));
        }
    }
    pairs
}

/// What a ProgId runs and shows, in one hive: its open command and its icon.
fn commands_in(root: HKEY, base: &str, progid: &str) -> Vec<String> {
    [r"shell\open\command", "DefaultIcon"]
        .iter()
        .filter_map(|sub| {
            Key::open(
                root,
                &format!(r"{base}\Classes\{progid}\{sub}"),
                View::Bits64,
            )
            .and_then(|k| k.text(""))
        })
        .collect()
}

/// What a ProgId runs and shows, in any hive.
pub fn commands_of(hives: &Hives, progid: &str) -> Vec<String> {
    hives
        .each()
        .into_iter()
        .flat_map(|(_, root, base)| commands_in(root, base, progid))
        .collect()
}

/// The extensions registered whose program is inside the app's folder: the ones the
/// app registered, even when they were not seen before it was uninstalled.
pub fn discover_extensions(hives: &Hives, seeds: &Seeds) -> Vec<String> {
    match &seeds.folder {
        Some(folder) => extensions_into(folder, &registered_extensions(hives), |progid| {
            commands_of(hives, progid)
        }),
        None => Vec::new(),
    }
}

/// The file types of the app (`Classes\<ext>` and its ProgId) and the keys of Explorer
/// that hang from its extensions. `seeds.extensions` should include the discovered ones.
pub fn find_associations(hives: &Hives, seeds: &Seeds) -> Vec<Candidate> {
    let mut found = Vec::new();
    let pairs = registered_extensions(hives);
    for ext in &seeds.extensions {
        for (label, root, base) in hives.each() {
            let classes = format!(r"{base}\Classes");
            if exists(root, &format!(r"{classes}\{ext}")) {
                found.push(key_candidate(label, &format!(r"{classes}\{ext}"), None));
            }
            let progid = pairs
                .iter()
                .find(|(e, _)| e.eq_ignore_ascii_case(ext))
                .map(|(_, p)| p.as_str())
                .filter(|p| !p.starts_with('.'));
            if let Some(progid) = progid {
                let path = format!(r"{classes}\{progid}");
                if exists(root, &path) {
                    let text = commands_in(root, base, progid).join("\n");
                    found.push(key_candidate(label, &path, Some(text)));
                }
            }
        }
        // Explorer keeps the user's own lists by extension, in HKCU only.
        let no_dot = ext.trim_start_matches('.');
        let base = &hives.hkcu;
        for rel in [
            format!(r"{EXPLORER}\FileExts\{ext}"),
            format!(r"{EXPLORER}\RecentDocs\{ext}"),
            format!(r"{EXPLORER}\ComDlg32\OpenSavePidlMRU\{ext}"),
            format!(r"{EXPLORER}\ComDlg32\OpenSavePidlMRU\{no_dot}"),
        ] {
            let path = format!(r"{base}\{rel}");
            if exists(HKEY_CURRENT_USER, &path) {
                found.push(key_candidate("HKCU", &path, None));
            }
        }
    }
    found
}

/// Keys named after the app's programs: notification settings, and `App Paths`.
pub fn find_program_keys(hives: &Hives, names: &[String], seeds: &Seeds) -> Vec<Candidate> {
    let mut found = Vec::new();
    let mut stems: Vec<String> = names.to_vec();
    stems.extend(
        seeds
            .exes
            .iter()
            .map(|e| e.trim_end_matches(".exe").to_string()),
    );
    stems.sort();
    stems.dedup();
    let base = &hives.hkcu;
    for stem in &stems {
        for rel in [
            format!(r"{CURRENT_VERSION}\Notifications\Settings\{stem}"),
            format!(r"{CURRENT_VERSION}\PushNotifications\Backup\{stem}"),
        ] {
            let path = format!(r"{base}\{rel}");
            if exists(HKEY_CURRENT_USER, &path) {
                found.push(key_candidate("HKCU", &path, None));
            }
        }
    }
    for exe in &seeds.exes {
        for (label, root, base) in hives.each() {
            let path = format!(r"{base}\{CURRENT_VERSION}\App Paths\{exe}");
            if exists(root, &path) {
                found.push(key_candidate(label, &path, None));
            }
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use purifex_domain::app_traces::{judge_with, Evidence};
    use purifex_domain::apps::InstalledApp;

    use super::super::registry_write::{delete_key_tree, set_value_raw, RawValue};
    use super::*;

    fn app() -> InstalledApp {
        InstalledApp {
            id: "x".into(),
            name: "Fake App".into(),
            ..Default::default()
        }
    }

    /// Plants in a scratch branch what a registered app leaves: its extension and ProgId,
    /// the Explorer lists, a notification key, `App Paths` and three values in `MuiCache`
    /// (one of the app, one of `Fake App2`, one of Windows).
    fn plant(hk: &str, folder: &str, ext: &str, progid: &str, id: u32) {
        let exe = format!(r"{folder}\bin\fake.exe");
        let cv = r"Microsoft\Windows\CurrentVersion";
        let set = |key: &str, name: &str, value: &str| {
            set_value_raw(&format!(r"{hk}\{key}"), name, &RawValue::sz(value)).unwrap();
        };
        set(&format!(r"Classes\{ext}"), "", progid);
        set(
            &format!(r"Classes\{progid}\shell\open\command"),
            "",
            &format!(r#""{exe}" "%1""#),
        );
        set(&format!(r"Classes\.po{id}"), "", "txtfile");
        set(&format!(r"{cv}\Explorer\FileExts\{ext}"), "x", "1");
        set(&format!(r"{cv}\Explorer\RecentDocs\{ext}"), "x", "1");
        set(
            &format!(r"{cv}\Explorer\ComDlg32\OpenSavePidlMRU\{}", &ext[1..]),
            "x",
            "1",
        );
        set(&format!(r"{cv}\Notifications\Settings\fake"), "x", "1");
        set(&format!(r"{cv}\App Paths\fake.exe"), "", &exe);
        let mui = r"Classes\Local Settings\Software\Microsoft\Windows\Shell\MuiCache";
        set(mui, &format!("{exe}.FriendlyAppName"), "Fake");
        set(
            mui,
            &format!(r"{folder}2\bin\fake.exe.FriendlyAppName"),
            "Fake2",
        );
        set(mui, r"C:\Windows\other.exe.FriendlyAppName", "Other");
    }

    /// Judges what the finder proposed (with the scratch prefix swapped for the real one)
    /// and returns the last part of each location with its evidence. Panics if the rules
    /// refuse any of it.
    fn judged_ends(found: &[Candidate], seeds: &Seeds, scratch: &str) -> Vec<(String, Evidence)> {
        let real = |c: &Candidate| Candidate {
            location: c.location.replacen(scratch, "Software", 1),
            ..c.clone()
        };
        let judged: Vec<(String, Evidence)> = found
            .iter()
            .map(|c| judge_with(&app(), &[], seeds, &real(c)).map(|t| (t.location, t.evidence)))
            .collect::<Result<_, _>>()
            .unwrap_or_else(|e| panic!("a proposal was refused ({e:?}): {found:#?}"));
        judged
            .iter()
            .map(|(l, e)| (l.rsplit('\\').next().unwrap_or(l).to_string(), *e))
            .collect()
    }

    /// What the finder proposes in a scratch branch of HKCU, judged by the domain with the
    /// scratch prefix swapped for the real one: so the layout of the fixed list and the
    /// rules agree.
    #[test]
    fn what_the_finder_proposes_in_every_fixed_place_passes_the_rules_and_the_rest_does_not() {
        let id = std::process::id();
        let scratch = format!(r"Software\PurifexTest\h{id}");
        let hives = Hives {
            hkcu: scratch.clone(),
            hklm: None,
        };
        let folder = std::env::temp_dir()
            .join(format!("purifex-reg-{id}"))
            .join("Fake App");
        let folder = folder.to_string_lossy().into_owned();
        let ext = format!(".pt{}", id % 100_000);
        let progid = format!("PT.File{id}");
        let hk = format!(r"HKCU\{scratch}");
        plant(&hk, &folder, &ext, &progid, id);

        let seeds = Seeds::new(Some(&folder), &["fake.exe".into()], &[]);
        let discovered = discover_extensions(&hives, &seeds);
        let seeds = Seeds::new(Some(&folder), &["fake.exe".into()], &discovered);
        let mut found = find_values(&hives, &seeds);
        found.extend(find_associations(&hives, &seeds));
        found.extend(find_program_keys(&hives, &["fakeapp".into()], &seeds));

        let _ = delete_key_tree(&hk);

        assert_eq!(
            discovered,
            vec![ext.clone()],
            "the extension was not given but is found"
        );
        let ends = judged_ends(&found, &seeds, &scratch);
        assert_eq!(found.len(), 8, "{found:#?}");
        for want in [
            (ext, Evidence::AssociatedExtension),
            (progid, Evidence::Association),
            ("fake".to_string(), Evidence::NameMatch),
            ("fake.exe".to_string(), Evidence::NameMatch),
            ("MuiCache".to_string(), Evidence::PathReference),
        ] {
            assert!(ends.contains(&want), "{want:?} in {ends:?}");
        }
        assert_eq!(
            ends.iter()
                .filter(|(_, e)| *e == Evidence::AssociatedExtension)
                .count(),
            4,
            "the extension, FileExts, RecentDocs and OpenSavePidlMRU: {ends:?}"
        );
        let value = found.iter().find(|c| c.value.is_some()).unwrap();
        assert!(value
            .value
            .as_deref()
            .unwrap()
            .ends_with("fake.exe.FriendlyAppName"));
    }

    #[test]
    fn the_programs_seen_in_values_inside_the_folder_are_named() {
        let value = |name: &str, text: Option<&str>| Candidate {
            kind: TraceKind::RegistryValue,
            location: r"HKCU\Software\X".into(),
            value: Some(name.into()),
            text: text.map(String::from),
            ..Default::default()
        };
        let values = [
            value(
                r"C:\Program Files\Kdenlive\bin\Kdenlive.exe.FriendlyAppName",
                None,
            ),
            value("x", Some(r#""C:\Program Files\Kdenlive\bin\melt.exe" -x"#)),
            value(r"C:\Program Files\Kdenlive2\bin\other.exe", None),
            value(r"C:\Windows\notepad.exe", None),
        ];
        assert_eq!(
            exes_in_values(&values, Some(r"C:\Program Files\Kdenlive")),
            ["kdenlive.exe", "melt.exe"]
        );
        assert!(exes_in_values(&values, None).is_empty());
    }

    #[test]
    fn without_a_folder_there_are_no_values_and_a_missing_hive_gives_nothing() {
        let hives = Hives {
            hkcu: r"Software\PurifexTest\no_existe".into(),
            hklm: None,
        };
        let seeds = Seeds::default();
        assert!(find_values(&hives, &seeds).is_empty());
        assert!(discover_extensions(&hives, &seeds).is_empty());
        assert!(find_associations(&hives, &seeds).is_empty());
        assert!(find_program_keys(&hives, &[], &seeds).is_empty());
    }
}
