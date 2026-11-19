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
use crate::dialog::{self, DialogView};
use crate::scrollbar::{Pressed, Scrollbar};
use crate::theme::{self, PAGE_PAD, SIDEBAR_W, TOPBAR_H};
use crate::{apps_page, drivers_page, fixes_page, overview_page, sessions_page, shell};

/// The time the page takes to go most of the way to where the wheel wants it.
const SCROLL_MS: f32 = 70.0;
/// Units an arrow key scrolls.
const SCROLL_STEP: f32 = 64.0;

/// Whether what the page shows has been read (until then it says "Leyendo…").
fn page_is_ready(m: &Model) -> bool {
    match m.page {
        Page::Overview => m.overview.is_some(),
        Page::Drivers => m.drivers.is_some(),
        Page::Sessions => m.sessions.is_some(),
        Page::Fixes => m.fixes.is_some() && m.history.is_some(),
        Page::Apps => m.apps.is_some(),
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
    /// The message and the error shown last: when a new one appears the page goes
    /// to the top, where they are drawn (the user may be far below).
    seen_notice: (Option<String>, Option<String>),
    bar: Scrollbar,
    /// The dialog being shown (kept while it fades out, when the model no longer has it).
    dialog: Option<DialogView>,
    /// A dialog is open: it alone answers to the pointer and the keys.
    modal: bool,
    /// What Escape does while it is open.
    modal_cancel: Option<Action>,
    content_height: f32,
    viewport_height: f32,
    mouse: Option<(f32, f32)>,
    hits: Vec<Hit>,
    hovered: Option<usize>,
    /// The places with a hover effect of the last picture, and the one under the pointer.
    regions: Vec<(Rect, Option<Rect>)>,
    hover_region: Option<usize>,
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
            seen_notice: (None, None),
            bar: Scrollbar::default(),
            dialog: None,
            modal: false,
            modal_cancel: None,
            content_height: 0.0,
            viewport_height: 0.0,
            mouse: None,
            hits: Vec::new(),
            hovered: None,
            regions: Vec::new(),
            hover_region: None,
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
        let notice = (model.message.clone(), model.error.clone());
        if notice != self.seen_notice && (notice.0.is_some() || notice.1.is_some()) {
            self.scroll_target = 0.0;
        }
        self.seen_notice = notice;
        self.smooth_scroll(elapsed);
        self.query = model.query.clone();
        let anims = std::mem::take(&mut self.anims);
        let mut c = Canvas::new(text, self.mouse).with_anims(anims, self.entered_ms);
        c.search_focus = self.search_focus;
        // A dialog: it fades in and out, and while it shows nothing under it answers.
        let wanted = dialog::view(model);
        let open = wanted.is_some();
        self.modal_cancel = wanted.as_ref().map(|v| v.cancel.clone());
        if wanted.is_some() {
            self.dialog = wanted;
        }
        let shown = c.value("dialog", if open { 1.0 } else { 0.0 }, dialog::DIALOG_MS);
        let covered = shown > 0.001;
        self.modal = open;
        c.block_mouse(covered);
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
            Page::Apps => apps_page::draw(&mut c, model, area),
        };
        c.rest();
        c.unclip();
        self.content_height = used + 28.0 + 24.0;
        self.viewport_height = main.h;
        self.scroll_target = self.scroll_target.clamp(0.0, self.max_scroll());
        self.scroll = self.scroll.clamp(0.0, self.max_scroll());
        self.bar
            .layout(main, width, self.scroll, self.content_height);
        if let (Some(thumb), Some(track)) = (self.bar.thumb(), self.bar.track()) {
            let hover = if self.bar.dragging() {
                1.0
            } else {
                c.hover(track)
            };
            c.fill(thumb, 3.0, theme::TEXT_3.with_alpha(0.45 + 0.4 * hover));
        }
        shell::sidebar(&mut c, model, height);
        shell::topbar(&mut c, model, width);
        if covered {
            c.block_mouse(false);
            c.clear_hits();
            if let Some(view) = &self.dialog {
                dialog::draw(&mut c, view, shown, width, height);
            }
        } else {
            self.dialog = None;
        }
        self.search_box = c.take_search_box();
        self.anims = c.take_anims();
        self.anims.end();
        let c_regions = c.take_regions();
        let (list, hits) = c.finish();
        self.hits = hits;
        self.regions = c_regions;
        self.hover_region = self.region_at_mouse();
        self.hovered = self.hit_at_mouse();
        list
    }

    fn hit_at(&self, x: f32, y: f32) -> Option<usize> {
        self.hits.iter().rposition(|hit| {
            hit.rect.contains(x, y) && hit.clip.is_none_or(|clip| clip.contains(x, y))
        })
    }

    /// The last place with a hover effect that holds (`x`, `y`) (the one drawn on top).
    fn region_at(&self, x: f32, y: f32) -> Option<usize> {
        self.regions
            .iter()
            .rposition(|(rect, clip)| rect.contains(x, y) && clip.is_none_or(|c| c.contains(x, y)))
    }

    fn region_at_mouse(&self) -> Option<usize> {
        self.mouse.and_then(|(x, y)| self.region_at(x, y))
    }

    fn hit_at_mouse(&self) -> Option<usize> {
        self.mouse.and_then(|(x, y)| self.hit_at(x, y))
    }

    pub fn event(&mut self, event: UiEvent) -> UiOutput {
        let mut out = UiOutput::default();
        match event {
            // A dialog is open: it alone answers.
            UiEvent::Press { x, y } if self.modal => {
                self.mouse = Some((x, y));
                if let Some(index) = self.hit_at(x, y) {
                    out.actions.push(self.hits[index].action.clone());
                    out.redraw = true;
                }
            }
            UiEvent::Key(Key::Escape) if self.modal => {
                if let Some(action) = self.modal_cancel.clone() {
                    out.actions.push(action);
                    out.redraw = true;
                }
            }
            UiEvent::Wheel { .. } | UiEvent::Key(_) | UiEvent::Char(_) if self.modal => {}
            UiEvent::Move { x, y } if self.bar.dragging() => {
                self.mouse = Some((x, y));
                // Holding the thumb: the page follows the pointer at once, with no glide.
                if let Some(to) = self.bar.scroll_for(y, self.max_scroll()) {
                    (self.scroll, self.scroll_target) = (to, to);
                }
                out.redraw = true;
            }
            UiEvent::Release => out.redraw = self.bar.release(),
            UiEvent::Focus => {
                if self.page == Page::Apps {
                    out.actions.push(Action::RefreshApps);
                }
            }
            UiEvent::Key(Key::Down) => out.redraw = self.scroll_by(SCROLL_STEP),
            UiEvent::Key(Key::Up) => out.redraw = self.scroll_by(-SCROLL_STEP),
            UiEvent::Key(Key::PageDown) => out.redraw = self.scroll_by(self.viewport_height * 0.9),
            UiEvent::Key(Key::PageUp) => out.redraw = self.scroll_by(-self.viewport_height * 0.9),
            UiEvent::Key(Key::Home) => out.redraw = self.scroll_by(-self.content_height),
            UiEvent::Key(Key::End) => out.redraw = self.scroll_by(self.content_height),
            UiEvent::Move { x, y } => {
                self.mouse = Some((x, y));
                let now = self.hit_at(x, y);
                let region = self.region_at(x, y);
                // The picture changes when the pointer goes to another button, or to
                // another place with a hover effect (a row of a table).
                out.redraw = now != self.hovered || region != self.hover_region;
                (self.hovered, self.hover_region) = (now, region);
            }
            UiEvent::Leave => {
                self.mouse = None;
                out.redraw = self.hovered.take().is_some() | self.hover_region.take().is_some();
            }
            UiEvent::Press { x, y } => {
                self.mouse = Some((x, y));
                match self.bar.press(x, y) {
                    // On the scroll bar: the thumb is taken, or the page goes a screen.
                    Some(Pressed::Grabbed) => out.redraw = true,
                    Some(Pressed::Page(direction)) => {
                        out.redraw = self.scroll_by(direction * self.viewport_height * 0.9);
                    }
                    None => {
                        if let Some(index) = self.hit_at(x, y) {
                            out.actions.push(self.hits[index].action.clone());
                        }
                        let focus = self.over_search();
                        out.redraw = focus != self.search_focus || !out.actions.is_empty();
                        self.search_focus = focus;
                    }
                }
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
                if self.page != Page::Drivers && self.page != Page::Apps {
                    out.actions.push(Action::Go(Page::Drivers));
                }
                self.search_focus = true;
                out.redraw = true;
            }
            _ => {}
        }
        out
    }

    /// Moves where the page wants to be by `dy`; whether that changed anything.
    fn scroll_by(&mut self, dy: f32) -> bool {
        let before = self.scroll_target;
        self.scroll_target = (self.scroll_target + dy).clamp(0.0, self.max_scroll());
        self.scroll_target != before
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

    #[test]
    fn a_new_message_or_error_takes_the_page_to_the_top_where_it_is_drawn() {
        let mut model = tall_model();
        let mut ui = Ui::new();
        frame_at(&mut ui, &model, 0);
        ui.event(UiEvent::Wheel { dy: 600.0 });
        assert_eq!(ui.scroll_target, 600.0);
        frame_at(&mut ui, &model, 16);
        assert_eq!(
            ui.scroll_target, 600.0,
            "nothing new: it stays where the user is"
        );
        model.error = Some("algo falló".into());
        frame_at(&mut ui, &model, 32);
        assert_eq!(ui.scroll_target, 0.0, "the error is at the top: go there");
        ui.event(UiEvent::Wheel { dy: 300.0 });
        frame_at(&mut ui, &model, 48);
        assert_eq!(
            ui.scroll_target, 300.0,
            "the same error does not pull the page up again"
        );
        model.message = Some("hecho".into());
        frame_at(&mut ui, &model, 64);
        assert_eq!(ui.scroll_target, 0.0);
    }

    #[test]
    fn the_keys_scroll_a_line_a_page_or_to_either_end_and_never_past_them() {
        let model = tall_model();
        let mut ui = Ui::new();
        frame_at(&mut ui, &model, 0);
        let page = ui.viewport_height * 0.9;
        assert!(ui.event(UiEvent::Key(Key::Down)).redraw);
        assert_eq!(ui.scroll_target, 64.0);
        ui.event(UiEvent::Key(Key::PageDown));
        assert!((ui.scroll_target - (64.0 + page)).abs() < 0.01);
        ui.event(UiEvent::Key(Key::Up));
        assert!((ui.scroll_target - (page)).abs() < 0.01);
        ui.event(UiEvent::Key(Key::PageUp));
        assert_eq!(ui.scroll_target, 0.0);
        assert!(
            !ui.event(UiEvent::Key(Key::Up)).redraw,
            "already at the top"
        );
        let end = ui.event(UiEvent::Key(Key::End));
        assert!(end.redraw);
        assert_eq!(ui.scroll_target, ui.max_scroll());
        assert!(
            !ui.event(UiEvent::Key(Key::PageDown)).redraw,
            "already at the end"
        );
        ui.event(UiEvent::Key(Key::Home));
        assert_eq!(ui.scroll_target, 0.0);
    }

    #[test]
    fn the_thumb_can_be_dragged_and_the_page_follows_at_once_until_it_is_let_go() {
        let model = tall_model();
        let mut ui = Ui::new();
        frame_at(&mut ui, &model, 0);
        let thumb = ui.bar.thumb().expect("a long page has a bar");
        let (x, y) = (thumb.x + 2.0, thumb.y + 8.0);
        assert!(
            ui.event(UiEvent::Press { x, y }).actions.is_empty(),
            "the bar is not a button"
        );
        assert!(ui.bar.dragging());
        ui.event(UiEvent::Move { x, y: y + 150.0 });
        assert!(ui.scroll > 0.0, "the page moved with the thumb");
        assert_eq!(ui.scroll, ui.scroll_target, "no glide while it is held");
        let held_at = ui.scroll;
        // The pointer may leave the bar sideways: the thumb is still held.
        ui.event(UiEvent::Move {
            x: 300.0,
            y: y + 300.0,
        });
        assert!(ui.scroll > held_at);
        assert!(ui.event(UiEvent::Release).redraw);
        assert!(!ui.bar.dragging());
        let after = ui.scroll;
        ui.event(UiEvent::Move {
            x: 300.0,
            y: y + 500.0,
        });
        assert_eq!(
            ui.scroll, after,
            "let go: the pointer no longer moves the page"
        );
        assert!(!ui.event(UiEvent::Release).redraw, "nothing was held");
    }

    #[test]
    fn clicking_the_track_goes_a_page_toward_the_click() {
        let model = tall_model();
        let mut ui = Ui::new();
        frame_at(&mut ui, &model, 0);
        let (thumb, track) = (ui.bar.thumb().unwrap(), ui.bar.track().unwrap());
        let page = ui.viewport_height * 0.9;
        ui.event(UiEvent::Press {
            x: track.x + 4.0,
            y: thumb.bottom() + 30.0,
        });
        assert!((ui.scroll_target - page).abs() < 0.01);
        assert!(!ui.bar.dragging());
        frame_at(&mut ui, &model, 400);
        let thumb = ui.bar.thumb().unwrap();
        ui.event(UiEvent::Press {
            x: track.x + 4.0,
            y: thumb.y - 10.0,
        });
        assert!(ui.scroll_target < 1.0, "back up a page");
    }

    #[test]
    fn moving_over_a_row_of_a_table_redraws_once_per_row_and_not_only_over_its_button() {
        let model = tall_model();
        let mut ui = Ui::new();
        frame_at(&mut ui, &model, 0);
        let rows: Vec<Rect> = ui
            .regions
            .iter()
            .map(|(r, _)| *r)
            .filter(|r| r.w > 800.0 && r.h > 40.0 && r.h < 100.0)
            .take(2)
            .collect();
        assert_eq!(rows.len(), 2, "the table has rows with a hover effect");
        let (a, b) = (rows[0], rows[1]);
        // Far from any button: the middle of the row.
        let inside = |r: Rect, dx: f32| UiEvent::Move {
            x: r.x + 400.0 + dx,
            y: r.y + r.h / 2.0,
        };
        assert!(ui.event(inside(a, 0.0)).redraw, "entering a row");
        assert!(
            !ui.event(inside(a, 30.0)).redraw,
            "moving inside the same row"
        );
        assert!(ui.event(inside(b, 0.0)).redraw, "going to the next row");
        assert!(ui.event(UiEvent::Leave).redraw, "leaving the window");
        assert!(!ui.event(UiEvent::Leave).redraw);
    }

    /// The apps page with two apps, optionally with a dialog open.
    fn apps_model(confirm: Option<purifex_domain::view::Confirm>) -> Model {
        let app = |name: &str| purifex_domain::apps::InstalledApp {
            id: format!("HKLM64:{name}"),
            name: name.into(),
            publisher: Some("Acme".into()),
            version: None,
            install_location: None,
            uninstall: Some("u.exe".into()),
            quiet_uninstall: None,
            size_kb: None,
            kind: purifex_domain::apps::AppKind::Desktop,
            hidden: false,
            display_icon: None,
            observed: Default::default(),
        };
        Model {
            page: Page::Apps,
            apps: Some(vec![app("Discord"), app("Git")]),
            confirm,
            ..Model::default()
        }
    }

    fn click_on(ui: &mut Ui, action: &Action) -> Vec<Action> {
        let rect = ui
            .hits
            .iter()
            .find(|h| h.action == *action)
            .unwrap_or_else(|| panic!("no {action:?}"))
            .rect;
        ui.event(UiEvent::Press {
            x: rect.x + 4.0,
            y: rect.y + 4.0,
        })
        .actions
    }

    #[test]
    fn while_a_dialog_is_open_only_it_answers_and_escape_cancels_it() {
        use purifex_domain::view::Confirm;
        let page = apps_model(None);
        let asking = apps_model(Some(Confirm::Uninstall("HKLM64:Discord".into())));
        let mut ui = Ui::new();
        frame_at(&mut ui, &page, 0);
        // Without the dialog the page's own button answers.
        let ask = Action::AskUninstall("HKLM64:Git".into());
        assert_eq!(click_on(&mut ui, &ask), std::slice::from_ref(&ask));
        // With it open, that same button is under the dialog: it does nothing.
        for ms in (16..=400).step_by(16) {
            frame_at(&mut ui, &asking, ms);
        }
        assert!(
            ui.hits.iter().all(|h| h.action != ask),
            "what is under the dialog is not clickable"
        );
        assert_eq!(click_on(&mut ui, &Action::Confirm), [Action::Confirm]);
        assert_eq!(
            click_on(&mut ui, &Action::CancelConfirm),
            [Action::CancelConfirm]
        );
        // The wheel and the keys do not move the page behind it; Escape cancels.
        assert!(!ui.event(UiEvent::Wheel { dy: 200.0 }).redraw);
        assert!(!ui.event(UiEvent::Key(Key::PageDown)).redraw);
        assert_eq!(
            ui.event(UiEvent::Key(Key::Escape)).actions,
            [Action::CancelConfirm]
        );
    }

    #[test]
    fn a_dialog_fades_in_and_out_and_the_window_stops_asking_for_pictures_when_it_is_gone() {
        use purifex_domain::view::Confirm;
        let page = apps_model(None);
        let asking = apps_model(Some(Confirm::Uninstall("HKLM64:Discord".into())));
        let mut ui = Ui::new();
        for ms in (0..=800).step_by(16) {
            frame_at(&mut ui, &page, ms);
        }
        assert!(!ui.animating());
        let has_dialog = |ops: &[Op]| {
            ops.iter().any(
                |o| matches!(o, Op::Text { text, .. } if text.contains("¿Desinstalar Discord?")),
            )
        };
        frame_at(&mut ui, &asking, 1000);
        assert!(ui.animating(), "it starts to appear");
        let first = frame_at(&mut ui, &asking, 1016);
        assert!(has_dialog(&first), "a frame later it is there, still faint");
        let alpha = |ops: &[Op]| {
            ops.iter().find_map(|o| match o {
                Op::Text { text, style, .. } if text.contains("¿Desinstalar Discord?") => {
                    Some(style.color.a)
                }
                _ => None,
            })
        };
        let early = alpha(&first).unwrap();
        let settled = alpha(&frame_at(&mut ui, &asking, 1400)).unwrap();
        assert!(early < settled && settled == 255, "{early} -> {settled}");
        // The model closes it: it is still drawn while it fades, then gone.
        let fading = frame_at(&mut ui, &page, 1416);
        assert!(
            has_dialog(&fading),
            "kept from the last view while it fades out"
        );
        assert!(ui.animating());
        let gone = frame_at(&mut ui, &page, 2000);
        assert!(!has_dialog(&gone));
        assert!(
            !ui.animating(),
            "nothing moves: the window waits for messages again"
        );
    }

    #[test]
    fn with_animations_off_a_dialog_is_there_at_once_and_gone_at_once() {
        use purifex_domain::view::Confirm;
        let mut ui = Ui::new();
        ui.set_reduced_motion(true);
        let asking = apps_model(Some(Confirm::Uninstall("HKLM64:Discord".into())));
        let has = |ops: &[Op]| {
            ops.iter().any(|o| matches!(o, Op::Text { text, style, .. } if text.contains("¿Desinstalar") && style.color.a == 255))
        };
        assert!(has(&frame_at(&mut ui, &asking, 0)));
        assert!(!has(&frame_at(&mut ui, &apps_model(None), 16)));
    }
}
