//! Draws the fixes page in states that are hard to reach on a real PC (a pending
//! confirmation, the history with an undo, a notice) from a fixed model, to check
//! the design. It changes nothing on the computer.
//! `cargo run --example ui_states -- <ready|blocked|confirm|history|undo> out.png`

#[cfg(windows)]
mod states {
    use std::time::Duration;

    use purifex_domain::draw::DrawList;
    use purifex_domain::fix::{
        FixReport, HistoryEntry, HistoryState, Proposal, SettingValue, LOL_CLOSE_CLIENT,
    };
    use purifex_domain::ports::TextMetrics;
    use purifex_domain::ui_event::{Reaction, UiEvent, UiHandler};
    use purifex_domain::view::{Confirm, Model, Page};
    use purifex_ui::Ui;

    struct Fixed {
        ui: Ui,
        model: Model,
    }

    impl UiHandler for Fixed {
        fn frame(&mut self, w: f32, h: f32, text: &dyn TextMetrics, now: Duration) -> DrawList {
            self.ui.frame(&self.model, w, h, text, now)
        }
        fn animating(&self) -> bool {
            false
        }
        fn set_reduced_motion(&mut self, _: bool) {}
        fn event(&mut self, _: UiEvent) -> Reaction {
            Reaction::default()
        }
        fn tick(&mut self) -> Reaction {
            Reaction::default()
        }
        fn hand_cursor(&self) -> bool {
            false
        }
    }

    fn proposal(blocked: bool) -> Proposal {
        Proposal {
            id: LOL_CLOSE_CLIENT.into(),
            title: "Cerrar el cliente de League durante la partida".into(),
            evidence: "El cliente de League usó de media un 15.9 % del CPU durante tu última partida (lol-20261118-145915), y el juego un 13.5 %.".into(),
            current: SettingValue::Bool(false),
            target: SettingValue::Bool(true),
            undo: "Vuelve a poner el ajuste como estaba (el archivo de Riot se restaura a su valor anterior).".into(),
            needs_admin: false,
            blockers: if blocked { vec!["Cierra el cliente de League: reescribe esta configuración al salir y desharía el cambio".into()] } else { vec![] },
        }
    }

    fn entry(seq: u64, state: HistoryState, undoes: Option<u64>) -> HistoryEntry {
        HistoryEntry {
            seq,
            fix: LOL_CLOSE_CLIENT.into(),
            before: SettingValue::Bool(false),
            after: SettingValue::Bool(true),
            state,
            undoes,
            session: Some("lol-20261118-145915".into()),
        }
    }

    pub fn main() {
        let args: Vec<String> = std::env::args().skip(1).collect();
        let state = args.first().map_or("ready", String::as_str);
        let out = args
            .get(1)
            .cloned()
            .unwrap_or_else(|| "ui_states.png".into());
        let model = model_for(state, args.get(2).map(String::as_str));
        let mut handler = Fixed {
            ui: Ui::new(),
            model,
        };
        match purifex_platform::render_png(
            std::path::Path::new(&out),
            1440.0,
            1100.0,
            1.0,
            &mut handler,
        ) {
            Ok(()) => println!("{out}"),
            Err(error) => {
                println!("error: {error}");
                std::process::exit(1);
            }
        }
    }

    /// The model of a state: the fixes page ready (the default), or one of the others.
    fn model_for(state: &str, extra: Option<&str>) -> Model {
        let ready = Model {
            page: Page::Fixes,
            history: Some(vec![]),
            fixes: Some(vec![proposal(false)]),
            ..Model::default()
        };
        match state {
            "blocked" | "confirm" | "history" | "undo" => fixes_state(ready, state),
            "apps" => apps_state(ready, extra),
            "monitor" => monitor_state(apps_state(ready, None), extra.unwrap_or("logs")),
            "batch" => batch_state(apps_state(ready, None), extra.unwrap_or("marked")),
            "kdenlive" => kdenlive_state(ready),
            "kdenlive-admin" => Model {
                elevated: Some(false),
                ..kdenlive_state(ready)
            },
            "offers" => offers_state(ready),
            _ => ready,
        }
    }

