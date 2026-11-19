//! What it takes to clean the traces of an uninstalled app on Windows: finding the
//! places that might hold them, starting the app's own uninstaller, and a
//! quarantine that can give back whatever was removed.

mod finder;
mod launcher;
mod quarantine;
// Used by the quarantine from the next step on.
#[allow(dead_code)]
mod registry_write;

pub use finder::WindowsTraceFinder;
pub use launcher::RegisteredUninstaller;
pub use quarantine::LocalCleanup;
