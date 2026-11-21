//! What an app registers to be part of the shell: its COM classes and the verbs and handlers
//! of the context menu. Each is proposed with the program it runs or loads, when that
//! program is inside the app's folder; the rules of the domain (`com_key`,
//! `context_menu_key`) decide. This only reads.

use purifex_domain::app_traces::{Candidate, TraceKind};
use purifex_domain::reference::path_refers_to;
use purifex_domain::seeds::Seeds;
use windows::Win32::System::Registry::{HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};

use super::find_services::expand;
use crate::registry::{Key, View};

/// A `Classes` key: the machine's (in its two views) and the user's.
#[derive(Clone, Copy)]
pub struct Classes<'a> {
    root: HKEY,
    label: &'a str,
    path: &'a str,
}

pub const REAL: [Classes<'static>; 3] = [
    Classes {
        root: HKEY_LOCAL_MACHINE,
        label: "HKLM",
        path: r"SOFTWARE\Classes",
    },
    Classes {
        root: HKEY_LOCAL_MACHINE,
        label: "HKLM",
        path: r"SOFTWARE\WOW6432Node\Classes",
    },
    Classes {
        root: HKEY_CURRENT_USER,
        label: "HKCU",
        path: r"Software\Classes",
    },
];

/// Where the verbs of the context menu hang.
const VERBS: [&str; 5] = [
    r"*\shell",
    r"Directory\shell",
    r"Directory\Background\shell",
    r"Drive\shell",
    r"AllFilesystemObjects\shell",
];

/// Where its handlers hang (each child names a COM class).
const HANDLERS: [&str; 5] = [
    r"*\shellex\ContextMenuHandlers",
    r"Directory\shellex\ContextMenuHandlers",
    r"Directory\Background\shellex\ContextMenuHandlers",
    r"Drive\shellex\ContextMenuHandlers",
    r"AllFilesystemObjects\shellex\ContextMenuHandlers",
];

/// Everything of the shell that runs or loads a program of the app.
pub fn find_shell(seeds: &Seeds) -> Vec<Candidate> {
    // The classes are thousands of keys in each place: each place is read on its own thread.
    // (`REAL` is a constant: every thread makes its own copy of the roots.)
    std::thread::scope(|scope| {
        let classes: Vec<_> = (0..REAL.len())
            .map(|i| scope.spawn(move || find_com(&REAL[i..=i], seeds)))
            .collect();
        let menu = scope.spawn(|| find_context_menu(&REAL, seeds));
        let mut found: Vec<Candidate> = classes
            .into_iter()
            .flat_map(|place| place.join().unwrap_or_default())
            .collect();
        found.extend(menu.join().unwrap_or_default());
        found
    })
}

fn candidate(c: &Classes, relative: &str, text: String) -> Candidate {
    Candidate {
        kind: TraceKind::RegistryKey,
        location: format!(r"{}\{}\{relative}", c.label, c.path),
        text: Some(text),
        ..Default::default()
    }
}

/// The text of the default value of a key, with its variables expanded.
fn default_of(c: &Classes, relative: &str) -> Option<String> {
    let key = Key::open(c.root, &format!(r"{}\{relative}", c.path), View::Bits64)?;
    key.text("").map(|t| expand(&t)).filter(|t| !t.is_empty())
}

/// The program that serves a COM class, looked for in every `Classes` key.
fn server_of(all: &[Classes], guid: &str) -> Option<String> {
    all.iter().find_map(|c| {
        ["InprocServer32", "LocalServer32"]
            .iter()
            .find_map(|kind| default_of(c, &format!(r"CLSID\{guid}\{kind}")))
    })
}

/// The COM classes whose server is inside the folder of the app.
pub fn find_com(all: &[Classes], seeds: &Seeds) -> Vec<Candidate> {
    let Some(folder) = seeds.folder.as_deref() else {
        return Vec::new();
    };
    let mut found = Vec::new();
    for c in all {
        let Some(clsids) = Key::open(c.root, &format!(r"{}\CLSID", c.path), View::Bits64) else {
            continue;
        };
        for guid in clsids.subkeys() {
            let server = ["InprocServer32", "LocalServer32"]
                .iter()
                .find_map(|kind| default_of(c, &format!(r"CLSID\{guid}\{kind}")));
            if let Some(server) = server.filter(|s| path_refers_to(s, folder)) {
                found.push(candidate(c, &format!(r"CLSID\{guid}"), server));
            }
        }
    }
    found
}

