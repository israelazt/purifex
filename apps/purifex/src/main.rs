//! Purifex: the program. It creates the real adapters, hands them to the
//! application and prints what it returns. The decisions are not here.
//! See `ARCHITECTURE.md`.

#[cfg(windows)]
mod shell;

/// Writes a line to the standard output. A closed pipe (`| head`) is not an error.
#[cfg(windows)]
fn say(line: &str) {
    use std::io::Write;
    let _ = writeln!(std::io::stdout(), "{line}");
}

#[cfg(windows)]
fn main() {
    use purifex_app::Command;

    purifex_platform::install_stderr_log();
    let args: Vec<String> = std::env::args().skip(1).collect();
    let command = match Command::parse(&args) {
        Ok(command) => command,
        Err(error) => {
            purifex_domain::log_error!("{error}");
            std::process::exit(2);
        }
    };
    let mut app = build_app();
    match command {
        Command::Mcp => serve_mcp(&mut app),
        Command::Ui(options) => open_window(app, options),
        other => run_command(other, &mut app),
    }
}

/// The application with the real adapters of Windows: the only place that knows them.
#[cfg(windows)]
fn build_app() -> impl purifex_app::Services + 'static {
    use purifex_app::{App, Archive, Candidates, Cleaning, Fixer, Offering, Recorder, Scanner};
    use purifex_platform::{
        BrowserOpener, JsonlCandidates, JsonlHistory, LocalCleanup, LocalSessionArchive,
        LocalSessionFile, RegisteredUninstaller, RegistryApps, SetupApiInventory, SystemClock,
        SystemFixes, WindowsElevation, WindowsSampler, WindowsSeedProbe, WindowsTraceFinder,
        WindowsUpdateOffers,
    };

    let scanner = Scanner::new(SetupApiInventory, SystemClock::new());
    // The game's program, as it appears in the process list.
    let sampler = WindowsSampler::new(&["League of Legends.exe"]);
    let recorder = Recorder::new(sampler, SystemClock::new(), LocalSessionFile::default());
    let archive = Archive::new(LocalSessionArchive::new());
    let fixer = Fixer::new(Box::new(SystemFixes::new()), Box::new(JsonlHistory::new()));
    let offering = Offering::new(std::sync::Arc::new(WindowsUpdateOffers));
    let cleaning = Cleaning::new(
        Box::new(WindowsTraceFinder),
        Box::new(RegisteredUninstaller),
        Box::new(LocalCleanup::new()),
    )
    .with_probe(Box::new(WindowsSeedProbe))
    .with_clock(Box::new(SystemClock::new()));
    App::new(scanner, recorder, archive)
        .with_fixer(fixer)
        .with_offers(offering)
        .with_candidates(Candidates::new(Box::new(JsonlCandidates::new())))
        .with_opener(Box::new(BrowserOpener))
        .with_apps(Box::new(RegistryApps))
        .with_cleaning(cleaning)
        .with_elevation(Box::new(WindowsElevation))
}

/// The MCP server: it talks over the standard input and output until the client leaves.
#[cfg(windows)]
fn serve_mcp(app: &mut dyn purifex_app::Services) {
    let stdin = std::io::stdin();
    if let Err(error) = purifex_app::mcp::serve(app, stdin.lock(), std::io::stdout()) {
        purifex_domain::log_error!("mcp: {error}");
        std::process::exit(1);
    }
}

/// The window (or the screenshot of it).
#[cfg(windows)]
fn open_window(app: impl purifex_app::Services + 'static, options: purifex_app::UiOptions) {
    if let Err(error) = shell::run(Box::new(app), options) {
        purifex_domain::log_error!("{error}");
        std::process::exit(1);
    }
}

/// Any other order: run it, print what it says, and exit with its status.
#[cfg(windows)]
fn run_command(command: purifex_app::Command, app: &mut dyn purifex_app::Services) {
    let output = command.run(app, &mut |line| say(line));
    say(&output.text);
    std::process::exit(if output.ok { 0 } else { 1 });
}

/// Purifex only has its system adapters for Windows so far.
#[cfg(not(windows))]
fn main() {
    purifex_domain::log_error!("Purifex does not have an adapter for this system yet.");
    std::process::exit(1);
}
