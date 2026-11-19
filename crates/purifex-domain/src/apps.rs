//! The apps installed in the computer, as the system lists them, and what may be
//! done with each. Plain facts and rules: reading the registry and running an
//! uninstaller belong to the adapters.

use serde::{Deserialize, Serialize};

/// How an app was installed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AppKind {
    /// A classic program with its own uninstaller (listed in the registry).
    Desktop,
    /// An app of the Microsoft Store (a package).
    Store,
}

/// One installed app. Every field but the name may be unknown.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstalledApp {
    /// Stable id: the registry key of the app, or the full name of the package.
    pub id: String,
    pub name: String,
    pub publisher: Option<String>,
    pub version: Option<String>,
    pub install_location: Option<String>,
    /// The command that uninstalls it, as the app registered it.
    pub uninstall: Option<String>,
    /// The silent version of that command, when the app has one.
    pub quiet_uninstall: Option<String>,
    pub size_kb: Option<u64>,
    pub kind: AppKind,
    /// The app says it is a part of the system or of another program (hidden in
    /// Windows' own list of apps), or it is an update.
    pub hidden: bool,
    /// The icon the app registered (`"C:\X\x.exe",0`): its folder is a seed when
    /// the app gave no install location.
    #[serde(default)]
    pub display_icon: Option<String>,
    /// What was observed of the app before its uninstaller was started (its programs
    /// and file extensions), kept because afterwards nothing is left to look at.
    #[serde(default)]
    pub observed: crate::seeds::Seeds,
}

impl InstalledApp {
    pub fn to_line(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    pub fn from_line(line: &str) -> Result<Self, String> {
        serde_json::from_str(line).map_err(|e| e.to_string())
    }

    /// Whether the list of apps shows it: it has a name and a way to uninstall it,
    /// and it is not hidden.
    pub fn is_listable(&self) -> bool {
        !self.hidden
            && !self.name.trim().is_empty()
            && (self.uninstall.is_some() || self.kind == AppKind::Store)
    }

    /// Why it must not be removed from here, if so: other programs and Windows
    /// depend on it, or it is what keeps the PC working.
    pub fn protection(&self) -> Option<&'static str> {
        let name = self.name.to_lowercase();
        let publisher = self.publisher.as_deref().unwrap_or("").to_lowercase();
        let microsoft = publisher.contains("microsoft");
        let runtimes = [
            "visual c++",
            ".net",
            "webview2",
            "windows sdk",
            "directx",
            "vulkan runtime",
        ];
        if (microsoft || name.contains("visual c++")) && runtimes.iter().any(|r| name.contains(r)) {
            return Some("lo usan otros programas: quitarlo puede romperlos");
        }
        if name.contains("microsoft edge") {
            return Some("es parte de Windows");
        }
        let drivers = [
            "nvidia graphics driver",
            "nvidia controlador",
            "amd software",
            "intel(r) graphics",
            "realtek",
        ];
        if drivers.iter().any(|d| name.contains(d)) {
            return Some("es un driver: se gestiona desde Drivers");
        }
        None
    }
}

/// The apps to show: only the listable ones, without repeats (the same program
/// appears in the 32 and 64-bit parts of the registry), by name.
pub fn listed(apps: Vec<InstalledApp>) -> Vec<InstalledApp> {
    let mut apps: Vec<InstalledApp> = apps.into_iter().filter(InstalledApp::is_listable).collect();
    apps.sort_by_key(|a| (a.name.to_lowercase(), a.version.clone(), a.id.clone()));
    apps.dedup_by(|b, a| a.name.eq_ignore_ascii_case(&b.name) && a.version == b.version);
    apps
}

/// A size in KB as text (`1,4 GB`, `220 MB`).
pub fn size_text(kb: Option<u64>) -> String {
    match kb {
        None | Some(0) => "–".into(),
        Some(kb) if kb >= 1_048_576 => {
            format!("{:.1} GB", kb as f64 / 1_048_576.0).replace('.', ",")
        }
        Some(kb) if kb >= 1024 => format!("{} MB", kb / 1024),
        Some(kb) => format!("{kb} KB"),
    }
}