/// The verbs and handlers of the context menu that run or load a program of the app.
pub fn find_context_menu(all: &[Classes], seeds: &Seeds) -> Vec<Candidate> {
    let Some(folder) = seeds.folder.as_deref() else {
        return Vec::new();
    };
    let mut found = Vec::new();
    for c in all.iter().filter(|c| !c.path.contains("WOW6432Node")) {
        for place in VERBS {
            let Some(parent) = Key::open(c.root, &format!(r"{}\{place}", c.path), View::Bits64)
            else {
                continue;
            };
            for verb in parent.subkeys() {
                let command = default_of(c, &format!(r"{place}\{verb}\command"));
                if let Some(command) = command.filter(|t| path_refers_to(t, folder)) {
                    found.push(candidate(c, &format!(r"{place}\{verb}"), command));
                }
            }
        }
        for place in HANDLERS {
            let Some(parent) = Key::open(c.root, &format!(r"{}\{place}", c.path), View::Bits64)
            else {
                continue;
            };
            for handler in parent.subkeys() {
                // The class is the default value, or the name of the key when it is a guid.
                let guid = default_of(c, &format!(r"{place}\{handler}"))
                    .or_else(|| handler.starts_with('{').then(|| handler.clone()));
                let server = guid.and_then(|g| server_of(all, &g));
                if let Some(server) = server.filter(|s| path_refers_to(s, folder)) {
                    found.push(candidate(c, &format!(r"{place}\{handler}"), server));
                }
            }
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::super::quarantine::reg;
    use super::*;

    /// A scratch `Classes` under the user's registry, with an app's COM class, a verb and a
    /// handler, and the same of another app.
    fn plant(base: &str) {
        let full = format!(r"HKCU\{base}");
        let add = |key: &str, value: &str| {
            reg(&["add", &format!(r"{full}\{key}"), "/ve", "/d", value, "/f"]).unwrap();
        };
        add(
            r"CLSID\{AAAA}\InprocServer32",
            r"C:\Programas\PurifexApp\shell.dll",
        );
        add(
            r"CLSID\{BBBB}\InprocServer32",
            r"C:\Programas\PurifexApp2\shell.dll",
        );
        add(
            r"CLSID\{CCCC}\InprocServer32",
            r"C:\Windows\System32\shell32.dll",
        );
        add(
            r"*\shell\Abrir\command",
            r#""C:\Programas\PurifexApp\a.exe" "%1""#,
        );
        add(
            r"*\shell\Otro\command",
            r#""C:\Programas\PurifexApp2\a.exe" "%1""#,
        );
        add(r"*\shellex\ContextMenuHandlers\MiMenu", "{AAAA}");
        add(r"*\shellex\ContextMenuHandlers\OtroMenu", "{BBBB}");
    }

    #[test]
    fn only_what_runs_a_program_of_the_folder_is_found_in_the_com_classes_and_the_menu() {
        let base = format!(r"Software\PurifexShellTest{}", std::process::id());
        plant(&base);
        let scratch = [Classes {
            root: HKEY_CURRENT_USER,
            label: "HKCU",
            path: &base,
        }];
        let seeds = Seeds::new(Some(r"C:\Programas\PurifexApp"), &[], &[]);
        let com = find_com(&scratch, &seeds);
        let menu = find_context_menu(&scratch, &seeds);
        let _ = reg(&["delete", &format!(r"HKCU\{base}"), "/f"]);
        let ends = |found: &[Candidate]| -> Vec<String> {
            found
                .iter()
                .map(|c| c.location.rsplit('\\').next().unwrap().to_string())
                .collect()
        };
        assert_eq!(ends(&com), ["{AAAA}"]);
        assert_eq!(ends(&menu), ["Abrir", "MiMenu"]);
        let none = Seeds::default();
        assert!(
            find_com(&scratch, &none).is_empty() && find_context_menu(&scratch, &none).is_empty()
        );
    }
}
