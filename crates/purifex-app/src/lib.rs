//! The services of Purifex. They work only against the ports of
//! `purifex-domain`, so the whole of them is tested with fakes and without
//! touching Windows. See `ARCHITECTURE.md`.

#![forbid(unsafe_code)]

pub mod apps_inventory;
pub mod archive;
pub mod candidates;
pub mod cleaning;
pub mod command;
mod command_apps;
pub mod core;
pub mod fixer;
mod help;
pub mod mcp;
pub mod monitoring;
pub mod offering;
pub mod outcome;
mod outcome_lists;
pub mod recorder;
pub mod scanner;
pub mod services;
#[cfg(test)]
mod testing;
mod uninstall_by_kind;
mod updates_wait;

pub use apps_inventory::AllApps;
pub use archive::Archive;
pub use candidates::Candidates;
pub use cleaning::Cleaning;
pub use command::{Command, Output, Screenshot, UiOptions};
pub use core::Core;
pub use fixer::Fixer;
pub use monitoring::Monitoring;
pub use offering::Offering;
pub use outcome::Outcome;
pub use recorder::{Recorder, WatchOptions};
pub use scanner::Scanner;
pub use services::{App, Services};
pub use uninstall_by_kind::UninstallByKind;
