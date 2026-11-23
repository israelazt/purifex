//! What it takes to clean the traces of an uninstalled app on Windows: finding the
//! places that might hold them, starting the app's own uninstaller, and a
//! quarantine that can give back whatever was removed.

mod backup;
#[cfg(test)]
mod end_to_end;
mod find_files;
mod find_firewall;
mod find_registry;
mod find_services;
mod find_shell;
mod find_typelib;
mod finder;
pub(crate) use find_services::expand;
pub(crate) use finder::{folder_size, roots};
pub(crate) use lnk::target as lnk_target;
pub(crate) use tasks::{set_enabled as set_task_enabled, startup_tasks};
mod launcher;
mod lnk;
mod locks;
mod profiles;
mod quarantine;
mod reboot;
pub(crate) mod registry_write;
mod seed_probe;
mod tasks;

pub use backup::LocalBackups;
pub use finder::WindowsTraceFinder;
pub use launcher::RegisteredUninstaller;
pub use quarantine::LocalCleanup;
pub use seed_probe::WindowsSeedProbe;
