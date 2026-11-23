//! Hunter mode: the person drags a sight onto a window and the program says which app it
//! belongs to. Only the reading of what was pointed at is here: which installed app a program
//! belongs to, and which programs may be closed. What is done to it (uninstall, close, open
//! the folder) goes through the same confirmations and rules as everywhere else.

use crate::apps::{AppKind, InstalledApp};
use crate::forced::RunningProcess;
use crate::seeds::seeds_of;
use crate::startup::StartupItem;

/// The window state of the sight: held by the person, and what it was let go on.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Hunt {
    /// The sight is being dragged (the pointer is a crosshair).
    pub aiming: bool,
    /// What the last drop landed on.
    pub found: Option<Hunted>,
}

/// A program pointed at, and the installed app it belongs to when it is one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hunted {
    pub process: RunningProcess,
    pub app: Option<InstalledApp>,
}

impl Hunted {
    /// The folder of its `.exe`.
    pub fn folder(&self) -> &str {
        self.process
            .path
            .rsplit_once('\\')
            .map_or(&self.process.path, |(folder, _)| folder)
    }

    /// Why this program must not be closed from here, if so: Windows itself runs it (closing
    /// `explorer.exe` takes the desktop away) or it is this program.
    pub fn protected(&self, own: u32) -> Option<&'static str> {
        let path = self.process.path.to_lowercase();
        if self.process.pid == own {
            Some("es el propio Purifex")
        } else if path.starts_with(r"c:\windows\")
            || path.contains(r"\windowsapps\microsoft.windows")
        {
            Some("es parte de Windows: cerrarlo puede dejar el equipo sin escritorio")
        } else {
            None
        }
    }
}

/// The installed app a program belongs to: the one of the Store whose package folder holds it,
/// else the one whose install folder holds it (the deepest, when one is inside another).
pub fn app_of(path: &str, apps: &[InstalledApp]) -> Option<InstalledApp> {
    let path = path.to_lowercase();
    let store = apps.iter().find(|a| {
        a.kind == AppKind::Store && path.split('\\').any(|part| part == a.id.to_lowercase())
    });
    let folder_of = |a: &InstalledApp| seeds_of(a).folder.map(|f| f.to_lowercase());
    let by_folder = apps
        .iter()
        .filter_map(|a| Some((a, folder_of(a)?)))
        .filter(|(_, f)| {
            path.strip_prefix(f.trim_end_matches('\\'))
                .is_some_and(|rest| rest.starts_with('\\'))
        })
        .max_by_key(|(_, f)| f.len());
    store.or(by_folder.map(|(a, _)| a)).cloned()
}

impl Hunted {
    /// The entry of what starts with Windows that runs this program, or belongs to its app, and
    /// that may be taken out (a service is only shown).
    pub fn startup_entry<'a>(&self, items: &'a [StartupItem]) -> Option<&'a StartupItem> {
        let mine = |i: &&StartupItem| {
            i.program().eq_ignore_ascii_case(&self.process.path)
                || self
                    .app
                    .as_ref()
                    .is_some_and(|a| i.app.as_deref() == Some(a.name.as_str()))
        };
        items.iter().filter(|i| i.trace().is_some()).find(mine)
    }
}

/// What a program pointed at is: its app, if it has one among the `apps` listed.
pub fn hunted(process: RunningProcess, apps: &[InstalledApp]) -> Hunted {
    let app = app_of(&process.path, apps);
    Hunted { process, app }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app(name: &str, folder: Option<&str>) -> InstalledApp {
        InstalledApp {
            id: format!("HKLM:{name}"),
            name: name.into(),
            install_location: folder.map(str::to_string),
            ..Default::default()
        }
    }

    fn process(pid: u32, path: &str) -> RunningProcess {
        RunningProcess {
            pid,
            path: path.into(),
        }
    }

    #[test]
    fn a_program_belongs_to_the_app_whose_folder_holds_it_and_the_deepest_one_wins() {
        let apps = [
            app("Suite", Some(r"C:\Program Files\Suite")),
            app("Suite Tool", Some(r"C:\Program Files\Suite\Tool")),
            app("Other", Some(r"C:\Program Files\Other")),
        ];
        let name = |p: &str| app_of(p, &apps).map(|a| a.name);
        assert_eq!(
            name(r"C:\Program Files\Suite\main.exe").as_deref(),
            Some("Suite")
        );
        assert_eq!(
            name(r"c:\program files\suite\tool\t.exe").as_deref(),
            Some("Suite Tool")
        );
        // A folder that only starts with the same letters is another folder.
        assert_eq!(name(r"C:\Program Files\SuiteX\x.exe"), None);
        assert_eq!(name(r"C:\Windows\explorer.exe"), None);
    }

    #[test]
    fn a_program_of_the_store_belongs_to_the_package_that_holds_it() {
        let package = "Microsoft.Paint_11.2.0.0_x64__8wekyb3d8bbwe";
        let paint = InstalledApp {
            id: package.into(),
            name: "Paint".into(),
            kind: AppKind::Store,
            ..Default::default()
        };
        let path = format!(r"C:\Program Files\WindowsApps\{package}\PaintApp\mspaint.exe");
        assert_eq!(
            app_of(&path, &[paint]).map(|a| a.name).as_deref(),
            Some("Paint")
        );
    }

    #[test]
    fn windows_and_this_program_are_never_closed_from_the_sight() {
        let on = |pid, path: &str| hunted(process(pid, path), &[]);
        assert!(on(4, r"C:\Windows\explorer.exe").protected(99).is_some());
        assert!(on(99, r"C:\Tools\purifex.exe").protected(99).is_some());
        assert!(on(7, r"C:\Tools\notes.exe").protected(99).is_none());
        assert_eq!(on(7, r"C:\Tools\notes.exe").folder(), r"C:\Tools");
    }
}
