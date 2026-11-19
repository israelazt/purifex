//! The interface as a whole: it draws the window from the model and turns the
//! mouse and the keys into actions. It keeps only what belongs to looking
//! (where the page is scrolled, where the pointer is, whether the search has the
//! keyboard); everything else is in the model, which the application owns.

use std::time::Duration;

use purifex_domain::draw::{DrawList, Rect};
use purifex_domain::ports::TextMetrics;
use purifex_domain::ui_event::{Key, UiEvent};
use purifex_domain::view::{Action, Model, Page};

use crate::anim::Anims;
use crate::canvas::{Canvas, Hit};
use crate::theme::{self, PAGE_PAD, SIDEBAR_W, TOPBAR_H};
use crate::{drivers_page, fixes_page, overview_page, sessions_page, shell};

/// The time the page takes to go most of the way to where the wheel wants it.
const SCROLL_MS: f32 = 70.0;

/// Whether what the page shows has been read (until then it says "Leyendo…").
fn page_is_ready(m: &Model) -> bool {
    match m.page {
        Page::Overview => m.overview.is_some(),
        Page::Drivers => m.drivers.is_some(),
        Page::Sessions => m.sessions.is_some(),
        Page::Fixes => m.fixes.is_some() && m.history.is_some(),
    }
}

/// What an event asks of the window: draw again, and these actions.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct UiOutput {
    pub redraw: bool,
    pub actions: Vec<Action>,
}

pub struct Ui {
    page: Page,
    /// Where the page is scrolled to now, and where the wheel wants it to be (it
    /// goes there smoothly).
    scroll: f32,
    scroll_target: f32,
    anims: Anims,
    /// When the content of the page last began to appear (a new page, or its data
    /// just arrived).
    entered_ms: u64,
    was_ready: bool,
    last_ms: u64,
    content_height: f32,
    viewport_height: f32,
    mouse: Option<(f32, f32)>,
    hits: Vec<Hit>,
    hovered: Option<usize>,
    search_box: Option<Rect>,
    search_focus: bool,
    query: String,
}

impl Default for Ui {
    fn default() -> Self {
        Self::new()
    }
}

impl Ui {
    pub fn new() -> Self {
        Self {
            page: Page::Overview,
            scroll: 0.0,
            scroll_target: 0.0,
            anims: Anims::new(),
            entered_ms: 0,
            was_ready: false,
            last_ms: 0,
            content_height: 0.0,
            viewport_height: 0.0,
            mouse: None,
            hits: Vec::new(),
            hovered: None,
            search_box: None,
            search_focus: false,
            query: String::new(),
        }
    }

    /// Whether the pointer is over something that can be clicked.
    pub fn hand_cursor(&self) -> bool {
        self.hovered.is_some() || self.over_search()
    }

    fn over_search(&self) -> bool {
        match (self.mouse, self.search_box, self.main_area()) {
            (Some((x, y)), Some(search), Some(main)) => {
                search.contains(x, y) && main.contains(x, y)
            }
            _ => false,
        }
    }

    fn main_area(&self) -> Option<Rect> {
        (self.viewport_height > 0.0)
            .then(|| Rect::new(SIDEBAR_W, TOPBAR_H, f32::MAX / 4.0, self.viewport_height))
    }

    fn max_scroll(&self) -> f32 {
        (self.content_height - self.viewport_height).max(0.0)
    }

