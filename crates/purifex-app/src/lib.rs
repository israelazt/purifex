//! The services of Purifex. They work only against the ports of
//! `purifex-domain`, so the whole of them is tested with fakes and without
//! touching Windows. See `ARCHITECTURE.md`.

#![forbid(unsafe_code)]

pub mod archive;
pub mod candidates;
pub mod command;
pub mod core;
pub mod fixer;
mod help;
pub mod mcp;
pub mod offering;
pub mod outcome;
pub mod recorder;
pub mod scanner;
pub mod services;
#[cfg(test)]
mod testing;
mod updates_wait;

pub use archive::Archive;
pub use candidates::Candidates;
pub use command::{Command, Output, Screenshot, UiOptions};
pub use core::Core;
pub use fixer::Fixer;
pub use offering::Offering;
pub use outcome::Outcome;
pub use recorder::{Recorder, WatchOptions};
pub use scanner::Scanner;
pub use services::{App, Services};
