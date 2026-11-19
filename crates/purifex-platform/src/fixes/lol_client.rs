//! The setting "close the League client during the game": a line of
//! `LCULocalPreferences.yaml`, the file where the client keeps the options of the
//! user. It is changed with care: only that line (the editor of the domain keeps
//! the rest byte for byte, line ends included), a copy of the file first, the
//! new file written beside it and moved over the old one in one step, and never
//! while the client is open (it rewrites the file when it exits and would undo
//! the change).

use std::fs;
use std::path::{Path, PathBuf};

use purifex_domain::fix::{league_folder, SettingValue};
use purifex_domain::yaml_key;

const KEY: [&str; 3] = ["lol-user-experience", "data", "closeLeagueClientDuringGame"];
/// Where Riot writes what is installed.
const INSTALLS: &str = r"C:\ProgramData\Riot Games\RiotClientInstalls.json";
/// Where League goes by default.
const DEFAULT_FOLDER: &str = r"C:\Riot Games\League of Legends";
const FILE: &[&str] = &["Config", "LCULocalPreferences.yaml"];
/// The programs of the client: while any runs, the file is not touched.
const CLIENT_PROGRAMS: [&str; 3] = [
    "leagueclient.exe",
    "leagueclientux.exe",
    "leagueclientuxrender.exe",
];

type RunningPrograms = Box<dyn Fn() -> Result<Vec<String>, String>>;

pub struct LolClient {
    config: PathBuf,
    backups: PathBuf,
    running: RunningPrograms,
}

fn config_of(folder: &str) -> PathBuf {
    let mut path = PathBuf::from(folder.replace('/', "\\"));
    path.extend(FILE);
    path
}

impl LolClient {
    /// Where League is installed: what Riot says, or the default folder.
    pub fn new(backups: PathBuf) -> Self {
        let folder = fs::read_to_string(INSTALLS)
            .ok()
            .and_then(|text| league_folder(&text))
            .unwrap_or_else(|| DEFAULT_FOLDER.to_string());
        Self {
            config: config_of(&folder),
            backups,
            running: Box::new(crate::running_programs),
        }
    }

    /// With the file, the folder of the copies and the list of running programs given.
    #[cfg(test)]
    pub fn with(config: PathBuf, backups: PathBuf, running: RunningPrograms) -> Self {
        Self {
            config,
            backups,
            running,
        }
    }

    fn read(&self) -> Result<String, String> {
        fs::read_to_string(&self.config).map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                format!(
                    "no encuentro la configuración de League en {}",
                    self.config.display()
                )
            } else {
                format!("no se pudo leer {}: {e}", self.config.display())
            }
        })
    }

    pub fn current(&self) -> Result<SettingValue, String> {
        Ok(match yaml_key::get(&self.read()?, &KEY).as_deref() {
            None => SettingValue::Unset,
            Some("true") => SettingValue::Bool(true),
            Some("false") => SettingValue::Bool(false),
            Some(other) => SettingValue::Text(other.to_string()),
        })
    }

    pub fn blockers(&self) -> Result<Vec<String>, String> {
        let open = (self.running)()?
            .iter()
            .any(|name| CLIENT_PROGRAMS.contains(&name.to_lowercase().as_str()));
        Ok(if open {
            vec!["Cierra el cliente de League: reescribe esta configuración al salir y deshaciría el cambio".to_string()]
        } else {
            Vec::new()
        })
    }

    /// Writes the value. A copy of the file goes to the folder of copies first.
    pub fn set(&self, value: &SettingValue) -> Result<(), String> {
        let text = match value {
            SettingValue::Bool(true) => "true",
            // Not writing the setting is the same as the default, which is off.
            SettingValue::Bool(false) | SettingValue::Unset => "false",
            SettingValue::Text(_) => return Err("este ajuste solo admite sí o no".into()),
        };
        let original = self.read()?;
        let changed = yaml_key::set(&original, &KEY, text)?;
        fs::create_dir_all(&self.backups)
            .map_err(|e| format!("no se pudo crear la carpeta de copias: {e}"))?;
        let name = self
            .config
            .file_name()
            .map_or_else(|| "config".into(), |n| n.to_string_lossy().into_owned());
        let backup = self
            .backups
            .join(format!("{}-{name}", crate::clock::local_stamp()));
        fs::write(&backup, &original)
            .map_err(|e| format!("no se pudo guardar la copia {}: {e}", backup.display()))?;
        write_beside_and_move(&self.config, &changed)
    }
}

