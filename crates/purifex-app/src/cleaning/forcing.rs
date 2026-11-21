//! A forced uninstall: for an app whose own uninstaller fails or is gone. Its uninstaller is not
//! started. What Windows knows of it (its entry in `Uninstall`) goes to the quarantine, after the
//! same safety net as a normal uninstall (a restore point, a copy of its registration); with the
//! entry gone the app counts as removed, and what it left is looked for and shown like any other.

use purifex_domain::app_traces::{Evidence, Trace, TraceKind};
use purifex_domain::apps::InstalledApp;
use purifex_domain::safeguard::Started;

use super::{find, Cleaning};

impl Cleaning {
    /// Forces the removal of an installed app. Protected apps are refused, and so is one that
    /// has no entry of its own to take out (an app of the Store).
    pub fn force_uninstall(
        &mut self,
        apps: &[InstalledApp],
        query: &str,
    ) -> Result<Started, String> {
        let mut app = find(apps, query)?.clone();
        if let Some(why) = app.protection() {
            return Err(format!("{} no se quita desde aquí: {why}", app.name));
        }
        let Some(key) = app.registry_key() else {
            return Err(format!(
                "{} no tiene entrada propia en el registro: usa su desinstalador",
                app.name
            ));
        };
        // Observed and remembered first, as for a normal uninstall: once its entry is gone
        // the system forgets the app, and what it left must still be looked for.
        if let Some(probe) = &self.probe {
            app.observed = probe.observe(&app);
        }
        self.store()?.remember(&app)?;
        let restore_point = self.restore_point_for(&app);
        let registry_copy = self.registry_copy_of(&app);
        let entry = Trace {
            kind: TraceKind::RegistryKey,
            location: key,
            why: "la entrada con la que la app se registró en Windows".into(),
            evidence: Evidence::RegisteredInstall,
            ..Default::default()
        };
        let done = self.remove_all(&app.name, std::slice::from_ref(&entry))?;
        if let Some((_, why)) = done.failed.into_iter().next() {
            return Err(format!("no se pudo quitar su entrada del registro: {why}"));
        }
        Ok(Started {
            app,
            restore_point,
            registry_copy,
        })
    }
}
