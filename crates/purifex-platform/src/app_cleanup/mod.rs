//! What it takes to clean the traces of an uninstalled app on Windows: finding the
//! places that might hold them, starting the app's own uninstaller, and a
//! quarantine that can give back whatever was removed.

mod backup;
mod find_files;
mod find_registry;
mod finder;
mod launcher;
mod lnk;
mod quarantine;
mod registry_write;
mod seed_probe;

pub use backup::LocalBackups;
pub use finder::WindowsTraceFinder;
pub use launcher::RegisteredUninstaller;
pub use quarantine::LocalCleanup;
pub use seed_probe::WindowsSeedProbe;
