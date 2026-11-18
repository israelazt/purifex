//! The services of Purifex. They work only against the ports of
//! `purifex-domain`, so the whole of them is tested with fakes and without
//! touching Windows. See `ARCHITECTURE.md`.

#![forbid(unsafe_code)]

pub mod command;
pub mod outcome;
pub mod recorder;
pub mod scanner;
pub mod services;
#[cfg(test)]
mod testing;

pub use command::{Command, Output};
pub use outcome::Outcome;
pub use recorder::{Recorder, WatchOptions};
pub use scanner::Scanner;
pub use services::{App, Services};
