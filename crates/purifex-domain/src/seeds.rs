//! What must be known about an app to recognise its traces by reference: the folder
//! it installed in, its `.exe` names and the file extensions it registered. A seed
//! that is broad, short or generic is dropped, so a reference to it proves nothing
//! wrongly: no seed means no reference traces.

use serde::{Deserialize, Serialize};

use crate::app_traces::{last_part, simple, BROAD};
use crate::apps::InstalledApp;

/// Extensions that belong to everything: never proof of one app.
const GENERIC_EXTENSIONS: [&str; 24] = [
    ".txt", ".exe", ".dll", ".png", ".jpg", ".jpeg", ".gif", ".bmp", ".ico", ".pdf", ".zip",
    ".xml", ".json", ".html", ".htm", ".log", ".ini", ".cfg", ".dat", ".tmp", ".bat", ".cmd",
    ".lnk", ".url",
];

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Seeds {
    /// The install folder, absolute, without a trailing `\`.
    #[serde(default)]
    pub folder: Option<String>,
    /// File names of the app's programs, lowercase (`kdenlive.exe`).
    #[serde(default)]
    pub exes: Vec<String>,
    /// Extensions the app registered, lowercase with the dot (`.kdenlive`).
    #[serde(default)]
    pub extensions: Vec<String>,
}