    /// The states of the fixes page: a blocked proposal, a pending confirmation, the
    /// history with a notice, and its undo question.
    fn fixes_state(mut model: Model, state: &str) -> Model {
        match state {
            "blocked" => model.fixes = Some(vec![proposal(true)]),
            "confirm" => model.confirm = Some(Confirm::Apply(LOL_CLOSE_CLIENT.into())),
            _ => {
                model.fixes = Some(vec![]);
                model.history = Some(vec![
                    entry(1, HistoryState::Applied, None),
                    entry(2, HistoryState::Undone, Some(1)),
                    entry(3, HistoryState::Applied, None),
                ]);
                model.notice = Some(applied_notice());
                if state == "undo" {
                    model.confirm = Some(Confirm::Undo(3));
                }
            }
        }
        model
    }

    fn applied_notice() -> FixReport {
        FixReport {
            fix: LOL_CLOSE_CLIENT.into(),
            title: "Cerrar el cliente de League durante la partida".into(),
            before: SettingValue::Bool(false),
            after: SettingValue::Bool(true),
            preview: false,
            seq: Some(3),
            undo: String::new(),
            needs_confirmation: false,
            was_undo: false,
        }
    }

    /// The apps page with the copies of the registration of two apps that are gone.
    fn with_backups(mut model: Model) -> Model {
        model.traces_open = false;
        model.running = Some(purifex_domain::view::Running {
            app: "Inkscape".into(),
            names: vec!["inkscape.exe".into(), "inkview.exe".into()],
        });
        model.in_use = Some(purifex_domain::view::InUse {
            app: "Inkscape".into(),
            items: 3,
        });
        model.backups = Some(vec![
            purifex_domain::safeguard::BackupEntry {
                app: "Kdenlive".into(),
                key: "HKCU".into(),
                registry_copy: "kdenlive.reg".into(),
                on: purifex_domain::date::Date::new(2026, 11, 20),
            },
            purifex_domain::safeguard::BackupEntry {
                app: "Inkscape".into(),
                key: "HKCU".into(),
                registry_copy: "inkscape.reg".into(),
                on: purifex_domain::date::Date::new(2026, 11, 18),
            },
        ]);
        model
    }

    /// An installed app for the pictures: its install day comes from its size, to vary.
    fn sample_app(name: &str, publisher: &str, kb: u64) -> purifex_domain::apps::InstalledApp {
        purifex_domain::apps::InstalledApp {
            id: format!("HKLM64:{name}"),
            name: name.into(),
            publisher: Some(publisher.into()),
            version: Some("1.0".into()),
            uninstall: Some("u.exe".into()),
            size_kb: Some(kb),
            install_location: Some(format!(r"C:\Program Files\{name}")),
            website: Some("https://example.org".into()),
            install_date: purifex_domain::date::Date::new(2026, 11, (kb % 27) as u8 + 1),
            ..Default::default()
        }
    }

    /// The apps page after uninstalling Inkscape, with its traces; `extra` picks a variant
    /// (`all`, `uninstall`, `unchecked`).
    fn apps_state(mut model: Model, extra: Option<&str>) -> Model {
        model.today = purifex_domain::date::Date::new(2026, 11, 21);
        model.page = Page::Apps;
        model.apps = Some(vec![
            sample_app("Discord", "Discord Inc.", 110_000),
            sample_app("GIMP 2.10", "The GIMP Team", 1_250_000),
            sample_app("Microsoft .NET Runtime 8", "Microsoft Corporation", 190_000),
        ]);
        model.uninstalled = Some("Inkscape".into());
        model.message = Some("Se abrió el desinstalador de Inkscape.".into());
        model.traces = Some(inkscape_traces());
        model.traces_open = true;
        match extra {
            Some("all") => model.confirm = Some(Confirm::CleanAll("Inkscape".into())),
            Some("uninstall") => {
                model.traces_open = false;
                model.confirm = Some(Confirm::Uninstall("HKLM64:Discord".into()));
            }
            Some("quarantine" | "empty") => {
                model.traces_open = false;
                model.quarantine = Some(purifex_domain::quarantine::QuarantineInfo {
                    items: 14,
                    bytes: 205_569_471,
                    oldest: purifex_domain::date::Date::new(2026, 11, 14),
                    expired: 0,
                    latest: Some(purifex_domain::quarantine::Latest {
                        app: "Kdenlive".into(),
                        items: 14,
                    }),
                });
                if extra == Some("empty") {
                    model.confirm = Some(Confirm::EmptyQuarantine);
                }
            }
            Some("backups") => model = with_backups(model),
            Some("hunted" | "aiming") => model = hunt_state(model, extra == Some("aiming")),
            Some("list") => {
                model.traces_open = false;
                model.app_view = purifex_domain::app_list::AppView::List;
            }
            Some("details") => {
                model.traces_open = false;
                model.app_selected = Some("HKLM64:Discord".into());
            }
            Some("unchecked") => {
                model.unchecked = vec![r"C:\Users\yo\AppData\Local\Programs\Inkscape".into()];
            }
            _ => {}
        }
        model
    }

