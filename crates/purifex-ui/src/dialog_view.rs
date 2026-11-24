//! What a dialog is made of, as data: built from the model, drawn by `dialog`.

use purifex_domain::view::Action;

use crate::theme::Tone;

/// One line of a list with a checkbox.
#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    /// The heading of the group this row opens (drawn above it), if it starts one.
    pub heading: Option<String>,
    pub label: String,
    pub detail: String,
    pub size: String,
    pub user_data: bool,
    pub checked: bool,
    pub toggle: Action,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Btn {
    pub label: String,
    pub icon: &'static str,
    pub primary: bool,
    pub action: Option<Action>,
}

/// The sign of something going on that a window shows.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum Busy {
    #[default]
    None,
    /// Something else is working (the app's uninstaller): a spinner around the icon.
    Spinner,
}

/// Everything a dialog needs to be drawn.
#[derive(Debug, Clone, PartialEq)]
pub struct DialogView {
    pub width: f32,
    pub icon: &'static str,
    pub tone: Tone,
    pub title: String,
    pub body: String,
    pub rows: Vec<Row>,
    /// A link-like button at the left of the buttons (select all / none).
    pub aside: Option<Btn>,
    /// Buttons in the top right corner, beside the title, from the right edge in (a way out
    /// of what the dialog warns about, or a setting of it: they do not fit among the buttons).
    pub corner: Vec<Btn>,
    pub buttons: Vec<Btn>,
    /// What Escape does.
    pub cancel: Action,
    /// The steps drawn at the top, when the window is one of a way with several (removing an app).
    pub steps: Option<crate::wizard_dialog::Steps>,
    /// What is going on, if the window is waiting for something.
    pub busy: Busy,
}

impl Default for DialogView {
    fn default() -> Self {
        Self {
            width: 520.0,
            icon: "triangle-alert",
            tone: Tone::Amber,
            title: String::new(),
            body: String::new(),
            rows: Vec::new(),
            aside: None,
            corner: Vec::new(),
            buttons: Vec::new(),
            cancel: Action::CancelConfirm,
            steps: None,
            busy: Busy::None,
        }
    }
}
