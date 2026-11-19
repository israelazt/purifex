//! Purifex: the program. It creates the real adapters, hands them to the
//! application and prints what it returns. The decisions are not here.
//! See `ARCHITECTURE.md`.

/// Writes a line to the standard output. A closed pipe (`| head`) is not an error.
#[cfg(windows)]
mod shell;

#[cfg(windows)]
fn say(line: &str) {
    use std::io::Write;
    let _ = writeln!(std::io::stdout(), "{line}");
}

#[cfg(windows)]
fn main() {
    use purifex_app::{App, Archive, Candidates, Command, Fixer, Offering, Recorder, Scanner};
    use purifex_platform::{
        install_stderr_log, JsonlCandidates, JsonlHistory, LocalSessionArchive, LocalSessionFile,
        SetupApiInventory, SystemClock, SystemFixes, WindowsSampler, WindowsUpdateOffers,
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
    let recorder = Recorder::new(sampler, SystemClock::new(), LocalSessionFile::default());
    let archive = Archive::new(LocalSessionArchive::new());
    let fixer = Fixer::new(Box::new(SystemFixes::new()), Box::new(JsonlHistory::new()));
    let offering = Offering::new(std::sync::Arc::new(WindowsUpdateOffers));
    let mut app = App::new(scanner, recorder, archive)
        .with_fixer(fixer)
        .with_offers(offering)
        .with_candidates(Candidates::new(Box::new(JsonlCandidates::new())));
    if matches!(command, Command::Mcp) {
        let stdin = std::io::stdin();
        if let Err(error) = purifex_app::mcp::serve(&mut app, stdin.lock(), std::io::stdout()) {
            purifex_domain::log_error!("mcp: {error}");
            std::process::exit(1);
        }
        return;
    }
    if let Command::Ui(options) = command {
        if let Err(error) = shell::run(Box::new(app), options) {
            purifex_domain::log_error!("{error}");
            std::process::exit(1);
        }
        return;
    }
    let output = command.run(&mut app, &mut |line| {
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
