//! The look of Purifex: it builds the picture of the window (a list of drawing
//! operations) from the model and turns the mouse and the keys into actions. It
//! knows no system (the text is measured through a port) and decides nothing of
//! what is shown: that is the domain's. See `docs/DESIGN.md`.

#![forbid(unsafe_code)]

pub mod anim;
mod apps_page;
mod backups_panel;
pub mod canvas;
mod compare_panel;
mod dialog;
mod drivers_filters;
pub mod drivers_page;
mod fix_history;
pub mod fixes_page;
mod in_use_panel;
mod lead_list;
mod overview_cards;
pub mod overview_page;
mod overview_panels;
pub mod page;
mod scrollbar;
pub mod sessions_page;
pub mod shell;
pub mod theme;
mod traces_dialog;
pub mod ui;
mod updates_panel;
pub mod widgets;

pub use ui::{Ui, UiOutput};
