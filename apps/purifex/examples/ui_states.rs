//! Draws the fixes page in states that are hard to reach on a real PC (a pending
//! confirmation, the history with an undo, a notice) from a fixed model, to check
//! the design. It changes nothing on the computer.
//! `cargo run --example ui_states -- <ready|blocked|confirm|history|undo> out.png`

#[cfg(windows)]
mod states {
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
        fn frame(&mut self, w: f32, h: f32, text: &dyn TextMetrics) -> DrawList {
            self.ui.frame(&self.model, w, h, text)
        }
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
