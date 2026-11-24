//! The look of Purifex: it builds the picture of the window (a list of drawing
//! operations) from the model and turns the mouse and the keys into actions. It
//! knows no system (the text is measured through a port) and decides nothing of
//! what is shown: that is the domain's. See `docs/DESIGN.md`.

#![forbid(unsafe_code)]

pub mod anim;
mod apps_detail;
mod apps_filters;
mod apps_page;
mod apps_row;
mod backups_panel;
mod batch_panel;
pub mod canvas;
mod compare_panel;
mod context_menu;
mod dialog;
mod dialog_view;
mod drivers_filters;
pub mod drivers_page;
mod fix_history;
pub mod fixes_page;
mod hunter_dialog;
mod in_use_panel;
mod junk_page;
mod lead_list;
pub mod loaders;
mod monitor_panel;
mod overview_cards;
pub mod overview_page;
mod overview_panels;
pub mod page;
mod quarantine_panel;
mod running_panel;
mod scrollbar;
pub mod sessions_page;
pub mod shell;
mod startup_page;
pub mod theme;
mod traces_dialog;
pub mod ui;
mod updates_panel;
pub mod widgets;
mod wizard_busy;
mod wizard_dialog;

pub use ui::{Ui, UiOutput};
