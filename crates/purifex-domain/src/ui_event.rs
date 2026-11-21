//! What the window tells the interface (the mouse, the keys) and what the
//! interface tells the window to do. The window (platform) turns the system's
//! messages into these; the interface (`purifex-ui`) and the application never
//! see a system message.

use std::time::Duration;

use crate::draw::DrawList;
use crate::ports::TextMetrics;

/// A key that is not a character.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    Backspace,
    Escape,
    Enter,
    /// Ctrl+K: go to the search.
    CtrlK,
    /// Ctrl+F: forced uninstall, which is the search of the apps page (where a name, a
    /// folder or a `.exe` of an app that is gone is written).
    CtrlF,
    /// Scrolling keys.
    Up,
    Down,
    PageUp,
    PageDown,
    Home,
    End,
}

/// Positions are in the units of the drawing (1/96 inch), from the top-left of
/// the window's drawing area.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum UiEvent {
    Move {
        x: f32,
        y: f32,
    },
    /// The pointer left the window.
    Leave,
    Press {
        x: f32,
        y: f32,
    },
    /// The secondary (right) button was pressed: a context menu where it points.
    SecondaryPress {
        x: f32,
        y: f32,
    },
    /// The window got the keyboard focus (the user came back to it).
    Focus,
    /// The main button was let go (ends dragging the scroll bar).
    Release,
    /// Positive scrolls the content up (reading further down), in units.
    Wheel {
        dy: f32,
    },
    Char(char),
    Key(Key),
}

/// What to do after an event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Reaction {
    /// Draw the window again.
    pub redraw: bool,
    /// Call [`UiHandler::tick`] soon: there is work to do (the last picture
    /// showed that something is being read, and now it is read).
    pub tick: bool,
    /// Close the window: the program has started again somewhere else (as an
    /// administrator) and this one has nothing left to do.
    pub quit: bool,
}

/// What the window needs from the program to run: a picture when it must draw,
/// and a way to tell it what happened.
pub trait UiHandler {
    /// The picture of the window at that size, in units. `text` measures text and
    /// `now` is the time since the window opened (what animations are measured by).
    fn frame(&mut self, width: f32, height: f32, text: &dyn TextMetrics, now: Duration)
        -> DrawList;
    /// Whether something is still moving: the window asks for another picture soon.
    /// When nothing moves it asks for none, so a quiet window costs no CPU.
    fn animating(&self) -> bool;
    /// The user turned off animations in Windows: nothing moves, everything is at
    /// its final place at once.
    fn set_reduced_motion(&mut self, reduced: bool);
    fn event(&mut self, event: UiEvent) -> Reaction;
    /// Called after a reaction asked for it, outside any event.
    fn tick(&mut self) -> Reaction;
    /// Whether the pointer is over something that can be clicked.
    fn hand_cursor(&self) -> bool;
    /// Whether the sight of hunter mode is being dragged: the pointer is a crosshair.
    fn crosshair_cursor(&self) -> bool {
        false
    }
}
