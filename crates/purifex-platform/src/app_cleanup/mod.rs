//! What it takes to clean the traces of an uninstalled app on Windows: finding the
//! places that might hold them, starting the app's own uninstaller, and a
//! quarantine that can give back whatever was removed.

mod find_files;
mod finder;
mod launcher;
mod lnk;
mod quarantine;
mod registry_write;
mod seed_probe;

pub use finder::WindowsTraceFinder;
pub use launcher::RegisteredUninstaller;
pub use quarantine::LocalCleanup;
pub use seed_probe::WindowsSeedProbe;
