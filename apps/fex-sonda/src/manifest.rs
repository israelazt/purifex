//! The list of everything Fex Sonda writes, in one place: `install`, `status` and `purge` all read
//! it. Adding a trace is adding a row.

// Nothing reads the list until `install` (T.2), `status` and `purge` (T.4) exist: remove this then.
#![allow(dead_code)]

/// Where a path starts. Resolved from the environment by `places`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Root {
    ProgramFiles,
    AppData,
    LocalAppData,
    ProgramData,
    /// The Start menu of every user.
    StartMenuCommon,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hive {
    Hkcu,
    Hklm,
}

/// What a file holds when it is written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Content {
    /// A copy of this very program.
    Itself,
    Text(&'static str),
    /// This many bytes of zeros.
    Zeros(usize),
    Empty,
}

/// A value of the registry. `{F}` in a text is the install folder.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Data {
    Text(&'static str),
    Dword(u32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Item {
    File {
        root: Root,
        path: &'static str,
        content: Content,
    },
    /// A folder and what is inside it. `files` are written with the folder.
    Dir {
        root: Root,
        path: &'static str,
        files: &'static [(&'static str, Content)],
    },
    RegKey {
        hive: Hive,
        path: &'static str,
        values: &'static [(&'static str, Data)],
    },
    /// A shortcut in a `Root`, pointing to `target` (`{F}` is the install folder).
    Lnk {
        root: Root,
        path: &'static str,
        target: &'static str,
    },
    /// A scheduled task that runs `command`.
    Task {
        name: &'static str,
        command: &'static str,
    },
}

/// What the official uninstaller does with a row, and so what Purifex should do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    /// The uninstaller removes it.
    RemovedByUninstaller,
    /// The uninstaller forgets it on purpose: Purifex must propose it.
    Leftover,
    /// Not Fex Sonda's: Purifex must never propose it.
    Control,
}

pub struct Row {
    /// «1», «2», … for what Fex Sonda writes; «C1», «C2»… for the controls.
    pub id: &'static str,
    pub item: Item,
    pub role: Role,
}

pub const UNINSTALL_KEY: &str = r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall";

const fn row(id: &'static str, role: Role, item: Item) -> Row {
    Row { id, item, role }
}

