//! The services of Windows whose program is inside the app's folder. An uninstaller that
//! did not finish leaves its service registered, and Windows keeps trying to start it. This
//! only reads and proposes (the rule of the domain, `service_key`, decides).

use purifex_domain::app_traces::{Candidate, TraceKind};
use purifex_domain::reference::path_refers_to;
use purifex_domain::seeds::Seeds;
use windows::Win32::System::Registry::{HKEY, HKEY_LOCAL_MACHINE};

use crate::registry::{Key, View};

const SERVICES: &str = r"SYSTEM\CurrentControlSet\Services";

/// The services of this machine whose `ImagePath` points into `seeds.folder`.
pub fn find_services(seeds: &Seeds) -> Vec<Candidate> {
    services_in(HKEY_LOCAL_MACHINE, "HKLM", SERVICES, seeds)
}

/// The same under any key (a scratch one in the tests).
fn services_in(root: HKEY, label: &str, path: &str, seeds: &Seeds) -> Vec<Candidate> {
    let Some(folder) = seeds.folder.as_deref() else {
        return Vec::new();
    };
    let Some(parent) = Key::open(root, path, View::Bits64) else {
        return Vec::new();
    };
    parent
        .subkeys()
        .into_iter()
        .filter_map(|name| {
            let key = Key::open(root, &format!(r"{path}\{name}"), View::Bits64)?;
            let image = expand(&key.text("ImagePath")?);
            path_refers_to(&image, folder).then(|| Candidate {
                kind: TraceKind::RegistryKey,
                location: format!(r"{label}\{path}\{name}"),
                text: Some(image),
                ..Default::default()
            })
        })
        .collect()
}

/// `%ProgramFiles%\App\a.exe` as the path it names: `ImagePath` is an expandable string.
/// A variable that does not exist is left as it is.
pub(super) fn expand(text: &str) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(start) = rest.find('%') {
        let after = &rest[start + 1..];
        let Some(end) = after.find('%') else { break };
        out.push_str(&rest[..start]);
        match std::env::var(&after[..end]) {
            Ok(value) => out.push_str(&value),
            Err(_) => out.push_str(&rest[start..start + end + 2]),
        }
        rest = &after[end + 1..];
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::super::quarantine::reg;
    use super::*;
    use windows::Win32::System::Registry::HKEY_CURRENT_USER;

    #[test]
    fn variables_are_expanded_and_unknown_ones_are_left_alone() {
        std::env::set_var("PURIFEX_TEST_DIR", r"C:\Programas");
        assert_eq!(
            expand(r"%PURIFEX_TEST_DIR%\App\a.exe -x"),
            r"C:\Programas\App\a.exe -x"
        );
        assert_eq!(expand(r"%NO_EXISTE_XYZ%\a.exe"), r"%NO_EXISTE_XYZ%\a.exe");
        assert_eq!(expand("100% seguro"), "100% seguro");
    }

    #[test]
    fn only_the_services_whose_program_is_in_the_folder_are_found() {
        // A scratch branch of the user's registry stands in for the services key.
        let base = format!(r"Software\PurifexServicesTest{}", std::process::id());
        let full = format!(r"HKCU\{base}");
        let add = |name: &str, image: &str| {
            reg(&[
                "add",
                &format!(r"{full}\{name}"),
                "/v",
                "ImagePath",
                "/t",
                "REG_EXPAND_SZ",
                "/d",
                image,
                "/f",
            ])
            .unwrap()
        };
        add("Dentro", r"C:\Programas\PurifexApp\svc.exe -run");
        add("Cerca", r"C:\Programas\PurifexApp2\svc.exe");
        add("Fuera", r"C:\Windows\System32\svchost.exe -k x");
        let seeds = Seeds::new(Some(r"C:\Programas\PurifexApp"), &[], &[]);
        let found = services_in(HKEY_CURRENT_USER, "HKCU", &base, &seeds);
        let _ = reg(&["delete", &full, "/f"]);
        let names: Vec<&str> = found.iter().map(|c| c.location.as_str()).collect();
        assert_eq!(names, [format!(r"HKCU\{base}\Dentro")]);
        // Without a folder there is nothing to compare with.
        assert!(services_in(HKEY_CURRENT_USER, "HKCU", &base, &Seeds::default()).is_empty());
    }
}