/// The apps as text for the command line.
pub fn apps_to_text(apps: &[InstalledApp]) -> String {
    use std::fmt::Write;
    if apps.is_empty() {
        return "No se encontró ninguna app instalada.".into();
    }
    let mut out = String::new();
    for a in apps {
        let _ = writeln!(
            out,
            "• {}  {}  [{}]  {}",
            a.name,
            a.version.as_deref().unwrap_or(""),
            a.publisher.as_deref().unwrap_or("editor desconocido"),
            size_text(a.size_kb)
        );
        if let Some(why) = a.protection() {
            let _ = writeln!(out, "    no tocar: {why}");
        }
    }
    let _ = write!(out, "\n{} apps", apps.len());
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app(name: &str, publisher: &str, version: &str) -> InstalledApp {
        InstalledApp {
            id: format!("HKLM\\{name}{version}"),
            name: name.into(),
            publisher: Some(publisher.into()),
            version: Some(version.into()),
            install_location: None,
            uninstall: Some("C:\\x\\unins000.exe".into()),
            quiet_uninstall: None,
            size_kb: Some(2048),
            kind: AppKind::Desktop,
            hidden: false,
            display_icon: None,
            observed: Default::default(),
        }
    }

    #[test]
    fn only_apps_with_a_name_and_a_way_out_that_are_not_hidden_are_listed() {
        let mut no_name = app("", "X", "1");
        no_name.name = "  ".into();
        let mut hidden = app("Parche", "X", "1");
        hidden.hidden = true;
        let mut no_exit = app("Sin salida", "X", "1");
        no_exit.uninstall = None;
        let mut store = app("Tienda", "X", "1");
        store.uninstall = None;
        store.kind = AppKind::Store;
        let shown = listed(vec![
            app("Zeta", "X", "1"),
            no_name,
            hidden,
            no_exit,
            store,
            app("Alfa", "X", "1"),
        ]);
        assert_eq!(
            shown.iter().map(|a| a.name.as_str()).collect::<Vec<_>>(),
            ["Alfa", "Tienda", "Zeta"]
        );
    }

    #[test]
    fn the_same_program_of_the_two_registry_views_appears_once_but_two_versions_stay() {
        let a = app("Git", "Git", "2.40");
        let mut b = app("git", "Git", "2.40");
        b.id = "HKLM\\WOW6432\\Git".into();
        let c = app("Git", "Git", "2.30");
        let shown = listed(vec![a, b, c]);
        assert_eq!(shown.len(), 2);
    }

    #[test]
    fn what_other_programs_or_windows_rely_on_is_protected_with_the_reason() {
        for name in [
            "Microsoft Visual C++ 2015-2022 Redistributable (x64)",
            "Microsoft .NET Runtime - 8.0",
            "Microsoft Edge WebView2 Runtime",
        ] {
            assert!(
                app(name, "Microsoft Corporation", "1")
                    .protection()
                    .unwrap()
                    .contains("otros programas"),
                "{name}"
            );
        }
        assert!(app("Microsoft Edge", "Microsoft Corporation", "1")
            .protection()
            .is_some());
        assert!(app("NVIDIA Graphics Driver 566", "NVIDIA", "1")
            .protection()
            .unwrap()
            .contains("Drivers"));
        assert_eq!(app("Discord", "Discord Inc.", "1").protection(), None);
        assert_eq!(app("Steam", "Valve", "1").protection(), None);
    }

    #[test]
    fn sizes_read_like_a_person_would() {
        assert_eq!(size_text(None), "–");
        assert_eq!(size_text(Some(512)), "512 KB");
        assert_eq!(size_text(Some(225_280)), "220 MB");
        assert_eq!(size_text(Some(1_572_864)), "1,5 GB");
    }

    #[test]
    fn the_text_lists_each_app_and_says_why_a_protected_one_is_left_alone() {
        let text = apps_to_text(&[
            app("Discord", "Discord Inc.", "1.0"),
            app("Microsoft .NET Runtime", "Microsoft", "8"),
        ]);
        assert!(text.contains("• Discord"));
        assert!(text.contains("no tocar: lo usan otros programas"));
        assert!(text.ends_with("2 apps"));
        assert!(apps_to_text(&[]).contains("ninguna app"));
    }
}
