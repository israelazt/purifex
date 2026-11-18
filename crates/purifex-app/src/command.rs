//! What the user asks for on the command line, and what the program prints.
//! Reading the arguments is a decision, so it is here and tested, not in `main`.

use std::time::Duration;

use purifex_domain::report::Scope;
use purifex_domain::session_id::is_valid_id;
use purifex_domain::view::Page;

use crate::outcome::Outcome;
use crate::recorder::WatchOptions;
use crate::services::Services;

/// What the command line asks for.
#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    Scan {
        json: bool,
        scope: Scope,
    },
    Watch(WatchOptions),
    Sessions {
        json: bool,
    },
    Analyze {
        id: Option<String>,
        json: bool,
    },
    /// Open the window (or draw it to a file). The program does it; this order
    /// only asks for it.
    Ui(UiOptions),
    Help,
}

/// A picture of the interface saved to a file instead of opening the window: how
/// the design is checked from a script.
#[derive(Debug, Clone, PartialEq)]
pub struct Screenshot {
    pub path: String,
    pub page: Page,
    /// In units (1/96 inch).
    pub width: f32,
    pub height: f32,
    pub scale: f32,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct UiOptions {
    pub screenshot: Option<Screenshot>,
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
  purifex sessions [--json]       Lista las grabaciones, la más nueva primero
  purifex analyze [NOMBRE] [--json]
                                  Analiza una grabación (por defecto, la última)
  purifex ui [--screenshot ARCHIVO.png [--page NOMBRE] [--size ANCHOxALTO] [--scale N]]
                                  Abre la interfaz (o la guarda como imagen)
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
            Some("sessions") => return Self::parse_sessions(args),
            Some("analyze") => return Self::parse_analyze(args),
            Some("ui") => return Self::parse_ui(args),
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

    fn parse_sessions<'a>(args: impl Iterator<Item = &'a str>) -> Result<Self, String> {
        let mut json = false;
        for arg in args {
            match arg {
                "--json" => json = true,
                other => return Err(format!("opción desconocida: {other}")),
            }
        }
        Ok(Self::Sessions { json })
    }

