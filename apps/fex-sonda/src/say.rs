//! The only place that talks to the console: a test tool whose output is its report.

pub fn line(text: &str) {
    println!("{text}");
}

pub fn error(text: &str) {
    eprintln!("{text}");
}
