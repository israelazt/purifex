//! What starts with Windows: the entries of `Run`, the Startup folders, the services that start
//! by themselves and the tasks that run at logon. Pure: reading them and changing them go
//! through `ports::StartupSource`.

use crate::app_traces::{Trace, TraceKind};
use crate::apps::InstalledApp;
use crate::reference::path_refers_to;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StartupKind {
    /// A value of a `Run` or `RunOnce` key.
    Registry,
    /// A file (usually a shortcut) in a Startup folder.
    Folder,
    /// A service of Windows set to start by itself (shown, never changed from here).
    Service,
    /// A scheduled task that runs at logon or at boot.
    Task,
}

impl StartupKind {
    pub fn label(self) -> &'static str {
        match self {
            StartupKind::Registry => "registro",
            StartupKind::Folder => "carpeta Inicio",
            StartupKind::Service => "servicio",
            StartupKind::Task => "tarea",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartupItem {
    /// Tells it apart from the others: kind, place and name.
    pub id: String,
    pub name: String,
    /// What it runs, with the variables of the system already expanded.
    pub command: String,
    pub kind: StartupKind,
    /// Where it lives: the key, the folder, the service or the task path.
    pub place: String,
    pub enabled: bool,
    /// Whether turning it on and off is something Windows keeps apart (without deleting).
    pub can_toggle: bool,
    /// Changing it needs an administrator (it is of the machine, not of the user).
    pub needs_admin: bool,
    /// The installed app whose folder it runs from, when one is.
    pub app: Option<String>,
}

impl StartupItem {
    /// The program it runs: the first `.exe` (or the whole command when none is named), without
    /// quotes or arguments.
    pub fn program(&self) -> String {
        program_of(&self.command)
    }

    /// The folder of that program, to open it.
    pub fn folder(&self) -> Option<String> {
        let program = self.program();
        program
            .rsplit_once('\\')
            .map(|(folder, _)| folder.to_string())
            .filter(|f| !f.is_empty())
    }

    /// What taking it out removes, as a trace the quarantine can keep and give back. Services
    /// are not removed from here.
    pub fn trace(&self) -> Option<Trace> {
        let (kind, location, value) = match self.kind {
            StartupKind::Registry => (
                TraceKind::RegistryValue,
                self.place.clone(),
                Some(self.name.clone()),
            ),
            StartupKind::Folder => (TraceKind::File, self.place.clone(), None),
            StartupKind::Task => (TraceKind::Task, self.place.clone(), None),
            StartupKind::Service => return None,
        };
        Some(Trace {
            kind,
            location,
            why: format!("programa de arranque: {}", self.name),
            value,
            ..Trace::default()
        })
    }

    /// Fills `app` from the apps whose folder this item runs from.
    pub fn with_app(mut self, apps: &[InstalledApp]) -> Self {
        let program = self.program();
        self.app = apps
            .iter()
            .filter(|a| a.protection().is_none() || a.install_location.is_some())
            .find(|a| {
                a.install_location.as_deref().is_some_and(|folder| {
                    !folder.trim().is_empty() && path_refers_to(&program, folder)
                })
            })
            .map(|a| a.name.clone());
        self
    }
}

/// The program of a command line: `"C:\A B\a.exe" /x` and `C:\A\a.exe /x` are `C:\A B\a.exe`
/// and `C:\A\a.exe`.
pub fn program_of(command: &str) -> String {
    let text = command.trim();
    if let Some(rest) = text.strip_prefix('"') {
        return rest.split('"').next().unwrap_or(rest).to_string();
    }
    let lower = text.to_lowercase();
    match lower.find(".exe") {
        Some(at) => text[..at + 4].to_string(),
        None => text.split_whitespace().next().unwrap_or("").to_string(),
    }
}

/// Enabled ones first, then by name.
pub fn sorted(mut items: Vec<StartupItem>) -> Vec<StartupItem> {
    items.sort_by(|a, b| {
        b.enabled
            .cmp(&a.enabled)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    items
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(kind: StartupKind, place: &str, name: &str, command: &str) -> StartupItem {
        StartupItem {
            id: format!("{place}|{name}"),
            name: name.into(),
            command: command.into(),
            kind,
            place: place.into(),
            enabled: true,
            can_toggle: true,
            needs_admin: false,
            app: None,
        }
    }

    #[test]
    fn the_program_is_found_in_quoted_and_plain_commands() {
        assert_eq!(
            program_of(r#""C:\Program Files\A B\a.exe" --tray"#),
            r"C:\Program Files\A B\a.exe"
        );
        assert_eq!(program_of(r"C:\A\a.EXE /x"), r"C:\A\a.EXE");
        assert_eq!(program_of("rundll32 shell32,x"), "rundll32");
        let i = item(
            StartupKind::Registry,
            "HKCU",
            "A",
            r#""C:\Apps\Zed\z.exe" -s"#,
        );
        assert_eq!(i.folder().as_deref(), Some(r"C:\Apps\Zed"));
    }

    #[test]
    fn it_is_matched_to_the_app_whose_folder_it_runs_from_and_not_to_a_neighbour() {
        let app = |name: &str, folder: &str| InstalledApp {
            id: name.into(),
            name: name.into(),
            install_location: Some(folder.into()),
            ..Default::default()
        };
        let apps = [
            app("Zed", r"C:\Apps\Zed"),
            app("Zed Pro", r"C:\Apps\Zed Pro"),
        ];
        let i = item(
            StartupKind::Registry,
            "HKCU",
            "Z",
            r#""C:\Apps\Zed Pro\z.exe""#,
        )
        .with_app(&apps);
        assert_eq!(i.app.as_deref(), Some("Zed Pro"));
        let none = item(StartupKind::Registry, "HKCU", "Z", r"D:\x\z.exe").with_app(&apps);
        assert_eq!(none.app, None);
    }

    #[test]
    fn what_can_be_taken_out_becomes_a_trace_and_a_service_does_not() {
        let r = item(
            StartupKind::Registry,
            r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run",
            "Zed",
            "z.exe",
        );
        let t = r.trace().unwrap();
        assert_eq!(
            (t.kind, t.value.as_deref()),
            (TraceKind::RegistryValue, Some("Zed"))
        );
        assert_eq!(
            item(StartupKind::Task, r"\Zed\Start", "Start", "z.exe")
                .trace()
                .unwrap()
                .kind,
            TraceKind::Task
        );
        assert_eq!(
            item(StartupKind::Folder, r"C:\S\z.lnk", "z", "z.exe")
                .trace()
                .unwrap()
                .kind,
            TraceKind::File
        );
        assert!(item(StartupKind::Service, "ZedSvc", "Zed", "z.exe")
            .trace()
            .is_none());
    }

    #[test]
    fn enabled_ones_come_first_then_by_name() {
        let mut off = item(StartupKind::Registry, "k", "Alpha", "a.exe");
        off.enabled = false;
        let on = item(StartupKind::Registry, "k", "beta", "b.exe");
        let names: Vec<_> = sorted(vec![off, on]).into_iter().map(|i| i.name).collect();
        assert_eq!(names, ["beta", "Alpha"]);
    }
}
