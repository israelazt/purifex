//! What the pointer and the keys do to the window: a dialog answers first, then each kind
//! of event has its own small method. They decide which actions to ask for and whether
//! to draw again; the picture itself is built in `ui.rs`.

use purifex_domain::ui_event::{Key, UiEvent};
use purifex_domain::view::{Action, Page};

use super::{Menu, Ui, UiOutput, SCROLL_STEP};
use crate::scrollbar::Pressed;

impl Ui {
    /// What the pointer and the keys ask for: the actions to take and whether to draw again.
    pub fn event(&mut self, event: UiEvent) -> UiOutput {
        let mut out = UiOutput::default();
        // A dialog is open: it alone answers to presses and keys.
        if self.modal && self.modal_event(event, &mut out) {
            return out;
        }
        match event {
            UiEvent::Move { x, y } if self.bar.dragging() => self.drag_thumb(x, y, &mut out),
            UiEvent::Move { x, y } => self.hover(x, y, &mut out),
            UiEvent::Leave => {
                self.mouse = None;
                out.redraw = self.hovered.take().is_some() | self.hover_region.take().is_some();
            }
            UiEvent::Press { x, y } if self.menu.is_some() => self.press_in_menu(x, y, &mut out),
            UiEvent::Press { x, y } => self.press(x, y, &mut out),
            UiEvent::SecondaryPress { x, y } => self.open_menu(x, y, &mut out),
            UiEvent::Release => out.redraw = self.bar.release(),
            UiEvent::Focus => {
                if self.page == Page::Apps {
                    out.actions.push(Action::RefreshApps);
                }
            }
            UiEvent::Wheel { .. } if self.menu.is_some() => self.close_menu(&mut out),
            UiEvent::Wheel { dy } => out.redraw = self.scroll_by(dy),
            UiEvent::Key(key) => self.key(key, &mut out),
            UiEvent::Char(c) if self.search_focus => {
                let query = format!("{}{c}", self.query);
                self.type_query(query, &mut out);
            }
            UiEvent::Char(_) => {}
        }
        out
    }

    /// An event while a dialog is open. Returns whether the dialog took it: a press on one of
    /// its buttons, Escape (which cancels), and every other press, key or wheel, which are
    /// swallowed so nothing under the dialog moves.
    fn modal_event(&mut self, event: UiEvent, out: &mut UiOutput) -> bool {
        match event {
            UiEvent::Press { x, y } => {
                self.mouse = Some((x, y));
                if let Some(index) = self.hit_at(x, y) {
                    out.actions.push(self.hits[index].action.clone());
                    out.redraw = true;
                }
                true
            }
            UiEvent::Key(Key::Escape) => {
                if let Some(action) = self.modal_cancel.clone() {
                    out.actions.push(action);
                    out.redraw = true;
                }
                true
            }
            UiEvent::Wheel { dy } => {
                // The wheel moves the rows of the dialog, when they do not all fit.
                let to = (self.dialog_scroll + dy).clamp(0.0, self.dialog_list.max_scroll());
                out.redraw = std::mem::replace(&mut self.dialog_scroll, to) != to;
                true
            }
            UiEvent::Key(_) | UiEvent::Char(_) => true,
            _ => false,
        }
    }

    /// The right button: over a row of the list it opens the menu of that app; elsewhere it
    /// only closes a menu that was open.
    fn open_menu(&mut self, x: f32, y: f32, out: &mut UiOutput) {
        self.mouse = Some((x, y));
        let row = self.hits.iter().rev().find_map(|hit| match &hit.action {
            Action::SelectApp(id)
                if hit.rect.contains(x, y) && hit.clip.is_none_or(|c| c.contains(x, y)) =>
            {
                Some(id.clone())
            }
            _ => None,
        });
        self.menu = row.map(|app| Menu { app, at: (x, y) });
        out.redraw = true;
    }

    /// A press while the menu is open: on an entry it does it, anywhere else it only closes
    /// the menu (the click does not reach what is under it).
    fn press_in_menu(&mut self, x: f32, y: f32, out: &mut UiOutput) {
        self.mouse = Some((x, y));
        let inside = self.menu_rect.is_some_and(|rect| rect.contains(x, y));
        if let Some(index) = self.hit_at(x, y).filter(|_| inside) {
            out.actions.push(self.hits[index].action.clone());
        }
        self.close_menu(out);
    }

    fn close_menu(&mut self, out: &mut UiOutput) {
        self.menu = None;
        self.menu_rect = None;
        out.redraw = true;
    }

    /// Holding the thumb of the scroll bar: the page follows the pointer at once, with no
    /// glide.
    fn drag_thumb(&mut self, x: f32, y: f32, out: &mut UiOutput) {
        self.mouse = Some((x, y));
        if let Some(to) = self.bar.scroll_for(y, self.max_scroll()) {
            (self.scroll, self.scroll_target) = (to, to);
        }
        out.redraw = true;
    }

    /// The pointer moved: the picture changes when it goes to another button, or to another
    /// place with a hover effect (a row of a table).
    fn hover(&mut self, x: f32, y: f32, out: &mut UiOutput) {
        self.mouse = Some((x, y));
        let now = self.hit_at(x, y);
        let region = self.region_at(x, y);
        out.redraw = now != self.hovered || region != self.hover_region;
        (self.hovered, self.hover_region) = (now, region);
    }

    /// A press: on the scroll bar it takes the thumb or goes a screen; elsewhere it clicks
    /// what is there and gives or takes the keyboard from the search box.
    fn press(&mut self, x: f32, y: f32, out: &mut UiOutput) {
        self.mouse = Some((x, y));
        match self.bar.press(x, y) {
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

    /// The keys that scroll, edit the search or open it (Ctrl+K).
    fn key(&mut self, key: Key, out: &mut UiOutput) {
        match key {
            Key::Down => out.redraw = self.scroll_by(SCROLL_STEP),
            Key::Up => out.redraw = self.scroll_by(-SCROLL_STEP),
            Key::PageDown => out.redraw = self.scroll_by(self.viewport_height * 0.9),
            Key::PageUp => out.redraw = self.scroll_by(-self.viewport_height * 0.9),
            Key::Home => out.redraw = self.scroll_by(-self.content_height),
            Key::End => out.redraw = self.scroll_by(self.content_height),
            Key::Backspace if self.search_focus => {
                let mut query = self.query.clone();
                query.pop();
                self.type_query(query, out);
            }
            Key::Escape if self.menu.is_some() => self.close_menu(out),
            Key::Escape if self.search_focus => {
                if self.query.is_empty() {
                    self.search_focus = false;
                } else {
                    out.actions.push(self.typed(String::new()));
                }
                out.redraw = true;
            }
            Key::Enter if self.search_focus => {
                self.search_focus = false;
                out.redraw = true;
            }
            Key::CtrlK => {
                if self.page != Page::Drivers && self.page != Page::Apps {
                    out.actions.push(Action::Go(Page::Drivers));
                }
                self.search_focus = true;
                out.redraw = true;
            }
            Key::CtrlF => {
                if self.page != Page::Apps {
                    out.actions.push(Action::Go(Page::Apps));
                }
                self.search_focus = true;
                out.redraw = true;
            }
            _ => {}
        }
    }

    /// The search text changed: tell the program and draw again.
    fn type_query(&mut self, query: String, out: &mut UiOutput) {
        out.actions.push(self.typed(query));
        out.redraw = true;
    }
}