    /// The install monitor: logs kept (`logs`), one open (`open`) or an installer being watched
    /// (`watching`).
    fn monitor_state(mut model: Model, step: &str) -> Model {
        use purifex_domain::app_traces::TraceKind;
        use purifex_domain::monitor::{InstallLog, Mark};
        (model.traces_open, model.uninstalled, model.traces) = (false, None, None);
        let places = [
            (TraceKind::Folder, r"C:\Users\yo\AppData\Local\Programs\Zed"),
            (TraceKind::Folder, r"C:\Users\yo\AppData\Roaming\Zed"),
            (TraceKind::RegistryKey, r"HKCU\Software\Zed Industries"),
        ];
        let marks = places.map(|(kind, location)| Mark {
            kind,
            location: location.into(),
            value: None,
            stamp: String::new(),
            bytes: 1 << 20,
        });
        let log = |id: &str, app: &str, own| InstallLog {
            id: id.into(),
            installer: "zed-setup.exe".into(),
            app: Some(app.into()),
            on: purifex_domain::date::Date::new(2026, 11, 23),
            own,
            marks: marks.to_vec(),
        };
        model.install_logs = Some(vec![
            log("2026-11-23-1", "Zed Editor", true),
            log("importado-9", "Otra App", false),
        ]);
        model.log_open = (step == "open").then(|| "2026-11-23-1".to_string());
        model.monitoring = (step == "watching").then(|| "zed-setup.exe".to_string());
        model
    }

    /// A batch that ended with one app of each result.
    fn finished_batch() -> purifex_domain::batch::Batch {
        use purifex_domain::batch::{Batch, BatchEntry, BatchResult};
        let mut batch = Batch::new(vec![], Some("Se pidió un punto de restauración.".into()));
        batch.total = 3;
        let entry = |name: &str, result| BatchEntry {
            name: name.into(),
            result,
        };
        batch.done = vec![
            entry("Discord", BatchResult::Uninstalled),
            entry("GIMP 2.10", BatchResult::StillInstalled),
            entry("Kdenlive", BatchResult::Failed("sin desinstalador".into())),
        ];
        batch
    }

    /// Uninstalling several apps: two checked (`marked`), the question (`ask`), the first
    /// one running (`running`) or the summary (`done`).
    fn batch_state(mut model: Model, step: &str) -> Model {
        use purifex_domain::batch::Batch;
        (model.traces_open, model.uninstalled, model.traces) = (false, None, None);
        model.message = None;
        let ids = ["HKLM64:Discord", "HKLM64:GIMP 2.10"];
        model.marked = ids.map(String::from).to_vec();
        model.confirm = (step == "ask").then_some(Confirm::UninstallMarked);
        match step {
            "running" => {
                let mut batch = Batch::new(vec![ids[1].into()], None);
                batch.total = 2;
                batch.current = Some("Discord".into());
                model.batch = Some(batch);
                model.marked.clear();
                model.waiting = true;
            }
            "done" => {
                model.batch = Some(finished_batch());
                model.marked.clear();
            }
            _ => {}
        }
        model
    }

