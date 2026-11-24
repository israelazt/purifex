//! What the startup page asks for, over the port that reads and changes it.

use super::*;

impl<I, C, S, C2, F, A> App<I, C, S, C2, F, A> {
    pub(super) fn read_startup(&mut self) -> Result<Vec<StartupItem>, String> {
        let source = self
            .startup
            .as_ref()
            .ok_or("esta versión no lee el arranque")?;
        let apps = self
            .installed()
            .map(|(listed, _)| listed)
            .unwrap_or_default();
        let items = source
            .read()?
            .into_iter()
            .map(|i| i.with_app(&apps))
            .collect();
        Ok(purifex_domain::startup::sorted(items))
    }

    pub(super) fn read_startup_quick(&mut self) -> Result<Vec<StartupItem>, String> {
        let source = self
            .startup
            .as_ref()
            .ok_or("esta versión no lee el arranque")?;
        let apps = self
            .installed()
            .map(|(listed, _)| listed)
            .unwrap_or_default();
        Ok(source
            .read_quick()?
            .into_iter()
            .map(|i| i.with_app(&apps))
            .collect())
    }

    pub(super) fn change_startup(&mut self, id: &str, on: bool) -> Result<StartupItem, String> {
        let source = self
            .startup
            .as_ref()
            .ok_or("esta versión no cambia el arranque")?;
        let item = source
            .read()?
            .into_iter()
            .find(|i| i.id == id)
            .ok_or("esa entrada ya no está en el arranque")?;
        source.set_enabled(&item, on)?;
        Ok(StartupItem {
            enabled: on,
            ..item
        })
    }

    pub(super) fn take_out_startup(&mut self, id: &str) -> Result<CleanupEntry, String> {
        let source = self
            .startup
            .as_ref()
            .ok_or("esta versión no cambia el arranque")?;
        let item = source
            .read()?
            .into_iter()
            .find(|i| i.id == id)
            .ok_or("esa entrada ya no está en el arranque")?;
        let trace = item.trace().ok_or("esta entrada no se quita desde aquí")?;
        self.cleaning.remove_startup(&item.name, &trace)
    }
}
