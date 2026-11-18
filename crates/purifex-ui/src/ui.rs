//! The interface as a whole: it draws the window from the model and turns the
//! mouse and the keys into actions. It keeps only what belongs to looking
//! (where the page is scrolled, where the pointer is, whether the search has the
//! keyboard); everything else is in the model, which the application owns.

use purifex_domain::draw::{DrawList, Rect};
use purifex_domain::ports::TextMetrics;
use purifex_domain::ui_event::{Key, UiEvent};
use purifex_domain::view::{Action, Model, Page};

use crate::canvas::{Canvas, Hit};
use crate::theme::{self, PAGE_PAD, SIDEBAR_W, TOPBAR_H};
use crate::{drivers_page, overview_page, sessions_page, shell};

/// What an event asks of the window: draw again, and these actions.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct UiOutput {
    pub redraw: bool,
    pub actions: Vec<Action>,
}

pub struct Ui {
    page: Page,
    scroll: f32,
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
    ) -> DrawList {
        if model.page != self.page {
            self.page = model.page;
            self.scroll = 0.0;
        }
        self.query = model.query.clone();
        let mut c = Canvas::new(text, self.mouse);
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
        let used = match model.page {
            Page::Overview => overview_page::draw(&mut c, model, area),
            Page::Drivers => drivers_page::draw(&mut c, model, area),
            Page::Sessions => sessions_page::draw(&mut c, model, area),
        };
        c.unclip();
        self.content_height = used + 28.0 + 24.0;
        self.viewport_height = main.h;
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
                let before = self.scroll;
                self.scroll = (self.scroll + dy).clamp(0.0, self.max_scroll());
                out.redraw = self.scroll != before;
            }
            UiEvent::Char(c) if self.search_focus => {
                out.actions
                    .push(Action::SetQuery(format!("{}{c}", self.query)));
                out.redraw = true;
            }
            UiEvent::Key(Key::Backspace) if self.search_focus => {
                let mut query = self.query.clone();
                query.pop();
                out.actions.push(Action::SetQuery(query));
                out.redraw = true;
            }
            UiEvent::Key(Key::Escape) if self.search_focus => {
                if self.query.is_empty() {
                    self.search_focus = false;
                } else {
                    out.actions.push(Action::SetQuery(String::new()));
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

    /// Whether the search has the keyboard (the page draws its caret).
    pub fn search_focused(&self) -> bool {
        self.search_focus
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::canvas::testing::FakeText;

    fn ui_with_frame(model: &Model) -> Ui {
        let mut ui = Ui::new();
        ui.frame(model, 1440.0, 900.0, &FakeText);
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
    fn the_wheel_scrolls_within_the_content_and_never_past_its_ends() {
        let mut ui = ui_with_frame(&Model::default());
        // A short page does not scroll.
        assert!(!ui.event(UiEvent::Wheel { dy: 100.0 }).redraw);
        ui.content_height = 2000.0;
        assert!(ui.event(UiEvent::Wheel { dy: 100.0 }).redraw);
        assert_eq!(ui.scroll, 100.0);
        ui.event(UiEvent::Wheel { dy: 99999.0 });
        assert_eq!(ui.scroll, 2000.0 - ui.viewport_height);
        ui.event(UiEvent::Wheel { dy: -99999.0 });
        assert_eq!(ui.scroll, 0.0);
    }

    #[test]
    fn changing_page_starts_at_the_top() {
        let mut ui = ui_with_frame(&Model::default());
        ui.content_height = 2000.0;
        ui.event(UiEvent::Wheel { dy: 300.0 });
        assert_eq!(ui.scroll, 300.0);
        ui.frame(
            &Model {
                page: Page::Drivers,
                ..Model::default()
            },
            1440.0,
            900.0,
            &FakeText,
        );
        assert_eq!(ui.scroll, 0.0);
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
        assert_eq!(
            ui.event(UiEvent::Key(Key::Backspace)).actions,
            [Action::SetQuery("n".into())]
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
