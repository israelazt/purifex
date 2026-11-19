//! The services of Purifex. They work only against the ports of
//! `purifex-domain`, so the whole of them is tested with fakes and without
//! touching Windows. See `ARCHITECTURE.md`.

#![forbid(unsafe_code)]

pub mod archive;
pub mod command;
pub mod core;
pub mod fixer;
pub mod offering;
pub mod outcome;
pub mod recorder;
pub mod scanner;
pub mod services;
#[cfg(test)]
mod testing;
mod updates_wait;

pub use archive::Archive;
pub use command::{Command, Output, Screenshot, UiOptions};
pub use core::Core;
pub use fixer::Fixer;
pub use offering::Offering;
pub use outcome::Outcome;
pub use recorder::{Recorder, WatchOptions};
pub use scanner::Scanner;
pub use services::{App, Services};
