//! The services of Purifex. They work only against the ports of
//! `purifex-domain`, so the whole of them is tested with fakes and without
//! touching Windows. See `ARCHITECTURE.md`.

pub mod command;
pub mod recorder;
pub mod scanner;
#[cfg(test)]
mod testing;

pub use command::{Command, Output};
pub use recorder::{Recorder, WatchOptions};
pub use scanner::Scanner;
