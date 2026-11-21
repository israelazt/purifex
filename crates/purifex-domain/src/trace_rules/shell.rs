//! The rules for what an app registers to be part of the shell: its COM servers and the
//! entries of the context menu. Both are the app's only when the program they run or load
//! (read by the finder into `text`) is inside its folder, and only in the fixed places.

use super::{Ctx, Found};
use crate::app_traces::Evidence;
use crate::reference::path_refers_to;

/// The keys with one child per COM class.
const CLSID_KEYS: [&str; 3] = [
    r"hklm\software\classes\clsid",
    r"hklm\software\wow6432node\classes\clsid",
    r"hkcu\software\classes\clsid",
];

/// The keys with one child per type library.
const TYPELIB_KEYS: [&str; 3] = [
    r"hklm\software\classes\typelib",
    r"hklm\software\wow6432node\classes\typelib",
    r"hkcu\software\classes\typelib",
];

/// Where the verbs (`shell`) and the handlers (`shellex\ContextMenuHandlers`) of the context
/// menu hang, below `Classes` of the machine or of the user.
const MENU_PLACES: [&str; 10] = [
    r"\*\shell",
    r"\directory\shell",
    r"\directory\background\shell",
    r"\drive\shell",
    r"\allfilesystemobjects\shell",
    r"\*\shellex\contextmenuhandlers",
    r"\directory\shellex\contextmenuhandlers",
    r"\directory\background\shellex\contextmenuhandlers",
    r"\drive\shellex\contextmenuhandlers",
    r"\allfilesystemobjects\shellex\contextmenuhandlers",
];

/// A COM class whose server (a `.dll` or `.exe` the finder read) is inside the app's folder.
pub(super) fn com_key(c: &Ctx) -> Option<Found> {
    if !CLSID_KEYS.contains(&c.parent) {
        return None;
    }
    runs_inside_folder(
        c,
        Evidence::ComServer,
        "clase COM cuyo servidor está en la carpeta de la app",
    )
}

/// A type library whose files (the finder read them, all of them, into `text`) are inside the
/// app's folder.
pub(super) fn typelib_key(c: &Ctx) -> Option<Found> {
    if !TYPELIB_KEYS.contains(&c.parent) {
        return None;
    }
    runs_inside_folder(
        c,
        Evidence::ComServer,
        "biblioteca de tipos cuyo archivo está en la carpeta de la app",
    )
}

/// A verb or a handler of the context menu whose program is inside the app's folder.
pub(super) fn context_menu_key(c: &Ctx) -> Option<Found> {
    let place = [r"hklm\software\classes", r"hkcu\software\classes"]
        .iter()
        .find_map(|hive| c.parent.strip_prefix(hive))?;
    if !MENU_PLACES.contains(&place) {
        return None;
    }
    runs_inside_folder(
        c,
        Evidence::ContextMenu,
        "entrada del menú contextual que ejecuta un programa de la app",
    )
}

fn runs_inside_folder(c: &Ctx, evidence: Evidence, what: &str) -> Option<Found> {
    let folder = c.seeds.folder.as_deref()?;
    let text = c.candidate.text.as_deref()?;
    path_refers_to(text, folder).then(|| (evidence, format!("{what}: {}", c.name)))
}