/// The new content is written next to the file and moved over it in one step, so
/// the file is never half written.
fn write_beside_and_move(target: &Path, content: &str) -> Result<(), String> {
    let beside = target.with_extension("yaml.purifex-new");
    fs::write(&beside, content)
        .map_err(|e| format!("no se pudo escribir {}: {e}", beside.display()))?;
    fs::rename(&beside, target).map_err(|e| {
        let _ = fs::remove_file(&beside);
        format!("no se pudo reemplazar {}: {e}", target.display())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILE_TEXT: &str = "lol-audio:\r\n    data:\r\n        masterVolume: 21\r\n    schemaVersion: 5\r\nlol-user-experience:\r\n    data:\r\n        closeLeagueClientDuringGame: false\r\n        hasBeenPromptedForPotatoMode: true\r\n    schemaVersion: 3\r\n";

    fn folder(name: &str) -> PathBuf {
        let folder = std::env::temp_dir().join(format!("purifex-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&folder);
        fs::create_dir_all(&folder).unwrap();
        folder
    }

    fn client(dir: &Path, running: &'static [&'static str]) -> LolClient {
        let config = dir.join("LCULocalPreferences.yaml");
        fs::write(&config, FILE_TEXT).unwrap();
        LolClient::with(
            config,
            dir.join("copias"),
            Box::new(move || Ok(running.iter().map(|s| s.to_string()).collect())),
        )
    }

    #[test]
    fn it_reads_the_setting_and_a_missing_key_is_unset() {
        let dir = folder("lol-read");
        let c = client(&dir, &[]);
        assert_eq!(c.current().unwrap(), SettingValue::Bool(false));
        fs::write(
            &c.config,
            FILE_TEXT.replace("        closeLeagueClientDuringGame: false\r\n", ""),
        )
        .unwrap();
        assert_eq!(c.current().unwrap(), SettingValue::Unset);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn a_missing_file_says_where_it_looked() {
        let dir = folder("lol-missing");
        let c = LolClient::with(
            dir.join("nada.yaml"),
            dir.join("copias"),
            Box::new(|| Ok(vec![])),
        );
        assert!(c
            .current()
            .unwrap_err()
            .contains("no encuentro la configuración de League"));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn writing_changes_only_that_line_keeps_the_crlf_and_leaves_a_copy_of_the_original() {
        let dir = folder("lol-write");
        let c = client(&dir, &[]);
        c.set(&SettingValue::Bool(true)).unwrap();
        let now = fs::read_to_string(&c.config).unwrap();
        assert_eq!(
            now,
            FILE_TEXT.replace(
                "closeLeagueClientDuringGame: false",
                "closeLeagueClientDuringGame: true"
            )
        );
        assert_eq!(c.current().unwrap(), SettingValue::Bool(true));
        let copies: Vec<_> = fs::read_dir(dir.join("copias"))
            .unwrap()
            .flatten()
            .collect();
        assert_eq!(copies.len(), 1);
        assert_eq!(
            fs::read_to_string(copies[0].path()).unwrap(),
            FILE_TEXT,
            "the copy is the file as it was"
        );
        assert!(
            !dir.join("LCULocalPreferences.yaml.purifex-new").exists(),
            "no leftover"
        );
        // And back to the original, exactly.
        c.set(&SettingValue::Bool(false)).unwrap();
        assert_eq!(fs::read_to_string(&c.config).unwrap(), FILE_TEXT);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn an_unset_value_goes_back_as_the_default_and_a_text_is_refused() {
        let dir = folder("lol-unset");
        let c = client(&dir, &[]);
        c.set(&SettingValue::Bool(true)).unwrap();
        c.set(&SettingValue::Unset).unwrap();
        assert_eq!(c.current().unwrap(), SettingValue::Bool(false));
        assert!(c.set(&SettingValue::Text("x".into())).is_err());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn while_the_client_is_open_the_file_is_blocked_whatever_the_case_of_its_name() {
        let dir = folder("lol-blocked");
        assert!(client(&dir, &["explorer.exe", "LeagueClientUx.exe"])
            .blockers()
            .unwrap()[0]
            .contains("Cierra el cliente de League"));
        assert!(
            client(&dir, &["LEAGUECLIENT.EXE"])
                .blockers()
                .unwrap()
                .len()
                == 1
        );
        assert!(
            client(&dir, &["League of Legends.exe", "vgc.exe"])
                .blockers()
                .unwrap()
                .is_empty(),
            "the game itself does not block it"
        );
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn the_folder_comes_from_what_riot_says_with_forward_slashes() {
        assert_eq!(
            config_of("C:/Riot Games/League of Legends/"),
            PathBuf::from(r"C:\Riot Games\League of Legends\Config\LCULocalPreferences.yaml")
        );
    }
}
