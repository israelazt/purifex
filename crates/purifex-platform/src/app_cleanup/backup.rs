//! The copy of what an app registered, taken before its uninstaller runs:
//! `%LOCALAPPDATA%\Purifex\backups\<time>-<app>\registro.reg`. Importing that file registers
//! the app again, so Windows lists it and its own uninstaller can be run once more.

use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use purifex_domain::apps::InstalledApp;
use purifex_domain::ports::RegistryBackup;

use super::quarantine::reg;

pub struct LocalRegistryBackup {
    base: Option<PathBuf>,
}

impl LocalRegistryBackup {
    pub fn new() -> Self {
        Self {
            base: std::env::var_os("LOCALAPPDATA").map(|b| PathBuf::from(b).join("Purifex")),
        }
    }
}

impl LocalRegistryBackup {
    #[cfg(test)]
    fn in_dir(base: PathBuf) -> Self {
        Self { base: Some(base) }
    }
}

impl Default for LocalRegistryBackup {
    fn default() -> Self {
        Self::new()
    }
}

/// A name that is safe as part of a folder name.
fn slug(name: &str) -> String {
    let clean: String = name
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .take(40)
        .collect();
    clean.trim_matches('-').to_string()
}

impl RegistryBackup for LocalRegistryBackup {
    fn save(&self, app: &InstalledApp) -> Result<Option<String>, String> {
        let Some(key) = app.registry_key() else {
            return Ok(None);
        };
        let base = self
            .base
            .as_ref()
            .ok_or("no se encuentra la carpeta de datos del usuario")?;
        let seconds = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        let folder = base
            .join("backups")
            .join(format!("{seconds}-{}", slug(&app.name)));
        fs::create_dir_all(&folder).map_err(|e| format!("no se pudo crear la copia: {e}"))?;
        let file = folder.join("registro.reg").to_string_lossy().into_owned();
        reg(&["export", &key, &file, "/y"])?;
        Ok(Some(file))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_name_of_the_folder_has_only_safe_characters() {
        assert_eq!(slug("OBS Studio (64 bit)"), "OBS-Studio--64-bit");
        assert_eq!(slug(r"..\..\Windows"), "Windows");
    }

    #[test]
    fn the_registration_of_a_real_key_is_exported_to_a_file_that_names_it() {
        // A key made for this test, in the user's own part of the registry.
        let sub = format!("PurifexBackupTest{}", std::process::id());
        let key = format!(r"HKCU\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\{sub}");
        reg(&["add", &key, "/v", "DisplayName", "/d", "Prueba", "/f"]).unwrap();
        let dir = std::env::temp_dir().join(format!("purifex-backup-{}", std::process::id()));
        let app = InstalledApp {
            id: format!("HKCU:{sub}"),
            name: "Prueba".into(),
            ..Default::default()
        };
        let saved = LocalRegistryBackup::in_dir(dir.clone()).save(&app);
        let _ = reg(&["delete", &key, "/f"]);
        let file = saved.unwrap().unwrap();
        let bytes = fs::read(&file).unwrap();
        // `reg export` writes UTF-16: the name of the key is in it.
        let units: Vec<u16> = bytes
            .as_chunks::<2>()
            .0
            .iter()
            .map(|b| u16::from_le_bytes(*b))
            .collect();
        assert!(String::from_utf16_lossy(&units).contains(&sub));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn an_app_without_a_key_has_nothing_to_save() {
        let store_app = InstalledApp {
            id: "Microsoft.Paint_1_x64".into(),
            kind: purifex_domain::apps::AppKind::Store,
            ..Default::default()
        };
        assert_eq!(LocalRegistryBackup::new().save(&store_app), Ok(None));
    }
}
