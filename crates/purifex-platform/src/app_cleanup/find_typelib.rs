//! The type libraries an app registered: `Classes\TypeLib\{GUID}\<version>\<lcid>\win32|win64`
//! holds the path of the file. A library is proposed only when every file it names is inside
//! the app's folder (one that also names a file elsewhere is shared, and is left alone). It only
//! reads; the rules of the domain decide.

use purifex_domain::app_traces::{Candidate, TraceKind};
use purifex_domain::reference::path_refers_to;
use purifex_domain::seeds::Seeds;

use super::find_services::expand;
use super::find_shell::Classes;
use crate::registry::{Key, View};

/// The files a type library names, with their variables expanded (`win32` and `win64`, for each
/// version and language).
fn files_of(c: &Classes, guid: &str) -> Vec<String> {
    let base = format!(r"{}\TypeLib\{guid}", c.path());
    let Some(lib) = Key::open(c.root(), &base, View::Bits64) else {
        return Vec::new();
    };
    let mut files = Vec::new();
    for version in lib.subkeys() {
        let Some(languages) = Key::open(c.root(), &format!(r"{base}\{version}"), View::Bits64)
        else {
            continue;
        };
        for lcid in languages.subkeys() {
            for platform in ["win32", "win64"] {
                let path = format!(r"{base}\{version}\{lcid}\{platform}");
                if let Some(text) =
                    Key::open(c.root(), &path, View::Bits64).and_then(|k| k.text(""))
                {
                    files.push(expand(&text));
                }
            }
        }
    }
    files
}

/// The type libraries of the real `Classes` whose files are all inside the folder of the app.
pub fn find_typelibs(seeds: &Seeds) -> Vec<Candidate> {
    find_typelibs_in(&super::find_shell::REAL, seeds)
}

/// The same, in the `Classes` given.
pub fn find_typelibs_in(all: &[Classes], seeds: &Seeds) -> Vec<Candidate> {
    let Some(folder) = seeds.folder.as_deref() else {
        return Vec::new();
    };
    let mut found = Vec::new();
    for c in all {
        let Some(libs) = Key::open(c.root(), &format!(r"{}\TypeLib", c.path()), View::Bits64)
        else {
            continue;
        };
        for guid in libs.subkeys() {
            let files = files_of(c, &guid);
            if files.is_empty() || !files.iter().all(|f| path_refers_to(f, folder)) {
                continue;
            }
            found.push(Candidate {
                kind: TraceKind::RegistryKey,
                location: format!(r"{}\{}\TypeLib\{guid}", c.label(), c.path()),
                text: files.into_iter().next(),
                ..Default::default()
            });
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use windows::Win32::System::Registry::HKEY_CURRENT_USER;

    use super::super::quarantine::reg;
    use super::*;

    #[test]
    fn a_type_library_is_found_only_when_every_file_it_names_is_in_the_apps_folder() {
        let base = format!(r"Software\PurifexTypeLibTest{}", std::process::id());
        let add = |key: &str, file: &str| {
            let path = format!(r"HKCU\{base}\TypeLib\{key}");
            reg(&["add", &path, "/ve", "/d", file, "/f"]).unwrap();
        };
        add(r"{T1}\1.0\0\win32", r"C:\Programas\PurifexApp\lib.tlb");
        add(r"{T2}\1.0\0\win32", r"C:\Programas\PurifexApp2\lib.tlb");
        add(r"{T3}\1.0\0\win32", r"C:\Programas\PurifexApp\a.tlb");
        add(r"{T3}\2.0\0\win32", r"C:\Windows\System32\x.tlb");
        add(r"{T4}\1.0\0\win32", r"C:\Programas\PurifexApp\b.tlb");
        add(r"{T4}\1.0\0\win64", r"C:\Programas\PurifexApp\b64.tlb");
        let scratch = [Classes::new(HKEY_CURRENT_USER, "HKCU", &base)];
        let seeds = Seeds::new(Some(r"C:\Programas\PurifexApp"), &[], &[]);
        let found = find_typelibs_in(&scratch, &seeds);
        let none = find_typelibs_in(&scratch, &Seeds::default());
        let _ = reg(&["delete", &format!(r"HKCU\{base}"), "/f"]);
        let mut ends: Vec<&str> = found
            .iter()
            .map(|c| c.location.rsplit('\\').next().unwrap())
            .collect();
        ends.sort_unstable();
        assert_eq!(
            ends,
            ["{T1}", "{T4}"],
            "a shared library and a neighbour's are left alone"
        );
        assert!(
            none.is_empty(),
            "without a folder there is nothing to compare with"
        );
    }
}