    /// The picture of the window at `width` x `height` units.
    pub fn frame(
        &mut self,
        model: &Model,
        width: f32,
        height: f32,
        text: &dyn TextMetrics,
        now: Duration,
    ) -> DrawList {
        let now_ms = now.as_millis() as u64;
        self.anims.begin(now_ms);
        let elapsed = now_ms.saturating_sub(self.last_ms);
        self.last_ms = now_ms;
        let ready = page_is_ready(model);
        if model.page != self.page {
            self.page = model.page;
            (self.scroll, self.scroll_target) = (0.0, 0.0);
            self.entered_ms = now_ms;
        } else if ready && !self.was_ready {
            // What the page shows has just arrived: it appears now.
            self.entered_ms = now_ms;
        }
        self.was_ready = ready;
        self.smooth_scroll(elapsed);
        self.query = model.query.clone();
        let anims = std::mem::take(&mut self.anims);
        let mut c = Canvas::new(text, self.mouse).with_anims(anims, self.entered_ms);
        c.search_focus = self.search_focus;
        let main = Rect::new(
            SIDEBAR_W,
            TOPBAR_H,
            (width - SIDEBAR_W).max(0.0),
            (height - TOPBAR_H).max(0.0),
        );
        let top = main.y + 28.0;
        let area = Rect::new(
            main.x + PAGE_PAD,
            top - self.scroll,
            (main.w - 2.0 * PAGE_PAD).max(200.0),
            0.0,
        );
        c.clip(main);
        c.reveal(0);
        let used = match model.page {
            Page::Overview => overview_page::draw(&mut c, model, area),
            Page::Drivers => drivers_page::draw(&mut c, model, area),
            Page::Sessions => sessions_page::draw(&mut c, model, area),
            Page::Fixes => fixes_page::draw(&mut c, model, area),
        };
        c.rest();
        c.unclip();
        self.content_height = used + 28.0 + 24.0;
        self.viewport_height = main.h;
        self.scroll_target = self.scroll_target.clamp(0.0, self.max_scroll());
        self.scroll = self.scroll.clamp(0.0, self.max_scroll());
        if self.max_scroll() > 0.0 {
            let ratio = self.viewport_height / self.content_height;
            let thumb = Rect::new(
                width - 8.0,
                main.y + self.scroll / self.content_height * main.h + 2.0,
                4.0,
                (main.h * ratio - 4.0).max(24.0),
            );
            c.fill(thumb, 2.0, theme::TEXT_3.with_alpha(0.55));
        }
        shell::sidebar(&mut c, model, height);
        shell::topbar(&mut c, model, width);
        self.search_box = c.take_search_box();
        self.anims = c.take_anims();
        self.anims.end();
        let (list, hits) = c.finish();
        self.hits = hits;
        self.hovered = self.hit_at_mouse();
        list
    }

    fn hit_at(&self, x: f32, y: f32) -> Option<usize> {
        self.hits.iter().rposition(|hit| {
            hit.rect.contains(x, y) && hit.clip.is_none_or(|clip| clip.contains(x, y))
        })
    }

    fn hit_at_mouse(&self) -> Option<usize> {
        self.mouse.and_then(|(x, y)| self.hit_at(x, y))
    }

    pub fn event(&mut self, event: UiEvent) -> UiOutput {
        let mut out = UiOutput::default();
        match event {
            UiEvent::Move { x, y } => {
                self.mouse = Some((x, y));
                let now = self.hit_at(x, y);
                out.redraw = now != self.hovered;
                self.hovered = now;
            }
            UiEvent::Leave => {
                self.mouse = None;
                out.redraw = self.hovered.take().is_some();
            }
            UiEvent::Press { x, y } => {
                self.mouse = Some((x, y));
                if let Some(index) = self.hit_at(x, y) {
                    out.actions.push(self.hits[index].action.clone());
                }
                let focus = self.over_search();
                out.redraw = focus != self.search_focus || !out.actions.is_empty();
                self.search_focus = focus;
            }
            UiEvent::Wheel { dy } => {
                let before = self.scroll_target;
                self.scroll_target = (self.scroll_target + dy).clamp(0.0, self.max_scroll());
                out.redraw = self.scroll_target != before;
            }
            UiEvent::Char(c) if self.search_focus => {
                let query = format!("{}{c}", self.query);
                out.actions.push(self.typed(query));
                out.redraw = true;
            }
            UiEvent::Key(Key::Backspace) if self.search_focus => {
                let mut query = self.query.clone();
                query.pop();
                out.actions.push(self.typed(query));
                out.redraw = true;
            }
            UiEvent::Key(Key::Escape) if self.search_focus => {
                if self.query.is_empty() {
                    self.search_focus = false;
                } else {
                    out.actions.push(self.typed(String::new()));
                }
                out.redraw = true;
            }
            UiEvent::Key(Key::Enter) if self.search_focus => {
                self.search_focus = false;
                out.redraw = true;
            }
            UiEvent::Key(Key::CtrlK) => {
                if self.page != Page::Drivers {
                    out.actions.push(Action::Go(Page::Drivers));
                }
                self.search_focus = true;
                out.redraw = true;
            }
            _ => {}
        }
        out
    }

