//! The look of Purifex: it builds the picture of the window (a list of drawing
//! operations) from the model and turns the mouse and the keys into actions. It
//! knows no system (the text is measured through a port) and decides nothing of
//! what is shown: that is the domain's. See `docs/DESIGN.md`.

#![forbid(unsafe_code)]

pub mod canvas;
mod compare_panel;
pub mod drivers_page;
mod overview_cards;
pub mod overview_page;
mod overview_panels;
pub mod page;
pub mod sessions_page;
pub mod shell;
pub mod theme;
pub mod ui;
pub mod widgets;

pub use ui::{Ui, UiOutput};
