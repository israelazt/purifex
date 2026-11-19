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
        let (state, out) = (
            args.first().map_or("ready", String::as_str),
            args.get(1)
                .cloned()
                .unwrap_or_else(|| "ui_states.png".into()),
        );
        let mut model = Model {
            page: Page::Fixes,
            history: Some(vec![]),
            fixes: Some(vec![proposal(false)]),
            ..Model::default()
        };
        match state {
            "blocked" => model.fixes = Some(vec![proposal(true)]),
            "confirm" => model.confirm = Some(Confirm::Apply(LOL_CLOSE_CLIENT.into())),
            "history" | "undo" => {
                model.fixes = Some(vec![]);
                model.history = Some(vec![
                    entry(1, HistoryState::Applied, None),
                    entry(2, HistoryState::Undone, Some(1)),
                    entry(3, HistoryState::Applied, None),
                ]);
                model.notice = Some(FixReport {
                    fix: LOL_CLOSE_CLIENT.into(),
                    title: "Cerrar el cliente de League durante la partida".into(),
                    before: SettingValue::Bool(false),
                    after: SettingValue::Bool(true),
                    preview: false,
                    seq: Some(3),
                    undo: String::new(),
                    needs_confirmation: false,
                    was_undo: false,
                });
                if state == "undo" {
                    model.confirm = Some(Confirm::Undo(3));
                }
            }
            "offers" => {
                use purifex_domain::device::{Device, InstalledDriver};
                use purifex_domain::driver_offers::{match_offers, Offer, Offers};
                use purifex_domain::drivers_view::DriversView;
                use purifex_domain::report::{ScanReport, Scope};
                use purifex_domain::version::DriverVersion;
                let dev = |name: &str, hw: &str, class: &str| Device {
                    instance_id: format!(r"PCI\{hw}"),
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
                let devices = vec![
                    dev("NVIDIA GeForce GTX 1660 Ti", "VEN_10DE&DEV_2182", "Display"),
                    dev(
                        "Realtek High Definition Audio",
                        "VEN_10EC&DEV_0887",
                        "MEDIA",
                    ),
                    dev("Controlador USB", "VEN_8086&DEV_1", "USB"),
                ];
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
                    device: r"PCI\VEN_8086&DEV_1".into(),
                    source: purifex_domain::driver_candidate::Source::Suggested,
                    url: "https://www.intel.com/download".into(),
                    version: DriverVersion::parse("2.0").unwrap(),
                    title: String::new(),
                }]);
                model.offers = Offers::Found(purifex_domain::driver_offers::OfferReport {
                    offers: found,
                    unresolved: vec![],
                    vendor_covered: 0,
                });
            }
            _ => {}
        }
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
}

#[cfg(windows)]
fn main() {
    states::main();
}

#[cfg(not(windows))]
fn main() {}
