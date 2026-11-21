//! The rules for what Windows runs on its own: services and scheduled tasks. Both are the
//! app's only when what they run is inside its folder.

use super::{Ctx, Found};
use crate::app_traces::Evidence;
use crate::reference::path_refers_to;

/// The key where Windows keeps its services, one child per service.
const SERVICES_KEY: &str = r"hklm\system\currentcontrolset\services";

/// A service whose program (`ImagePath`, read by the finder into `text`) is inside the
/// app's folder: what an uninstaller that did not finish leaves registered, and Windows
/// keeps trying to start. Only the direct children of the services key, and only through
/// the folder: a name alone proves nothing here.
pub(super) fn service_key(c: &Ctx) -> Option<Found> {
    if c.parent != SERVICES_KEY {
        return None;
    }
    let folder = c.seeds.folder.as_deref()?;
    let text = c.candidate.text.as_deref()?;
    path_refers_to(text, folder).then(|| {
        (
            Evidence::ServicePath,
            format!(
                "servicio de Windows cuyo programa está en la carpeta de la app: {}",
                c.name
            ),
        )
    })
}

/// A scheduled task whose action (`text`: the commands it runs, read by the finder) runs a
/// program inside the app's folder: the updater or the starter that an uninstaller left.
pub(super) fn task_action(c: &Ctx) -> Option<Found> {
    let folder = c.seeds.folder.as_deref()?;
    let text = c.candidate.text.as_deref()?;
    path_refers_to(text, folder).then(|| {
        (
            Evidence::TaskAction,
            format!(
                "tarea programada que ejecuta un programa de la app: {}",
                c.name
            ),
        )
    })
}