impl Seeds {
    /// Keeps only the usable parts of what was observed.
    pub fn new(folder: Option<&str>, exes: &[String], extensions: &[String]) -> Seeds {
        let mut exes: Vec<String> = exes
            .iter()
            .map(|e| last_part(e.trim().trim_matches('"')).to_lowercase())
            .filter(|e| usable_exe(e))
            .collect();
        exes.sort();
        exes.dedup();
        let mut extensions: Vec<String> = extensions
            .iter()
            .map(|e| e.trim().to_lowercase())
            .filter(|e| usable_extension(e))
            .collect();
        extensions.sort();
        extensions.dedup();
        Seeds {
            folder: folder.and_then(usable_folder),
            exes,
            extensions,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.folder.is_none() && self.exes.is_empty() && self.extensions.is_empty()
    }
}

/// The seeds the registry entry alone gives: `InstallLocation`, else the folder of the
/// uninstall command. The `.exe` names and extensions are observed before uninstalling
/// and added with `Seeds::new`.
pub fn seeds_of(app: &InstalledApp) -> Seeds {
    let folder = app
        .install_location
        .as_deref()
        .and_then(usable_folder)
        .or_else(|| {
            app.uninstall
                .as_deref()
                .and_then(command_path)
                .and_then(|p| parent(&p))
                .and_then(|f| usable_folder(&f))
        });
    Seeds {
        folder,
        ..Seeds::default()
    }
}

fn parent(path: &str) -> Option<String> {
    path.rsplit_once('\\').map(|(p, _)| p.to_string())
}

/// The program path at the start of a command (`"C:\x\u.exe" /S` or `C:\x\u.exe /S`).
fn command_path(command: &str) -> Option<String> {
    let c = command.trim();
    if let Some(rest) = c.strip_prefix('"') {
        return rest.split('"').next().map(String::from);
    }
    let lower = c.to_lowercase();
    let end = lower.find(".exe")? + 4;
    Some(c[..end].to_string())
}

fn usable_folder(folder: &str) -> Option<String> {
    let f = folder.trim().trim_matches('"').trim_end_matches('\\');
    let b = f.as_bytes();
    if b.len() < 4 || b[1] != b':' || b[2] != b'\\' || !b[0].is_ascii_alphabetic() {
        return None;
    }
    let parts: Vec<&str> = f[3..].split('\\').collect();
    if parts
        .iter()
        .any(|p| p.is_empty() || *p == ".." || *p == ".")
    {
        return None;
    }
    let first = parts[0].to_lowercase();
    let last = parts[parts.len() - 1].to_lowercase();
    let broad = parts.len() < 2
        || first == "windows"
        || (first == "users" && parts.len() < 3)
        || BROAD.contains(&last.as_str())
        || simple(&last).len() < 3
        || parts
            .iter()
            .any(|p| p.starts_with('{') || p.eq_ignore_ascii_case("package cache"));
    (!broad).then(|| f.to_string())
}

fn usable_exe(exe: &str) -> bool {
    exe.strip_suffix(".exe")
        .is_some_and(|stem| simple(stem).len() >= 3)
}

fn usable_extension(ext: &str) -> bool {
    let Some(body) = ext.strip_prefix('.') else {
        return false;
    };
    (2..=10).contains(&body.len())
        && body.chars().all(|c| c.is_ascii_alphanumeric())
        && !GENERIC_EXTENSIONS.contains(&ext)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::apps::AppKind;

    fn app(location: Option<&str>, uninstall: Option<&str>) -> InstalledApp {
        InstalledApp {
            id: "x".into(),
            name: "Kdenlive".into(),
            publisher: None,
            version: None,
            install_location: location.map(String::from),
            uninstall: uninstall.map(String::from),
            quiet_uninstall: None,
            size_kb: None,
            kind: AppKind::Desktop,
            hidden: false,
        }
    }

    fn folder(location: &str) -> Option<String> {
        seeds_of(&app(Some(location), None)).folder
    }

    #[test]
    fn the_install_location_is_the_folder_seed() {
        assert_eq!(
            folder(r"C:\Program Files\Kdenlive\"),
            Some(r"C:\Program Files\Kdenlive".into())
        );
    }

    #[test]
    fn broad_short_or_generic_folders_are_not_seeds() {
        for bad in [
            r"C:\Program Files",
            r"C:\Program Files (x86)",
            r"C:\",
            r"C:\Windows\System32",
            r"C:\Users\Ana",
            r"C:\Users\Ana\AppData\Local",
            r"C:\ProgramData",
            r"C:\Apps",
            r"C:\Program Files\Ab",
            r"C:\Program Files\X",
            r"C:\ProgramData\Package Cache\{1-2}",
            r"relative\Kdenlive",
            r"C:\Program Files\..\Kdenlive",
        ] {
            assert_eq!(folder(bad), None, "{bad}");
        }
    }

    #[test]
    fn without_install_location_the_uninstall_commands_folder_is_used() {
        let a = app(
            None,
            Some(r#""C:\Program Files\Kdenlive\uninstall.exe" /S"#),
        );
        assert_eq!(
            seeds_of(&a).folder,
            Some(r"C:\Program Files\Kdenlive".into())
        );
        let b = app(None, Some(r"C:\Games\Cool Game\unins000.exe /SILENT"));
        assert_eq!(seeds_of(&b).folder, Some(r"C:\Games\Cool Game".into()));
    }

    #[test]
    fn a_shared_uninstaller_does_not_give_a_seed() {
        for cmd in [
            r"MsiExec.exe /X{ABC}",
            r"C:\Windows\System32\msiexec.exe /x {ABC}",
            r"C:\Windows\Installer\{ABC}\setup.exe",
        ] {
            assert_eq!(seeds_of(&app(None, Some(cmd))).folder, None, "{cmd}");
        }
    }

    #[test]
    fn exes_and_extensions_are_normalised_and_the_generic_ones_dropped() {
        let s = Seeds::new(
            Some(r"C:\Program Files\Kdenlive"),
            &[
                r"C:\Program Files\Kdenlive\bin\Kdenlive.exe".into(),
                "kdenlive.exe".into(),
                "a.exe".into(),
                "setup.msi".into(),
            ],
            &[".Kdenlive".into(), ".txt".into(), "kdl".into(), ".x".into()],
        );
        assert_eq!(s.exes, vec!["kdenlive.exe"]);
        assert_eq!(s.extensions, vec![".kdenlive"]);
        assert!(!s.is_empty());
        assert!(Seeds::new(None, &[], &[".txt".into()]).is_empty());
    }
}
