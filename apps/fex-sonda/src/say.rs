//! The only place that talks to the console: a test tool whose output is its report.

// Used from `status` (T.4) on: remove this then.
#[allow(dead_code)]
pub fn line(text: &str) {
    println!("{text}");
}

pub fn error(text: &str) {
    eprintln!("{text}");
}
