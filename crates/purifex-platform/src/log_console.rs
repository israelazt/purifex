//! The log of the program, for now to the standard error stream (the file on
//! disk comes with the interface).

use std::io::Write;

use purifex_domain::log::{set_sink, Level, LogSink};

struct Stderr;

impl LogSink for Stderr {
    fn write(&self, level: Level, message: &str) {
        let _ = writeln!(std::io::stderr(), "{:<5} {message}", level.label());
    }
}

/// Sends the log to the standard error stream.
pub fn install_stderr_log() {
    set_sink(Box::new(Stderr));
}