    fn parse_ui<'a>(mut args: impl Iterator<Item = &'a str>) -> Result<Self, String> {
        let (mut path, mut page, mut size, mut scale) =
            (None, Page::Overview, (1440.0, 900.0), 1.0f32);
        while let Some(arg) = args.next() {
            let mut value = |name: &str| {
                args.next()
                    .ok_or_else(|| format!("{name} necesita un valor"))
            };
            match arg {
                "--screenshot" => path = Some(value("--screenshot")?.to_string()),
                "--page" => {
                    page = match value("--page")? {
                        "overview" => Page::Overview,
                        "drivers" => Page::Drivers,
                        "sessions" => Page::Sessions,
                        other => {
                            return Err(format!(
                                "página desconocida: {other} (overview, drivers, sessions)"
                            ))
                        }
                    }
                }
                "--size" => {
                    let text = value("--size")?;
                    size = text
                        .split_once('x')
                        .and_then(|(w, h)| Some((w.parse::<f32>().ok()?, h.parse::<f32>().ok()?)))
                        .filter(|(w, h)| {
                            (400.0..=4000.0).contains(w) && (300.0..=4000.0).contains(h)
                        })
                        .ok_or_else(|| {
                            format!("--size: «{text}» no es ANCHOxALTO (por ejemplo 1440x900)")
                        })?;
                }
                "--scale" => {
                    let text = value("--scale")?;
                    scale = text
                        .parse::<f32>()
                        .ok()
                        .filter(|s| (0.5..=4.0).contains(s))
                        .ok_or_else(|| {
                            format!("--scale: «{text}» no es un número entre 0,5 y 4")
                        })?;
                }
                other => return Err(format!("opción desconocida: {other}")),
            }
        }
        let screenshot = path.map(|path| Screenshot {
            path,
            page,
            width: size.0,
            height: size.1,
            scale,
        });
        Ok(Self::Ui(UiOptions { screenshot }))
    }

    fn parse_analyze<'a>(args: impl Iterator<Item = &'a str>) -> Result<Self, String> {
        let (mut json, mut id) = (false, None);
        for arg in args {
            match arg {
                "--json" => json = true,
                "--last" => id = None,
                name if !name.starts_with('-') && id.is_none() => {
                    if !is_valid_id(name) {
                        return Err(format!("«{name}» no es un nombre de grabación válido"));
                    }
                    id = Some(name.to_string());
                }
                other => return Err(format!("opción desconocida: {other}")),
            }
        }
        Ok(Self::Analyze { id, json })
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
        matches!(
            self,
            Self::Scan { json: true, .. }
                | Self::Sessions { json: true }
                | Self::Analyze { json: true, .. }
        )
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
            Self::Ui(_) => Err("la interfaz la abre el programa, no un servicio".into()),
            Self::Analyze { id, .. } => services
                .analyze(id.as_deref())
                .map(|a| Outcome::Analysis(Box::new(a)))
                .map_err(|e| format!("No se pudo analizar la grabación: {e}")),
            Self::Sessions { .. } => services
                .sessions()
                .map(Outcome::Sessions)
                .map_err(|e| format!("No se pudieron leer las grabaciones: {e}")),
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
    use crate::archive::Archive;
    use crate::recorder::Recorder;
    use crate::scanner::Scanner;
    use crate::services::App;
    use crate::testing::{FakeArchive, FakeClock, FakeFile, FakeInventory, FakeSampler};

    type TestApp = App<FakeInventory, FakeClock, FakeSampler, FakeClock, FakeFile, FakeArchive>;

    fn app(inventory: FakeInventory) -> TestApp {
        app_with(inventory, FakeArchive::default())
    }

    fn app_with(inventory: FakeInventory, archive: FakeArchive) -> TestApp {
        App::new(
            Scanner::new(inventory, FakeClock::ticking(1)),
            Recorder::new(
                FakeSampler::new(0, 3, true),
                FakeClock::ticking(0),
                FakeFile::default(),
            ),
            Archive::new(archive),
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

    #[test]
    fn sessions_takes_json_and_nothing_else() {
        assert_eq!(
            Command::parse(&["sessions"]).unwrap(),
            Command::Sessions { json: false }
        );
        assert_eq!(
            Command::parse(&["sessions", "--json"]).unwrap(),
            Command::Sessions { json: true }
        );
        assert!(Command::parse(&["sessions", "--all"]).is_err());
    }

    #[test]
    fn sessions_lists_the_recordings() {
        let csv = "t_s,cpu_total,cpu_max_core\n1.0,1.0,1.0\n";
        let mut app = app_with(
            FakeInventory::with(vec![]),
            FakeArchive::with(&[("lol-20261118-165329", csv)]),
        );
        let out = Command::Sessions { json: false }.run(&mut app, &mut |_| {});
        assert!(
            out.ok && out.text.contains("lol-20261118-165329"),
            "{}",
            out.text
        );
        let json = Command::Sessions { json: true }.run(&mut app, &mut |_| {});
        let value: serde_json::Value = serde_json::from_str(&json.text).unwrap();
        assert_eq!(value["sessions"][0]["id"], "lol-20261118-165329");
    }

    #[test]
    fn analyze_takes_a_name_or_the_last_one() {
        assert_eq!(
            Command::parse(&["analyze"]).unwrap(),
            Command::Analyze {
                id: None,
                json: false
            }
        );
        assert_eq!(
            Command::parse(&["analyze", "lol-20261118-165329", "--json"]).unwrap(),
            Command::Analyze {
                id: Some("lol-20261118-165329".into()),
                json: true
            }
        );
        assert_eq!(
            Command::parse(&["analyze", "--last"]).unwrap(),
            Command::Analyze {
                id: None,
                json: false
            }
        );
        for bad in [
            &["analyze", "../x"][..],
            &["analyze", "a", "b"],
            &["analyze", "--x"],
        ] {
            assert!(Command::parse(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn analyze_prints_the_summary_of_the_recording() {
        let csv = "t_s,cpu_total,cpu_max_core\n1.0,10.0,20.0\n2.0,11.0,21.0\n";
        let mut app = app_with(
            FakeInventory::with(vec![]),
            FakeArchive::with(&[("lol-20261118-165329", csv)]),
        );
        let out = Command::Analyze {
            id: None,
            json: false,
        }
        .run(&mut app, &mut |_| {});
        assert!(
            out.ok && out.text.contains("Grabación lol-20261118-165329"),
            "{}",
            out.text
        );
        let missing = Command::Analyze {
            id: Some("zzz".into()),
            json: false,
        }
        .run(&mut app, &mut |_| {});
        assert!(!missing.ok && missing.text.contains("no existe"));
    }

    #[test]
    fn ui_opens_the_window_and_can_save_a_picture_instead() {
        assert_eq!(
            Command::parse(&["ui"]).unwrap(),
            Command::Ui(UiOptions::default())
        );
        let Command::Ui(options) = Command::parse(&[
            "ui",
            "--screenshot",
            "a.png",
            "--page",
            "drivers",
            "--size",
            "1000x700",
            "--scale",
            "2",
        ])
        .unwrap() else {
            panic!()
        };
        assert_eq!(
            options.screenshot,
            Some(Screenshot {
                path: "a.png".into(),
                page: Page::Drivers,
                width: 1000.0,
                height: 700.0,
                scale: 2.0
            })
        );
    }

    #[test]
    fn the_options_of_ui_are_checked() {
        for bad in [
            &["ui", "--screenshot"][..],
            &["ui", "--page", "home"],
            &["ui", "--size", "big"],
            &["ui", "--size", "10x10"],
            &["ui", "--scale", "9"],
            &["ui", "--no-open"],
        ] {
            assert!(Command::parse(bad).is_err(), "{bad:?}");
        }
    }
}