    /// The search text after a key, remembered at once: several keys can arrive
    /// before the next picture (a paste, a held key), and each must add to the
    /// last one, not to what the last picture showed.
    fn typed(&mut self, query: String) -> Action {
        self.query = query.clone();
        Action::SetQuery(query)
    }

    /// Whether something is still moving (the window asks for another picture).
    pub fn animating(&self) -> bool {
        self.anims.active() || (self.scroll - self.scroll_target).abs() > 0.5
    }

    /// Windows has animations off: nothing moves, everything is at its place at once.
    pub fn set_reduced_motion(&mut self, reduced: bool) {
        self.anims.set_motion(!reduced);
        if reduced {
            self.scroll = self.scroll_target;
        }
    }

    /// The page goes toward where the wheel wants it, a fraction of the way each
    /// picture (the same speed whatever the pace of the pictures).
    fn smooth_scroll(&mut self, elapsed_ms: u64) {
        if !self.anims.motion() {
            self.scroll = self.scroll_target;
            return;
        }
        let k = 1.0 - (-(elapsed_ms as f32) / SCROLL_MS).exp();
        self.scroll += (self.scroll_target - self.scroll) * k;
        if (self.scroll_target - self.scroll).abs() < 0.5 {
            self.scroll = self.scroll_target;
        } else {
            self.anims.keep_going();
        }
    }

    /// Whether the search has the keyboard (the page draws its caret).
    pub fn search_focused(&self) -> bool {
        self.search_focus
    }
}

#[cfg(test)]
mod tests {
    use purifex_domain::draw::Op;

    use super::*;
    use crate::canvas::testing::FakeText;

    fn ui_with_frame(model: &Model) -> Ui {
        let mut ui = Ui::new();
        ui.frame(model, 1440.0, 900.0, &FakeText, Duration::ZERO);
        ui
    }

    fn press(ui: &mut Ui, x: f32, y: f32) -> UiOutput {
        ui.event(UiEvent::Press { x, y })
    }

    #[test]
    fn clicking_a_page_of_the_navigation_asks_to_go_there() {
        let model = Model::default();
        let mut ui = ui_with_frame(&model);
        // The Drivers entry: below the engine card and the "NAVEGACIÓN" label.
        let drivers = ui
            .hits
            .iter()
            .find(|h| h.action == Action::Go(Page::Drivers))
            .unwrap()
            .rect;
        let out = press(&mut ui, drivers.x + 20.0, drivers.y + 10.0);
        assert_eq!(out.actions, [Action::Go(Page::Drivers)]);
        assert!(out.redraw);
    }

    #[test]
    fn a_click_on_nothing_does_nothing() {
        let mut ui = ui_with_frame(&Model::default());
        let out = press(&mut ui, 700.0, 880.0);
        assert!(out.actions.is_empty());
    }

    #[test]
    fn the_hand_shows_over_what_can_be_clicked_and_a_move_redraws_only_when_it_changes() {
        let mut ui = ui_with_frame(&Model::default());
        let nav = ui
            .hits
            .iter()
            .find(|h| h.action == Action::Go(Page::Sessions))
            .unwrap()
            .rect;
        assert!(!ui.hand_cursor());
        let first = ui.event(UiEvent::Move {
            x: nav.x + 5.0,
            y: nav.y + 5.0,
        });
        assert!(first.redraw && ui.hand_cursor());
        let same = ui.event(UiEvent::Move {
            x: nav.x + 9.0,
            y: nav.y + 6.0,
        });
        assert!(!same.redraw, "still on the same place");
        assert!(ui.event(UiEvent::Leave).redraw);
        assert!(!ui.hand_cursor());
    }

    #[test]
    fn the_wheel_asks_for_a_place_within_the_content_and_never_past_its_ends() {
        let mut ui = ui_with_frame(&Model::default());
        // A short page does not scroll.
        assert!(!ui.event(UiEvent::Wheel { dy: 100.0 }).redraw);
        ui.content_height = 2000.0;
        assert!(ui.event(UiEvent::Wheel { dy: 100.0 }).redraw);
        assert_eq!(ui.scroll_target, 100.0);
        ui.event(UiEvent::Wheel { dy: 99999.0 });
        assert_eq!(ui.scroll_target, 2000.0 - ui.viewport_height);
        ui.event(UiEvent::Wheel { dy: -99999.0 });
        assert_eq!(ui.scroll_target, 0.0);
    }