    /// Hunter mode: the sight held (`aiming`) or let go over a window of Discord.
    fn hunt_state(mut model: Model, aiming: bool) -> Model {
        model.traces_open = false;
        if aiming {
            model.hunt.aiming = true;
        } else {
            model.hunt.found = Some(purifex_domain::hunter::Hunted {
                process: purifex_domain::forced::RunningProcess {
                    pid: 4242,
                    path: r"C:\Users\yo\AppData\Local\Discord\app-1.0.9\Discord.exe".into(),
                },
                app: Some(sample_app("Discord", "Discord Inc.", 360_000)),
            });
        }
        model
    }

    /// The apps page after uninstalling Kdenlive: the traces of a real search (files, a
    /// crash dump, registry keys and values), grouped as the window lists them.
    fn kdenlive_state(mut model: Model) -> Model {
        let traces = kdenlive_traces();
        let counts = purifex_domain::trace_text::Counts::of(&traces.proposed).to_text();
        model.page = Page::Apps;
        model.uninstalled = Some("Kdenlive".into());
        model.message = Some(format!(
            "Kdenlive ya se desinstaló. Estos son los restos que dejó. {counts}"
        ));
        model.traces = Some(traces);
        model.traces_open = true;
        model
    }

    fn kdenlive_traces() -> purifex_domain::cleanup_log::CleanReport {
        let mut proposed = kdenlive_files();
        proposed.extend(kdenlive_registry());
        purifex_domain::cleanup_log::CleanReport {
            app: "Kdenlive".into(),
            preview: true,
            proposed,
            ..Default::default()
        }
    }

    /// The files of the user that Kdenlive left (user data).
    fn kdenlive_files() -> Vec<purifex_domain::app_traces::Trace> {
        use purifex_domain::app_traces::{Evidence, Trace};
        let file = |location: String, bytes: u64, evidence: Evidence| Trace {
            location,
            bytes,
            evidence,
            user_data: true,
            ..Default::default()
        };
        let local = r"C:\Users\yo\AppData\Local";
        let recent = r"C:\Users\yo\AppData\Roaming\Microsoft\Windows\Recent";
        vec![
            file(
                format!(r"{local}\CrashDumps\kdenlive.exe.33876.dmp"),
                102_844_745,
                Evidence::CrashDump,
            ),
            file(
                format!(r"{local}\CrashDumps\kdenlive.exe(1).33876.dmp"),
                102_674_430,
                Evidence::CrashDump,
            ),
            file(format!(r"{local}\kdenliverc"), 47_438, Evidence::NamePrefix),
            file(
                format!(r"{recent}\Vídeo.kdenlive.lnk"),
                1_200,
                Evidence::AssociatedExtension,
            ),
            file(
                format!(r"{local}\stalefiles\kdenlive"),
                0,
                Evidence::NameMatch,
            ),
        ]
    }

    /// The registry keys and values that Kdenlive left.
    fn kdenlive_registry() -> Vec<purifex_domain::app_traces::Trace> {
        use purifex_domain::app_traces::{Evidence, Trace, TraceKind};
        let key = |location: String, evidence: Evidence| Trace {
            kind: TraceKind::RegistryKey,
            location,
            evidence,
            ..Default::default()
        };
        let mui = r"HKCU\Software\Classes\Local Settings\Software\Microsoft\Windows\Shell\MuiCache";
        let value = |name: &str| Trace {
            kind: TraceKind::RegistryValue,
            location: mui.into(),
            value: Some(name.into()),
            evidence: Evidence::PathReference,
            ..Default::default()
        };
        let current = r"HKCU\Software\Microsoft\Windows\CurrentVersion";
        let extension = Evidence::AssociatedExtension;
        vec![
            key(r"HKLM\SOFTWARE\Classes\.kdenlive".into(), extension),
            key(
                r"HKLM\SOFTWARE\Classes\Kdenlive".into(),
                Evidence::Association,
            ),
            value(r"C:\Program Files\Kdenlive\bin\kdenlive.exe.FriendlyAppName"),
            value(r"C:\Program Files\Kdenlive\uninstall.exe.ApplicationCompany"),
            key(
                format!(r"{current}\Notifications\Settings\kdenlive"),
                Evidence::NameMatch,
            ),
            key(format!(r"{current}\Explorer\FileExts\.kdenlive"), extension),
            key(
                format!(r"{current}\Explorer\RecentDocs\.kdenlive"),
                extension,
            ),
        ]
    }

