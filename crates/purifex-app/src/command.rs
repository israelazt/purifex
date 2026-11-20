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
    Fixes {
        json: bool,
    },
    /// The apps installed.
    Apps(crate::command_apps::AppsCommand),
    /// The MCP server for the user's AI (talks over the standard input and output).
    Mcp,
    /// What Windows Update offers (goes out to the network).
    Updates {
        json: bool,
    },
    FixApply {
        id: String,
        dry_run: bool,
        yes: bool,
        json: bool,
    },
    FixUndo {
        seq: u64,
        yes: bool,
        json: bool,
    },
    History {
        json: bool,
    },
    Compare {
        before: Option<String>,
        after: Option<String>,
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
    /// Open on the apps page already looking for the traces of this app (how the program
    /// starts again as an administrator).
    pub find_traces: Option<String>,
}

/// What the program should write, and whether it went well.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Output {
    pub text: String,
    pub ok: bool,
}

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
            Some("compare") => return Self::parse_compare(args),
            Some("apps") => return crate::command_apps::AppsCommand::parse(args).map(Self::Apps),
            Some("mcp") => return Ok(Self::Mcp),
            Some("updates") => return Self::parse_flags(args, |json| Self::Updates { json }),
            Some("fixes") => return Self::parse_flags(args, |json| Self::Fixes { json }),
            Some("history") => return Self::parse_flags(args, |json| Self::History { json }),
            Some("fix") => return Self::parse_fix(args),
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
        let mut find_traces = None;
        while let Some(arg) = args.next() {
            let mut value = |name: &str| {
                args.next()
                    .ok_or_else(|| format!("{name} necesita un valor"))
            };
            match arg {
                "--screenshot" => path = Some(value("--screenshot")?.to_string()),
                "--page" => page = parse_page(value("--page")?)?,
                "--size" => size = parse_size(value("--size")?)?,
                "--scale" => scale = parse_scale(value("--scale")?)?,
                "--find-traces" => find_traces = Some(value("--find-traces")?.to_string()),
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
        Ok(Self::Ui(UiOptions {
            screenshot,
            find_traces,
        }))
    }

    /// An order whose only option is `--json`.
    fn parse_flags<'a>(
        args: impl Iterator<Item = &'a str>,
        make: impl Fn(bool) -> Self,
    ) -> Result<Self, String> {
        let mut json = false;
        for arg in args {
            match arg {
                "--json" => json = true,
                other => return Err(format!("opción desconocida: {other}")),
            }
        }
        Ok(make(json))
    }

    fn parse_fix<'a>(mut args: impl Iterator<Item = &'a str>) -> Result<Self, String> {
        let action = args.next().ok_or("fix necesita «apply» o «undo»")?;
        let target = args
            .next()
            .ok_or_else(|| format!("fix {action} necesita un valor"))?;
        let (mut dry_run, mut yes, mut json) = (false, false, false);
        for arg in args {
            match arg {
                "--dry-run" if action == "apply" => dry_run = true,
                "--yes" => yes = true,
                "--json" => json = true,
                other => return Err(format!("opción desconocida: {other}")),
            }
        }
        match action {
            "apply" => {
                let valid = !target.is_empty()
                    && target.bytes().all(|b| {
                        b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'.' || b == b'-'
                    });
                if !valid {
                    return Err(format!(
                        "«{target}» no es el nombre de un arreglo (mira `purifex fixes`)"
                    ));
                }
                Ok(Self::FixApply {
                    id: target.to_string(),
                    dry_run,
                    yes,
                    json,
                })
            }
            "undo" => {
                let seq = target
                    .trim_start_matches('#')
                    .parse()
                    .map_err(|_| format!("«{target}» no es un número del historial"))?;
                Ok(Self::FixUndo { seq, yes, json })
            }
            other => Err(format!("fix {other}: solo existen «apply» y «undo»")),
        }
    }

    fn parse_compare<'a>(args: impl Iterator<Item = &'a str>) -> Result<Self, String> {
        let (mut json, mut names) = (false, Vec::new());
        for arg in args {
            match arg {
                "--json" => json = true,
                "--last" => names.clear(),
                name if !name.starts_with('-') => {
                    if !is_valid_id(name) {
                        return Err(format!("«{name}» no es un nombre de grabación válido"));
                    }
                    names.push(name.to_string());
                }
                other => return Err(format!("opción desconocida: {other}")),
            }
        }
        match names.len() {
            0 => Ok(Self::Compare {
                before: None,
                after: None,
                json,
            }),
            2 => Ok(Self::Compare {
                after: names.pop(),
                before: names.pop(),
                json,
            }),
            _ => Err("compare necesita dos nombres (ANTES DESPUÉS) o ninguno".into()),
        }
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
        if let Self::Apps(apps) = self {
            return apps.wants_json();
        }
        matches!(
            self,
            Self::Scan { json: true, .. }
                | Self::Sessions { json: true }
                | Self::Analyze { json: true, .. }
                | Self::Compare { json: true, .. }
                | Self::Updates { json: true }
                | Self::Fixes { json: true }
                | Self::FixApply { json: true, .. }
                | Self::FixUndo { json: true, .. }
                | Self::History { json: true }
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
            Self::Compare { before, after, .. } => services
                .compare(before.as_deref(), after.as_deref())
                .map(|r| Outcome::Comparison(Box::new(r)))
                .map_err(|e| format!("No se pudo comparar: {e}")),
            Self::Updates { .. } => crate::updates_wait::run(services, progress),
            Self::Apps(command) => command.execute(services),
            Self::Fixes { .. } => services
                .fixes()
                .map(Outcome::Fixes)
                .map_err(|e| format!("No se pudieron calcular los arreglos: {e}")),
            Self::FixApply {
                id, dry_run, yes, ..
            } => services
                .apply_fix(&id, yes, dry_run)
                .map(|r| Outcome::Fix(Box::new(r)))
                .map_err(|e| format!("No se aplicó: {e}")),
            Self::FixUndo { seq, yes, .. } => services
                .undo_fix(seq, yes)
                .map(|r| Outcome::Fix(Box::new(r)))
                .map_err(|e| format!("No se deshizo: {e}")),
            Self::History { .. } => services
                .history()
                .map(Outcome::History)
                .map_err(|e| format!("No se pudo leer el historial: {e}")),
            Self::Ui(_) => Err("la interfaz la abre el programa, no un servicio".into()),
            Self::Mcp => Err("el servidor MCP lo arranca el programa".into()),
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

/// The page named by `--page`.
fn parse_page(name: &str) -> Result<Page, String> {
    match name {
        "overview" => Ok(Page::Overview),
        "drivers" => Ok(Page::Drivers),
        "sessions" => Ok(Page::Sessions),
        "fixes" => Ok(Page::Fixes),
        "apps" => Ok(Page::Apps),
        other => Err(format!(
            "página desconocida: {other} (overview, drivers, sessions, fixes, apps)"
        )),
    }
}

/// `--size ANCHOxALTO`, each between 400 and 4000 (300 and 4000 for the height).
fn parse_size(text: &str) -> Result<(f32, f32), String> {
    text.split_once('x')
        .and_then(|(w, h)| Some((w.parse::<f32>().ok()?, h.parse::<f32>().ok()?)))
        .filter(|(w, h)| (400.0..=4000.0).contains(w) && (300.0..=4000.0).contains(h))
        .ok_or_else(|| format!("--size: «{text}» no es ANCHOxALTO (por ejemplo 1440x900)"))
}

/// `--scale N`, between 0.5 and 4.
fn parse_scale(text: &str) -> Result<f32, String> {
    text.parse::<f32>()
        .ok()
        .filter(|s| (0.5..=4.0).contains(s))
        .ok_or_else(|| format!("--scale: «{text}» no es un número entre 0,5 y 4"))
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
    fn the_window_can_open_looking_for_the_traces_of_an_app() {
        let Command::Ui(options) = Command::parse(&["ui", "--find-traces", "OBS Studio"]).unwrap()
        else {
            panic!()
        };
        assert_eq!(options.find_traces.as_deref(), Some("OBS Studio"));
        assert!(Command::parse(&["ui", "--find-traces"]).is_err());
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

    #[test]
    fn compare_takes_two_names_or_none() {
        assert_eq!(
            Command::parse(&["compare"]).unwrap(),
            Command::Compare {
                before: None,
                after: None,
                json: false
            }
        );
        assert_eq!(
            Command::parse(&["compare", "lol-1", "lol-2", "--json"]).unwrap(),
            Command::Compare {
                before: Some("lol-1".into()),
                after: Some("lol-2".into()),
                json: true
            }
        );
        for bad in [
            &["compare", "lol-1"][..],
            &["compare", "a", "b", "c"],
            &["compare", "../x", "b"],
            &["compare", "--x"],
        ] {
            assert!(Command::parse(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn compare_without_two_games_says_why_and_exits_with_an_error() {
        let mut app = app(FakeInventory::with(vec![]));
        let out = Command::Compare {
            before: None,
            after: None,
            json: false,
        }
        .run(&mut app, &mut |_| {});
        assert!(
            !out.ok && out.text.contains("al menos dos partidas"),
            "{}",
            out.text
        );
    }

    #[test]
    fn the_orders_of_the_fixes_are_read() {
        assert_eq!(
            Command::parse(&["fixes", "--json"]).unwrap(),
            Command::Fixes { json: true }
        );
        assert_eq!(
            Command::parse(&["history"]).unwrap(),
            Command::History { json: false }
        );
        assert_eq!(
            Command::parse(&["fix", "apply", "lol.close-client-during-game", "--dry-run"]).unwrap(),
            Command::FixApply {
                id: "lol.close-client-during-game".into(),
                dry_run: true,
                yes: false,
                json: false
            }
        );
        assert_eq!(
            Command::parse(&["fix", "apply", "lol.close-client-during-game", "--yes"]).unwrap(),
            Command::FixApply {
                id: "lol.close-client-during-game".into(),
                dry_run: false,
                yes: true,
                json: false
            }
        );
        assert_eq!(
            Command::parse(&["fix", "undo", "#3", "--yes"]).unwrap(),
            Command::FixUndo {
                seq: 3,
                yes: true,
                json: false
            }
        );
        for bad in [
            &["fix"][..],
            &["fix", "apply"],
            &["fix", "install", "x"],
            &["fix", "apply", "Bad Name"],
            &["fix", "apply", "../x"],
            &["fix", "undo", "x"],
            &["fix", "undo", "1", "--dry-run"],
            &["fixes", "--all"],
        ] {
            assert!(Command::parse(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn with_no_fixes_available_the_orders_say_so_and_fail() {
        let mut app = app(FakeInventory::with(vec![]));
        let fixes = Command::Fixes { json: false }.run(&mut app, &mut |_| {});
        assert!(
            fixes.ok && fixes.text.contains("no los justifica"),
            "{}",
            fixes.text
        );
        let apply = Command::FixApply {
            id: "lol.close-client-during-game".into(),
            dry_run: false,
            yes: true,
            json: false,
        }
        .run(&mut app, &mut |_| {});
        assert!(
            !apply.ok && apply.text.contains("No se aplicó"),
            "{}",
            apply.text
        );
        assert!(Command::History { json: false }
            .run(&mut app, &mut |_| {})
            .text
            .contains("Todavía no"));
    }
}