    /// A frame of `model` at `ms` milliseconds.
    fn frame_at(ui: &mut Ui, model: &Model, ms: u64) -> Vec<Op> {
        ui.frame(model, 1440.0, 900.0, &FakeText, Duration::from_millis(ms))
            .ops
    }

    /// The drivers page with 80 devices: taller than the window, so it scrolls.
    fn tall_model() -> Model {
        let devices = (0..80)
            .map(|i| purifex_domain::device::Device {
                instance_id: format!(r"PCI\VEN_{i}"),
                class: "System".into(),
                name: format!("Dispositivo {i}"),
                ..Default::default()
            })
            .collect();
        let scan = purifex_domain::report::ScanReport::build(
            devices,
            purifex_domain::report::Scope::Hardware,
            purifex_domain::date::Date::new(2026, 11, 18).unwrap(),
            5,
        );
        Model {
            page: Page::Drivers,
            drivers: Some(purifex_domain::drivers_view::DriversView::build(&scan)),
            ..Model::default()
        }
    }

    #[test]
    fn the_page_glides_to_where_the_wheel_asked_and_the_window_stops_asking_for_pictures() {
        let model = tall_model();
        let mut ui = Ui::new();
        frame_at(&mut ui, &model, 0);
        for ms in (16..=1000).step_by(16) {
            frame_at(&mut ui, &model, ms);
        }
        assert!(!ui.animating(), "the page has settled before the wheel");
        ui.event(UiEvent::Wheel { dy: 400.0 });
        assert!(ui.animating(), "it has not got there yet");
        frame_at(&mut ui, &model, 1016);
        assert!(
            ui.scroll > 0.0 && ui.scroll < 400.0,
            "on its way: {}",
            ui.scroll
        );
        assert!(ui.animating());
        for ms in (1032..=3000).step_by(16) {
            frame_at(&mut ui, &model, ms);
        }
        assert_eq!(ui.scroll, 400.0);
        assert!(
            !ui.animating(),
            "at rest: the window waits for messages again and costs no CPU"
        );
    }

    #[test]
    fn with_animations_off_the_page_goes_at_once() {
        let model = tall_model();
        let mut ui = Ui::new();
        ui.set_reduced_motion(true);
        frame_at(&mut ui, &model, 0);
        ui.event(UiEvent::Wheel { dy: 400.0 });
        frame_at(&mut ui, &model, 1);
        assert_eq!(ui.scroll, 400.0);
        assert!(!ui.animating());
    }

    /// The opacity of the title of the page in a picture (255 is fully drawn).
    fn title_alpha(ops: &[Op], title: &str) -> u8 {
        ops.iter()
            .find_map(|op| match op {
                Op::Text { text, style, .. } if text == title => Some(style.color.a),
                _ => None,
            })
            .unwrap_or_else(|| panic!("no «{title}» in the picture"))
    }

    #[test]
    fn a_page_fades_in_when_its_data_arrives_and_is_fully_drawn_when_it_ends() {
        let mut ui = Ui::new();
        let waiting = Model {
            busy: true,
            ..Model::default()
        };
        frame_at(&mut ui, &waiting, 0);
        let scan = purifex_domain::report::ScanReport::build(
            vec![],
            purifex_domain::report::Scope::Hardware,
            purifex_domain::date::Date::new(2026, 11, 18).unwrap(),
            5,
        );
        let ready = Model {
            overview: Some(purifex_domain::overview::Overview::build(&scan, None)),
            ..Model::default()
        };
        let first = frame_at(&mut ui, &ready, 1000);
        assert!(
            title_alpha(&first, "Estado de tu PC") < 40,
            "it starts almost clear: {}",
            title_alpha(&first, "Estado de tu PC")
        );
        assert!(ui.animating());
        let middle = frame_at(&mut ui, &ready, 1100);
        let (a, b) = (
            title_alpha(&first, "Estado de tu PC"),
            title_alpha(&middle, "Estado de tu PC"),
        );
        assert!(b > a && b < 255, "{a} -> {b}");
        let mid_cascade = frame_at(&mut ui, &ready, 1000 + 400);
        assert_eq!(
            title_alpha(&mid_cascade, "Estado de tu PC"),
            255,
            "the title is there"
        );
        assert!(
            ui.animating(),
            "the figures are still counting and the bar filling"
        );
        // The cascade, the figures and the bar all take under a second.
        let end = frame_at(&mut ui, &ready, 1000 + 1500);
        assert_eq!(title_alpha(&end, "Estado de tu PC"), 255);
        assert!(
            !ui.animating(),
            "everything settled: the window can wait for messages"
        );
    }

