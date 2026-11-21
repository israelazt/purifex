//! The copy of what an app registered, taken before its uninstaller runs:
//! `%LOCALAPPDATA%\Purifex\backups\<time>-<app>\registro.reg`. Importing that file registers
//! the app again, so Windows lists it and its own uninstaller can be run once more.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use purifex_domain::apps::InstalledApp;
use purifex_domain::ports::Backups;
use purifex_domain::safeguard::BackupEntry;

use super::quarantine::reg;

pub struct LocalBackups {
    base: Option<PathBuf>,
}

impl LocalBackups {
    pub fn new() -> Self {
        Self {
            base: std::env::var_os("LOCALAPPDATA").map(|b| PathBuf::from(b).join("Purifex")),
        }
    }
}

impl LocalBackups {
    #[cfg(test)]
    fn in_dir(base: PathBuf) -> Self {
        Self { base: Some(base) }
    }
}

impl Default for LocalBackups {
    fn default() -> Self {
        Self::new()
    }
}

impl LocalBackups {
    fn log(&self) -> Result<PathBuf, String> {
        let base = self
            .base
            .as_ref()
            .ok_or("no se encuentra la carpeta de datos del usuario")?;
        Ok(base.join("backups.jsonl"))
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

impl Backups for LocalBackups {
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

    fn record(&self, entry: &BackupEntry) -> Result<(), String> {
        let path = self.log()?;
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .map_err(|e| format!("no se pudo abrir {}: {e}", path.display()))?;
        writeln!(file, "{}", entry.to_line()).map_err(|e| format!("no se pudo escribir: {e}"))
    }

    fn entries(&self) -> Result<Vec<BackupEntry>, String> {
        // A missing file is no copies; a line that does not read is skipped, because the
        // list is only a convenience to find the copies.
        let text = fs::read_to_string(self.log()?).unwrap_or_default();
        Ok(text
            .lines()
            .filter_map(|l| BackupEntry::from_line(l).ok())
            .collect())
    }

    fn restore(&self, entry: &BackupEntry) -> Result<(), String> {
        // Only a copy this program made is imported: it must be one of its own files.
        let known = self.entries()?.iter().any(|e| e == entry);
        if !known {
            return Err("esa copia no es de Purifex".into());
        }
        if reg(&["query", &entry.key]).is_ok() {
            return Err("la app ya está registrada: no se mezcla con la copia".into());
        }
        reg(&["import", &entry.registry_copy])
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;

    /// `reg export` fails now and then when several run at once (seen on the CI): the tests
    /// that use the real registry take turns.
    static REAL_REGISTRY: Mutex<()> = Mutex::new(());

    #[test]
    fn the_name_of_the_folder_has_only_safe_characters() {
        assert_eq!(slug("OBS Studio (64 bit)"), "OBS-Studio--64-bit");
        assert_eq!(slug(r"..\..\Windows"), "Windows");
    }

    #[test]
    fn the_registration_of_a_real_key_is_exported_to_a_file_that_names_it() {
        let _turn = REAL_REGISTRY.lock().unwrap_or_else(|e| e.into_inner());
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
        let saved = LocalBackups::in_dir(dir.clone()).save(&app);
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
    fn a_copy_registers_the_app_again_but_never_over_one_that_exists() {
        let _turn = REAL_REGISTRY.lock().unwrap_or_else(|e| e.into_inner());
        let sub = format!("PurifexRestoreTest{}", std::process::id());
        let key = format!(r"HKCU\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\{sub}");
        reg(&["add", &key, "/v", "DisplayName", "/d", "Prueba", "/f"]).unwrap();
        let dir = std::env::temp_dir().join(format!("purifex-restore-{}", std::process::id()));
        let backups = LocalBackups::in_dir(dir.clone());
        let app = InstalledApp {
            id: format!("HKCU:{sub}"),
            name: "Prueba".into(),
            ..Default::default()
        };
        let entry = BackupEntry {
            app: app.name.clone(),
            key: key.clone(),
            registry_copy: backups.save(&app).unwrap().unwrap(),
            on: None,
        };
        backups.record(&entry).unwrap();
        assert_eq!(backups.entries().unwrap(), std::slice::from_ref(&entry));
        // Registered: nothing is merged.
        assert!(backups
            .restore(&entry)
            .unwrap_err()
            .contains("ya está registrada"));
        // Gone (the uninstaller ran): the copy brings it back.
        reg(&["delete", &key, "/f"]).unwrap();
        let back = backups.restore(&entry);
        let registered = reg(&["query", &key]).is_ok();
        let _ = reg(&["delete", &key, "/f"]);
        let _ = fs::remove_dir_all(dir);
        back.unwrap();
        assert!(registered);
    }

    #[test]
    fn a_file_that_the_log_does_not_know_is_never_imported() {
        let dir = std::env::temp_dir().join(format!("purifex-unknown-{}", std::process::id()));
        let stranger = BackupEntry {
            app: "X".into(),
            key: r"HKCU\SOFTWARE\Purifex\NoExiste".into(),
            registry_copy: r"C:\Windows\evil.reg".into(),
            on: None,
        };
        let error = LocalBackups::in_dir(dir).restore(&stranger).unwrap_err();
        assert!(error.contains("no es de Purifex"));
    }

    #[test]
    fn an_app_without_a_key_has_nothing_to_save() {
        let store_app = InstalledApp {
            id: "Microsoft.Paint_1_x64".into(),
            kind: purifex_domain::apps::AppKind::Store,
            ..Default::default()
        };
        assert_eq!(LocalBackups::new().save(&store_app), Ok(None));
    }
}
