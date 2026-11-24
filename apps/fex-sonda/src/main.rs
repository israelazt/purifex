//! Fex Sonda: an app made to be installed and uninstalled for real, to check that Purifex finds
//! and removes what an uninstaller leaves. It is its own app, installer and uninstaller. What it
//! writes is listed in `manifest`. See `docs/TEST_APP_PLAN.md`.

mod command;
mod install;
mod items;
mod manifest;
mod places;
mod purge;
mod removal;
mod say;
mod status;
mod system;
mod uninstall;

use std::process::ExitCode;

use command::Command;

fn started_as() -> String {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
        .unwrap_or_default()
}

fn run(command: Command) -> Result<(), String> {
    match command {
        Command::Install => install::install(),
        Command::Run => install::usage(),
        Command::Update => Ok(()),
        Command::Uninstall => uninstall::uninstall(),
        Command::Status { json } => status::status(json),
        Command::Purge => purge::purge(),
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match command::parse(&args, &started_as()).and_then(run) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            say::error(&error);
            ExitCode::FAILURE
        }
    }
}
