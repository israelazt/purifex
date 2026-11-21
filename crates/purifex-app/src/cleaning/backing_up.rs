//! The copies of the registration of the apps: taken before their uninstaller runs, listed
//! afterwards, and imported again when the person wants the app registered back.

use purifex_domain::apps::InstalledApp;
use purifex_domain::ports::Backups;
use purifex_domain::safeguard::BackupEntry;

use super::Cleaning;

impl Cleaning {
    /// The registration of each app is copied before its uninstaller runs.
    pub fn with_backups(mut self, backups: Box<dyn Backups>) -> Self {
        self.backups = Some(backups);
        self
    }

    /// Where the copy of the app's registration went. A copy that cannot be made is logged
    /// and does not stop the uninstall: the person can still cancel the uninstaller itself.
    pub(super) fn registry_copy_of(&self, app: &InstalledApp) -> Option<String> {
        let backups = self.backups.as_ref()?;
        let saved = match backups.save(app) {
            Ok(Some(path)) => path,
            Ok(None) => return None,
            Err(why) => {
                purifex_domain::log_warn!("{}: no se copió su registro: {why}", app.name);
                return None;
            }
        };
        let entry = BackupEntry {
            app: app.name.clone(),
            key: app.registry_key().unwrap_or_default(),
            registry_copy: saved.clone(),
            on: self.today(),
        };
        if let Err(why) = backups.record(&entry) {
            purifex_domain::log_warn!("{}: no se anotó su copia: {why}", app.name);
        }
        Some(saved)
    }

    /// The copies taken so far, newest first.
    pub fn backups(&self) -> Result<Vec<BackupEntry>, String> {
        let mut entries = self
            .backups
            .as_ref()
            .map_or(Ok(Vec::new()), |b| b.entries())?;
        entries.reverse();
        Ok(entries)
    }

    /// Registers the app again from the copy taken before its uninstaller ran.
    pub fn restore_backup(&self, registry_copy: &str) -> Result<BackupEntry, String> {
        let backups = self
            .backups
            .as_ref()
            .ok_or("esta versión no guarda copias del registro")?;
        let entry = backups
            .entries()?
            .into_iter()
            .rfind(|e| e.registry_copy == registry_copy)
            .ok_or("esa copia no está en la lista")?;
        backups.restore(&entry)?;
        Ok(entry)
    }
}