    /// The two traces that Inkscape left, as the preview lists them.
    fn inkscape_traces() -> purifex_domain::cleanup_log::CleanReport {
        use purifex_domain::app_traces::{Trace, TraceKind};
        let trace = |location: &str, bytes: u64, user: bool, why: &str| Trace {
            kind: TraceKind::Folder,
            location: location.into(),
            bytes,
            why: why.into(),
            user_data: user,
            ..Default::default()
        };
        purifex_domain::cleanup_log::CleanReport {
            app: "Inkscape".into(),
            preview: true,
            removed: vec![],
            proposed: vec![
                trace(
                    r"C:\Users\yo\AppData\Roaming\inkscape",
                    18_000_000,
                    true,
                    "lleva el nombre de la app: inkscape",
                ),
                trace(
                    r"C:\Users\yo\AppData\Local\Programs\Inkscape",
                    656_000_000,
                    false,
                    "la carpeta de instalación que registró la app",
                ),
            ],
            failed: vec![],
            ..Default::default()
        }
    }

    /// Three devices with a driver of version 1.0 each.
    fn offered_devices() -> Vec<purifex_domain::device::Device> {
        use purifex_domain::device::{Device, InstalledDriver};
        use purifex_domain::version::DriverVersion;
        let dev = |name: &str, hw: &str, class: &str| Device {
            instance_id: format!(r"PCI\{hw}"),
            name: name.into(),
            class: class.into(),
            hardware_ids: vec![format!(r"PCI\{hw}")],
            driver: Some(InstalledDriver {
                provider: Some("Fabricante".into()),
                version: DriverVersion::parse("1.0.0.0"),
                ..InstalledDriver::default()
            }),
            ..Device::default()
        };
        vec![
            dev("NVIDIA GeForce GTX 1660 Ti", "VEN_10DE&DEV_2182", "Display"),
            dev(
                "Realtek High Definition Audio",
                "VEN_10EC&DEV_0887",
                "MEDIA",
            ),
            dev("Controlador USB", "VEN_8086&DEV_1", "USB"),
        ]
    }

    /// The drivers page with updates on offer for two devices and a suggestion for a third.
    fn offers_state(mut model: Model) -> Model {
        use purifex_domain::driver_offers::{match_offers, Offer, OfferReport, Offers};
        use purifex_domain::drivers_view::DriversView;
        use purifex_domain::report::{ScanReport, Scope};
        use purifex_domain::version::DriverVersion;
        let devices = offered_devices();
        let offer = |hw: &str, v: &str| Offer {
            hardware_id: format!(r"PCI\{hw}"),
            title: format!("Fabricante - {v}"),
            provider: "Fabricante".into(),
            version: DriverVersion::parse(v),
            date: None,
        };
        let found = match_offers(
            &devices,
            &[
                offer("VEN_10DE&DEV_2182", "32.0.16.9000"),
                offer("VEN_10EC&DEV_0887", "6.0.9.7000"),
            ],
        );
        let today = purifex_domain::date::Date::new(2026, 11, 18).unwrap();
        model.page = Page::Drivers;
        model.drivers = Some(DriversView::build(&ScanReport::build(
            devices,
            Scope::Hardware,
            today,
            5,
        )));
        model.candidates = Some(vec![purifex_domain::driver_candidate::Candidate {
            device: r"PCI\VEN_8086&DEV_1".into(),
            source: purifex_domain::driver_candidate::Source::Suggested,
            url: "https://www.intel.com/download".into(),
            version: DriverVersion::parse("2.0").unwrap(),
            title: String::new(),
        }]);
        model.offers = Offers::Found(OfferReport {
            offers: found,
            unresolved: vec![],
            vendor_covered: 0,
        });
        model
    }
}

#[cfg(windows)]
fn main() {
    states::main();
}

#[cfg(not(windows))]
fn main() {}