const fn reg(hive: Hive, path: &'static str, values: &'static [(&'static str, Data)]) -> Item {
    Item::RegKey { hive, path, values }
}

pub const ITEMS: &[Row] = &[
    row(
        "1",
        Role::RemovedByUninstaller,
        Item::File {
            root: Root::ProgramFiles,
            path: r"FexSonda\fexsonda.exe",
            content: Content::Itself,
        },
    ),
    row(
        "2",
        Role::Leftover,
        Item::Dir {
            root: Root::ProgramFiles,
            path: "FexSonda",
            files: &[
                ("uninstall.exe", Content::Itself),
                (
                    r"logs\install.log",
                    Content::Text("installed by fex-sonda\n"),
                ),
            ],
        },
    ),
    row(
        "3",
        Role::RemovedByUninstaller,
        reg(
            Hive::Hklm,
            r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\FexSonda",
            &[
                ("DisplayName", Data::Text("Fex Sonda")),
                ("DisplayVersion", Data::Text("1.0.0")),
                ("Publisher", Data::Text("Fexarium (prueba)")),
                ("InstallLocation", Data::Text("{F}")),
                ("DisplayIcon", Data::Text(r"{F}\fexsonda.exe")),
                ("UninstallString", Data::Text(r#""{F}\uninstall.exe""#)),
                ("EstimatedSize", Data::Dword(256)),
            ],
        ),
    ),
    row(
        "4",
        Role::Leftover,
        Item::Dir {
            root: Root::AppData,
            path: "FexSonda",
            files: &[("settings.ini", Content::Text("[fexsonda]\ntheme=dark\n"))],
        },
    ),
    row(
        "5",
        Role::Leftover,
        Item::Dir {
            root: Root::LocalAppData,
            path: "FexSonda",
            files: &[(r"cache\a.bin", Content::Zeros(64 * 1024))],
        },
    ),
    row(
        "6",
        Role::Leftover,
        Item::Dir {
            root: Root::ProgramData,
            path: "FexSonda",
            files: &[("shared.dat", Content::Text("shared by fex-sonda\n"))],
        },
    ),
    row(
        "7",
        Role::Leftover,
        reg(
            Hive::Hkcu,
            r"Software\FexSonda",
            &[
                ("LastRun", Data::Text("2026-11-24")),
                ("Path", Data::Text(r"{F}\fexsonda.exe")),
            ],
        ),
    ),
    row(
        "8",
        Role::Leftover,
        reg(
            Hive::Hklm,
            r"SOFTWARE\FexSonda",
            &[("InstallDir", Data::Text("{F}"))],
        ),
    ),
    row(
        "9",
        Role::Leftover,
        reg(
            Hive::Hklm,
            r"SOFTWARE\Classes\.fexsonda",
            &[("", Data::Text("FexSonda.Doc"))],
        ),
    ),
    row(
        "10",
        Role::Leftover,
        reg(
            Hive::Hklm,
            r"SOFTWARE\Classes\FexSonda.Doc\shell\open\command",
            &[("", Data::Text(r#""{F}\fexsonda.exe" "%1""#))],
        ),
    ),
    row(
        "11",
        Role::Leftover,
        Item::Lnk {
            root: Root::StartMenuCommon,
            path: "Fex Sonda.lnk",
            target: r"{F}\fexsonda.exe",
        },
    ),
    row(
        "12",
        Role::Leftover,
        Item::Task {
            name: r"\FexSonda\Update",
            command: r#""{F}\fexsonda.exe" --update"#,
        },
    ),
    // The case of VLC: an extension that was another program's, taken on install and given back
    // on uninstall. It is not Fex Sonda's, so Purifex must not propose it.
    // What `.fexcomun` belonged to before Fex Sonda took it: another program's class and file.
    row(
        "C1a",
        Role::Control,
        reg(
            Hive::Hklm,
            r"SOFTWARE\Classes\FexOtra.Doc\shell\open\command",
            &[("", Data::Text(r#""C:\ProgramData\FexOtra\otra.exe" "%1""#))],
        ),
    ),
    row(
        "C1b",
        Role::Control,
        Item::Dir {
            root: Root::ProgramData,
            path: "FexOtra",
            files: &[("otra.exe", Content::Empty)],
        },
    ),
    row(
        "C1",
        Role::Control,
        reg(
            Hive::Hklm,
            r"SOFTWARE\Classes\.fexcomun",
            &[("", Data::Text("FexOtra.Doc"))],
        ),
    ),
    // A neighbour that is installed and looks alike: registration, folder and settings.
    row(
        "C2",
        Role::Control,
        reg(
            Hive::Hklm,
            r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\FexSondaVecina",
            &[
                ("DisplayName", Data::Text("Fex Sonda Vecina")),
                (
                    "InstallLocation",
                    Data::Text(r"C:\Program Files\FexSondaVecina"),
                ),
                (
                    "UninstallString",
                    Data::Text(r#""C:\Program Files\FexSondaVecina\vecina.exe""#),
                ),
            ],
        ),
    ),
    row(
        "C2b",
        Role::Control,
        Item::Dir {
            root: Root::ProgramFiles,
            path: "FexSondaVecina",
            files: &[("vecina.exe", Content::Empty)],
        },
    ),
    row(
        "C2c",
        Role::Control,
        reg(
            Hive::Hkcu,
            r"Software\FexSondaVecina",
            &[("Seen", Data::Text("1"))],
        ),
    ),
];

/// What every path of this tool must hold before it is removed: only what this tool wrote.
const OURS: [&str; 3] = ["fexsonda", "fexotra", "fexcomun"];

/// Whether a path or key belongs to Fex Sonda or to its controls. `purge` refuses anything else.
pub fn is_ours(path: &str) -> bool {
    let lower = path.to_lowercase();
    OURS.iter().any(|name| lower.contains(name))
}

/// The row with this id.
pub fn by_id(id: &str) -> Option<&'static Row> {
    ITEMS.iter().find(|r| r.id == id)
}

/// Puts the install folder where a text says `{F}`.
pub fn expand(text: &str, install_dir: &str) -> String {
    text.replace("{F}", install_dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_row_has_its_own_id() {
        let mut ids: Vec<_> = ITEMS.iter().map(|r| r.id).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), ITEMS.len());
    }

    #[test]
    fn the_official_uninstaller_removes_only_the_program_and_its_entry() {
        let removed: Vec<_> = ITEMS
            .iter()
            .filter(|r| r.role == Role::RemovedByUninstaller)
            .map(|r| r.id)
            .collect();
        assert_eq!(removed, ["1", "3"]);
    }

    #[test]
    fn the_controls_are_the_extension_that_was_given_back_and_the_neighbour() {
        let controls: Vec<_> = ITEMS
            .iter()
            .filter(|r| r.role == Role::Control)
            .map(|r| r.id)
            .collect();
        assert_eq!(controls, ["C1a", "C1b", "C1", "C2", "C2b", "C2c"]);
    }

    #[test]
    fn nothing_in_the_list_is_named_like_something_real() {
        for row in ITEMS {
            let place = match row.item {
                Item::File { path, .. } | Item::Dir { path, .. } | Item::Lnk { path, .. } => path,
                Item::RegKey { path, .. } => path,
                Item::Task { name, .. } => name,
            };
            let shortcut = place == "Fex Sonda.lnk";
            assert!(shortcut || is_ours(place), "{}: {place}", row.id);
        }
    }

    #[test]
    fn only_what_is_named_like_the_tool_may_be_removed() {
        assert!(is_ours(r"C:\Program Files\FexSonda"));
        assert!(is_ours(r"HKLM\SOFTWARE\Classes\.fexcomun"));
        assert!(!is_ours(r"C:\Program Files\VideoLAN"));
        assert!(!is_ours(r"HKLM\SOFTWARE\Classes\.mp3"));
    }

    #[test]
    fn the_install_folder_is_filled_in() {
        assert_eq!(expand(r"{F}\a.exe", r"C:\X"), r"C:\X\a.exe");
    }
}
