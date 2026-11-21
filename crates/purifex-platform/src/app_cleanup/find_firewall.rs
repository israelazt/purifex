//! The rules of Windows Firewall that name a program inside the app's folder. They are
//! values of one key of the machine; this only reads and proposes (the rule of the domain,
//! `firewall_value`, decides).

use purifex_domain::app_traces::{Candidate, TraceKind};
use purifex_domain::reference::path_refers_to;
use purifex_domain::seeds::Seeds;
use windows::Win32::System::Registry::{HKEY, HKEY_LOCAL_MACHINE};

use super::find_services::expand;
use crate::registry::{Key, View};

const RULES: &str =
    r"SYSTEM\CurrentControlSet\Services\SharedAccess\Parameters\FirewallPolicy\FirewallRules";

pub fn find_firewall_rules(seeds: &Seeds) -> Vec<Candidate> {
    rules_in(HKEY_LOCAL_MACHINE, "HKLM", RULES, seeds)
}

/// The same under any key (a scratch one in the tests).
fn rules_in(root: HKEY, label: &str, path: &str, seeds: &Seeds) -> Vec<Candidate> {
    let Some(folder) = seeds.folder.as_deref() else {
        return Vec::new();
    };
    let Some(key) = Key::open(root, path, View::Bits64) else {
        return Vec::new();
    };
    key.values()
        .into_iter()
        .filter_map(|(name, text)| {
            let text = expand(&text?);
            path_refers_to(&text, folder).then(|| Candidate {
                kind: TraceKind::RegistryValue,
                location: format!(r"{label}\{path}"),
                value: Some(name),
                text: Some(text),
                ..Default::default()
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::super::quarantine::reg;
    use super::*;
    use windows::Win32::System::Registry::HKEY_CURRENT_USER;

    #[test]
    fn only_the_rules_that_name_a_program_of_the_folder_are_found() {
        let base = format!(r"Software\PurifexFirewallTest{}", std::process::id());
        let full = format!(r"HKCU\{base}");
        let add = |name: &str, app: &str| {
            let text = format!("v2.33|Action=Allow|Active=TRUE|Dir=In|App={app}|Name=x|");
            reg(&["add", &full, "/v", name, "/d", &text, "/f"]).unwrap();
        };
        add("dentro", r"C:\Programas\PurifexApp\a.exe");
        add("cerca", r"C:\Programas\PurifexApp2\a.exe");
        add("fuera", r"C:\Windows\System32\svchost.exe");
        let seeds = Seeds::new(Some(r"C:\Programas\PurifexApp"), &[], &[]);
        let found = rules_in(HKEY_CURRENT_USER, "HKCU", &base, &seeds);
        let _ = reg(&["delete", &full, "/f"]);
        let names: Vec<_> = found.iter().filter_map(|c| c.value.as_deref()).collect();
        assert_eq!(names, ["dentro"]);
        assert!(rules_in(HKEY_CURRENT_USER, "HKCU", &base, &Seeds::default()).is_empty());
    }
}
