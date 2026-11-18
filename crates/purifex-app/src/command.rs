//! What the user asks for on the command line, and what the program prints.
//! Reading the arguments is a decision, so it is here and tested, not in `main`.

use std::time::Duration;

use purifex_domain::report::Scope;

use crate::outcome::Outcome;
use crate::recorder::WatchOptions;
use crate::services::Services;

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

    /// Whether the output is asked in JSON.
    pub fn wants_json(&self) -> bool {
        matches!(self, Self::Scan { json: true, .. })
    }

    /// Runs it against the services and returns what it produced as data.
    /// `progress` receives the lines that are worth showing while a long
    /// command works.
    pub fn execute(
        self,
        services: &mut dyn Services,
        progress: &mut dyn FnMut(&str),
    ) -> Result<Outcome, String> {
        match self {
            Self::Help => Ok(Outcome::Help),
            Self::Scan { scope, .. } => services
                .scan(scope)
                .map(Outcome::Report)
                .map_err(|e| format!("No se pudo analizar: {e}")),
            Self::Watch(options) => services
                .record(options, progress)
                .map(Outcome::Recording)
                .map_err(|e| format!("No se pudo grabar: {e}")),
        }
    }

    /// Like [`Command::execute`], rendered for the console.
    pub fn run(self, services: &mut dyn Services, progress: &mut dyn FnMut(&str)) -> Output {
        let json = self.wants_json();
        match self.execute(services, progress) {
            Ok(outcome) => Output {
                text: outcome.render(json),
                ok: true,
            },
            Err(text) => Output { text, ok: false },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::recorder::Recorder;
    use crate::scanner::Scanner;
    use crate::services::App;
    use crate::testing::{FakeClock, FakeFile, FakeInventory, FakeSampler};

    fn app(
        inventory: FakeInventory,
    ) -> App<FakeInventory, FakeClock, FakeSampler, FakeClock, FakeFile> {
        App::new(
            Scanner::new(inventory, FakeClock::ticking(1)),
            Recorder::new(
                FakeSampler::new(0, 3, true),
                FakeClock::ticking(0),
                FakeFile::default(),
            ),
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
        let mut app = app(FakeInventory::with(vec![]));
        let out = Command::parse(&["watch"])
            .unwrap()
            .run(&mut app, &mut |_| {});
        assert!(out.ok);
        assert!(
            out.text.contains("fake/lol.csv") && out.text.contains("Sesión de 3 s"),
            "{}",
            out.text
        );
    }

    #[test]
    fn a_failure_is_reported_and_not_ok() {
        let mut app = app(FakeInventory::failing("denied"));
        let out = Command::Scan {
            json: false,
            scope: Scope::All,
        }
        .run(&mut app, &mut |_| {});
        assert!(!out.ok);
        assert!(out.text.contains("denied"));
    }

    #[test]
    fn json_output_parses() {
        let mut app = app(FakeInventory::with(vec![]));
        let out = Command::Scan {
            json: true,
            scope: Scope::All,
        }
        .run(&mut app, &mut |_| {});
        assert!(out.ok);
        let value: serde_json::Value = serde_json::from_str(&out.text).unwrap();
        assert_eq!(value["summary"]["scanned"], 0);
    }
}
