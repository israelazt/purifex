//! The window of Purifex: it connects the application (`Core`), the interface
//! (`Ui`) and the system's window. No decision is made here.

use std::path::Path;
use std::time::Duration;

use purifex_app::{Core, Screenshot, Services, UiOptions};
use purifex_domain::draw::DrawList;
use purifex_domain::ports::TextMetrics;
use purifex_domain::ui_event::{Reaction, UiEvent, UiHandler};
use purifex_domain::view::{Action, Page};
use purifex_ui::Ui;

/// The application and the interface, as the window sees them.
struct Handler {
    core: Core,
    ui: Ui,
}

impl UiHandler for Handler {
    fn frame(
        &mut self,
        width: f32,
        height: f32,
        text: &dyn TextMetrics,
        now: Duration,
    ) -> DrawList {
        self.ui.frame(self.core.model(), width, height, text, now)
    }

    fn animating(&self) -> bool {
        self.ui.animating()
    }

    fn set_reduced_motion(&mut self, reduced: bool) {
        self.ui.set_reduced_motion(reduced);
    }

    fn event(&mut self, event: UiEvent) -> Reaction {
        let out = self.ui.event(event);
        let acted = !out.actions.is_empty();
        for action in out.actions {
            self.core.apply(action);
        }
        Reaction {
            redraw: out.redraw || acted,
            tick: self.core.needs_tick(),
            quit: self.core.wants_quit(),
        }
    }

    fn tick(&mut self) -> Reaction {
        // Only draw again if something changed: while a search goes on in the
        // background the page is still, and nothing is repainted for nothing.
        let changed = self.core.tick();
        Reaction {
            redraw: changed,
            tick: self.core.needs_tick(),
            quit: false,
        }
    }

    fn hand_cursor(&self) -> bool {
        self.ui.hand_cursor()
    }
}

/// Opens the window, or saves a picture of it.
pub fn run(services: Box<dyn Services>, options: UiOptions) -> Result<(), String> {
    let mut handler = Handler {
        core: Core::new(services),
        ui: Ui::new(),
    };
    if let Some(name) = options.find_traces {
        resume_traces(&mut handler.core, name);
    }
    match options.screenshot {
        Some(shot) => save(&mut handler, &shot),
        None => purifex_platform::run_window("Purifex", 1440.0, 900.0, &mut handler),
    }
}

/// Opens the apps page and looks for the traces of `name`: where the program, restarted as
/// an administrator, picks up what the person was doing.
fn resume_traces(core: &mut Core, name: String) {
    core.apply(Action::Go(Page::Apps));
    while core.needs_tick() {
        core.tick();
    }
    core.apply(Action::FindTraces(name));
}

/// Reads what the page needs, then draws it to a file.
fn save(handler: &mut Handler, shot: &Screenshot) -> Result<(), String> {
    handler.core.apply(Action::Go(shot.page));
    while handler.core.needs_tick() {
        handler.core.tick();
    }
    purifex_platform::render_png(
        Path::new(&shot.path),
        shot.width,
        shot.height,
        shot.scale,
        handler,
    )
}