    #[test]
    fn the_side_bar_and_the_top_bar_never_fade() {
        let mut ui = Ui::new();
        let ops = frame_at(&mut ui, &Model::default(), 0);
        assert_eq!(title_alpha(&ops, "PURIFEX"), 255);
        assert_eq!(title_alpha(&ops, "Estado general"), 255);
    }

    #[test]
    fn changing_page_starts_at_the_top() {
        let mut ui = ui_with_frame(&Model::default());
        ui.content_height = 2000.0;
        ui.event(UiEvent::Wheel { dy: 300.0 });
        assert_eq!(ui.scroll_target, 300.0);
        ui.frame(
            &Model {
                page: Page::Drivers,
                ..Model::default()
            },
            1440.0,
            900.0,
            &FakeText,
            Duration::ZERO,
        );
        assert_eq!((ui.scroll, ui.scroll_target), (0.0, 0.0));
    }

    #[test]
    fn ctrl_k_goes_to_the_drivers_and_gives_the_keyboard_to_the_search() {
        let mut ui = ui_with_frame(&Model::default());
        let out = ui.event(UiEvent::Key(Key::CtrlK));
        assert_eq!(out.actions, [Action::Go(Page::Drivers)]);
        assert!(ui.search_focused());
        let on_drivers = Model {
            page: Page::Drivers,
            ..Model::default()
        };
        let mut ui = ui_with_frame(&on_drivers);
        assert!(
            ui.event(UiEvent::Key(Key::CtrlK)).actions.is_empty(),
            "already there"
        );
        assert!(ui.search_focused());
    }

    #[test]
    fn keys_that_arrive_before_the_next_picture_all_count() {
        let model = Model {
            page: Page::Drivers,
            ..Model::default()
        };
        let mut ui = ui_with_frame(&model);
        ui.event(UiEvent::Key(Key::CtrlK));
        // No frame between them: what a paste or a held key does.
        let typed: Vec<Action> = "nvidia"
            .chars()
            .flat_map(|c| ui.event(UiEvent::Char(c)).actions)
            .collect();
        assert_eq!(typed.last(), Some(&Action::SetQuery("nvidia".into())));
        assert_eq!(typed[2], Action::SetQuery("nvi".into()));
        let erased: Vec<Action> = (0..3)
            .flat_map(|_| ui.event(UiEvent::Key(Key::Backspace)).actions)
            .collect();
        assert_eq!(erased.last(), Some(&Action::SetQuery("nvi".into())));
    }

    #[test]
    fn typing_goes_to_the_search_only_while_it_has_the_keyboard() {
        let model = Model {
            page: Page::Drivers,
            query: "nv".into(),
            ..Model::default()
        };
        let mut ui = ui_with_frame(&model);
        assert!(
            ui.event(UiEvent::Char('i')).actions.is_empty(),
            "nobody is listening"
        );
        ui.event(UiEvent::Key(Key::CtrlK));
        assert_eq!(
            ui.event(UiEvent::Char('i')).actions,
            [Action::SetQuery("nvi".into())]
        );
        // The text it just asked for is what the next key works on: "nvi" loses its last letter.
        assert_eq!(
            ui.event(UiEvent::Key(Key::Backspace)).actions,
            [Action::SetQuery("nv".into())]
        );
        assert_eq!(
            ui.event(UiEvent::Key(Key::Escape)).actions,
            [Action::SetQuery(String::new())],
            "the first Esc clears"
        );
        let empty = Model {
            page: Page::Drivers,
            ..Model::default()
        };
        let mut ui = ui_with_frame(&empty);
        ui.event(UiEvent::Key(Key::CtrlK));
        assert!(ui.event(UiEvent::Key(Key::Escape)).actions.is_empty());
        assert!(!ui.search_focused(), "the second one leaves it");
    }
}
