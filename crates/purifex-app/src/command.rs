//! What the user asks for on the command line, and what the program prints.
//! Reading the arguments is a decision, so it is here and tested, not in `main`.

use std::time::Duration;

use purifex_domain::ports::{Clock, PerformanceSampler, SessionFile, SystemInventory};
use purifex_domain::report::Scope;

use crate::recorder::{Recorder, WatchOptions};
use crate::scanner::Scanner;

/// What the command line asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    Scan { json: bool, scope: Scope },
    Watch(WatchOptions),
    Help,
}

/// What the program should write, and whether it went well.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Output {
    pub text: String,
    pub ok: bool,
}

pub const HELP: &str = "Purifex: lo que hay en tu PC, rápido y sin tocar nada.

Uso:
  purifex scan [--json] [--all]   Lista los dispositivos y sus drivers
  purifex watch [--now] [--seconds N]
                                  Graba el rendimiento mientras juegas a LoL
  purifex help                    Esta ayuda

Opciones de scan:
  --json   Salida en JSON, para scripts
  --all    Incluye también los dispositivos que el sistema inventa (raíz, software)

Opciones de watch (sin opciones: espera a que abras la partida y para al cerrarla):
  --now        Graba ya, sin esperar al juego
  --seconds N  Graba N segundos (implica --now)
";

impl Command {
    /// Reads the arguments after the program's name. `scan` is the default.
    pub fn parse<S: AsRef<str>>(args: &[S]) -> Result<Self, String> {
        let mut args = args.iter().map(AsRef::as_ref);
        match args.next() {
            None | Some("scan") => {}
            Some("help" | "--help" | "-h") => return Ok(Self::Help),
            Some("watch") => return Self::parse_watch(args),
            Some(other) => return Err(format!("orden desconocida: {other}")),
        }
        let (mut json, mut scope) = (false, Scope::Hardware);
        for arg in args {
            match arg {
                "--json" => json = true,
                "--all" => scope = Scope::All,
                other => return Err(format!("opción desconocida: {other}")),
            }
        }
        Ok(Self::Scan { json, scope })
    }

    fn parse_watch<'a>(mut args: impl Iterator<Item = &'a str>) -> Result<Self, String> {
        let mut options = WatchOptions::default();
        while let Some(arg) = args.next() {
            match arg {
                "--now" => options.wait_for_game = false,
                "--seconds" => {
                    let value = args.next().ok_or("--seconds necesita un número")?;
                    let seconds: u64 =
                        value.parse().ok().filter(|n| *n > 0).ok_or_else(|| {
                            format!("--seconds: «{value}» no es un número válido")
                        })?;
                    options.max_seconds = Some(seconds);
                    options.wait_for_game = false;
                }
                other => return Err(format!("opción desconocida: {other}")),
            }
        }
        options.interval = Duration::from_secs(1);
        Ok(Self::Watch(options))
    }

    /// Runs it against the system's ports. `progress` receives the lines that
    /// are worth showing while a long command works.
    pub fn run<I, C, S, C2, F>(
        self,
        scanner: &Scanner<I, C>,
        recorder: &mut Recorder<S, C2, F>,
        progress: &mut dyn FnMut(&str),
    ) -> Output
    where
        I: SystemInventory,
        C: Clock,
        S: PerformanceSampler,
        C2: Clock,
        F: SessionFile,
    {
        match self {
            Self::Watch(options) => match recorder.run(options, "lol", progress) {
                Ok(done) => Output {
                    text: format!(
                        "
{}
Datos segundo a segundo: {}",
                        done.summary.to_text(),
                        done.file
                    ),
                    ok: true,
                },
                Err(error) => Output {
                    text: format!("No se pudo grabar: {error}"),
                    ok: false,
                },
            },
            Self::Help => Output {
                text: HELP.to_string(),
                ok: true,
            },
            Self::Scan { json, scope } => match scanner.scan(scope) {
                Ok(report) => Output {
                    text: if json {
                        report.to_json()
                    } else {
                        report.to_text()
                    },
                    ok: true,
                },
                Err(error) => Output {
                    text: format!("No se pudo analizar: {error}"),
                    ok: false,
                },
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{FakeClock, FakeFile, FakeInventory, FakeSampler};

    fn recorder() -> Recorder<FakeSampler, FakeClock, FakeFile> {
        Recorder::new(
            FakeSampler::new(0, 3, true),
            FakeClock::ticking(0),
            FakeFile::default(),
        )
    }

    #[test]
    fn scan_is_the_default_and_takes_its_options() {
        let none: [&str; 0] = [];
        assert_eq!(
            Command::parse(&none).unwrap(),
            Command::Scan {
                json: false,
                scope: Scope::Hardware
            }
        );
        assert_eq!(
            Command::parse(&["scan", "--json", "--all"]).unwrap(),
            Command::Scan {
                json: true,
                scope: Scope::All
            }
        );
        assert_eq!(Command::parse(&["--help"]).unwrap(), Command::Help);
    }

    #[test]
    fn what_it_does_not_know_is_an_error() {
        assert!(Command::parse(&["install"])
            .unwrap_err()
            .contains("install"));
        assert!(Command::parse(&["scan", "--fast"])
            .unwrap_err()
            .contains("--fast"));
    }

    #[test]
    fn watch_waits_for_the_game_unless_told_otherwise() {
        let none = ["watch"];
        let Command::Watch(o) = Command::parse(&none).unwrap() else {
            panic!()
        };
        assert!(o.wait_for_game && o.max_seconds.is_none());
        let Command::Watch(o) = Command::parse(&["watch", "--now"]).unwrap() else {
            panic!()
        };
        assert!(!o.wait_for_game);
        let Command::Watch(o) = Command::parse(&["watch", "--seconds", "90"]).unwrap() else {
            panic!()
        };
        assert_eq!((o.wait_for_game, o.max_seconds), (false, Some(90)));
    }

    #[test]
    fn watch_rejects_a_bad_number_of_seconds() {
        for bad in [
            &["watch", "--seconds"][..],
            &["watch", "--seconds", "x"],
            &["watch", "--seconds", "0"],
        ] {
            assert!(Command::parse(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn watch_reports_the_summary_and_where_the_data_is() {
        let scanner = Scanner::new(FakeInventory::with(vec![]), FakeClock::ticking(1));
        let out = Command::parse(&["watch"])
            .unwrap()
            .run(&scanner, &mut recorder(), &mut |_| {});
        assert!(out.ok);
        assert!(
            out.text.contains("fake/lol.csv") && out.text.contains("Sesión de 3 s"),
            "{}",
            out.text
        );
    }

    #[test]
    fn a_failure_is_reported_and_not_ok() {
        let scanner = Scanner::new(FakeInventory::failing("denied"), FakeClock::ticking(1));
        let out = Command::Scan {
            json: false,
            scope: Scope::All,
        }
        .run(&scanner, &mut recorder(), &mut |_| {});
        assert!(!out.ok);
        assert!(out.text.contains("denied"));
    }

    #[test]
    fn json_output_parses() {
        let scanner = Scanner::new(FakeInventory::with(vec![]), FakeClock::ticking(1));
        let out = Command::Scan {
            json: true,
            scope: Scope::All,
        }
        .run(&scanner, &mut recorder(), &mut |_| {});
        assert!(out.ok);
        let value: serde_json::Value = serde_json::from_str(&out.text).unwrap();
        assert_eq!(value["summary"]["scanned"], 0);
    }
}
