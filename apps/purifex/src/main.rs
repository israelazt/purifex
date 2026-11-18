//! Purifex: the program. It creates the real adapters, hands them to the
//! application and prints what it returns. The decisions are not here.
//! See `ARCHITECTURE.md`.

/// Writes a line to the standard output. A closed pipe (`| head`) is not an error.
#[cfg(windows)]
fn say(line: &str) {
    use std::io::Write;
    let _ = writeln!(std::io::stdout(), "{line}");
}

#[cfg(windows)]
fn main() {
    use purifex_app::{Command, Recorder, Scanner};
    use purifex_platform::{
        install_stderr_log, LocalSessionFile, SetupApiInventory, SystemClock, WindowsSampler,
    };

    install_stderr_log();
    let args: Vec<String> = std::env::args().skip(1).collect();
    let command = match Command::parse(&args) {
        Ok(command) => command,
        Err(error) => {
            purifex_domain::log_error!("{error}");
            std::process::exit(2);
        }
    };
    let scanner = Scanner::new(SetupApiInventory, SystemClock::new());
    // The game's program, as it appears in the process list.
    let sampler = WindowsSampler::new(&["League of Legends.exe"]);
    let mut recorder = Recorder::new(sampler, SystemClock::new(), LocalSessionFile::default());
    let output = command.run(&scanner, &mut recorder, &mut |line| {
        say(line);
    });
    say(&output.text);
    std::process::exit(if output.ok { 0 } else { 1 });
}

/// Purifex only has its system adapters for Windows so far.
#[cfg(not(windows))]
fn main() {
    purifex_domain::log_error!("Purifex does not have an adapter for this system yet.");
    std::process::exit(1);
}
