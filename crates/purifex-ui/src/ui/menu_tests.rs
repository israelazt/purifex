//! The context menu of the list of apps: opened with the right button over a row, it does
//! what an entry says, and anything else only closes it.

use std::time::Duration;

use purifex_domain::apps::InstalledApp;
use purifex_domain::ui_event::{Key, UiEvent};
use purifex_domain::view::{Action, Model, Page};

use super::Ui;
use crate::canvas::testing::FakeText;

fn model() -> Model {
    Model {
        page: Page::Apps,
        apps: Some(vec![InstalledApp {
            id: "HKLM64:Tool".into(),
            name: "Tool".into(),
            uninstall: Some("u.exe".into()),
            install_location: Some(r"C:\Program Files\Tool".into()),
            ..Default::default()
        }]),
        ..Model::default()
    }
}

/// A picture of the model, and the middle of the row of the first app.
fn drawn(ui: &mut Ui, m: &Model) -> (f32, f32) {
    ui.frame(m, 1440.0, 900.0, &FakeText, Duration::ZERO);
    let row = ui
        .hits
        .iter()
        .find(|h| h.action == Action::SelectApp("HKLM64:Tool".into()))
        .expect("the row of the app is a place to click")
        .rect;
    (row.x + 200.0, row.y + 20.0)
}

fn entry(ui: &Ui, action: &Action) -> (f32, f32) {
    let rect = ui
        .hits
        .iter()
        .rev()
        .find(|h| &h.action == action)
        .unwrap()
        .rect;
    (rect.x + 20.0, rect.y + 10.0)
}

#[test]
fn the_right_button_over_a_row_opens_the_menu_and_an_entry_does_its_action() {
    let (m, mut ui) = (model(), Ui::new());
    let (x, y) = drawn(&mut ui, &m);
    assert!(ui.event(UiEvent::SecondaryPress { x, y }).redraw);
    drawn(&mut ui, &m);
    assert!(ui.menu_rect.is_some());
    let folder = Action::OpenFolder(r"C:\Program Files\Tool".into());
    let (ex, ey) = entry(&ui, &folder);
    let out = ui.event(UiEvent::Press { x: ex, y: ey });
    assert_eq!(out.actions, [folder]);
    assert!(ui.menu.is_none(), "choosing closes it");
}

#[test]
fn a_press_elsewhere_the_escape_key_and_the_wheel_close_it_without_doing_anything() {
    let (m, mut ui) = (model(), Ui::new());
    for close in [
        UiEvent::Press { x: 5.0, y: 5.0 },
        UiEvent::Key(Key::Escape),
        UiEvent::Wheel { dy: 40.0 },
    ] {
        let (x, y) = drawn(&mut ui, &m);
        ui.event(UiEvent::SecondaryPress { x, y });
        drawn(&mut ui, &m);
        let out = ui.event(close);
        assert!(out.actions.is_empty() && ui.menu.is_none(), "{close:?}");
    }
}

#[test]
fn an_entry_that_does_not_apply_does_nothing_and_a_right_click_off_the_list_opens_nothing() {
    let (m, mut ui) = (model(), Ui::new());
    let (x, y) = drawn(&mut ui, &m);
    ui.event(UiEvent::SecondaryPress { x, y });
    drawn(&mut ui, &m);
    assert!(
        ui.hits
            .iter()
            .all(|h| !matches!(h.action, Action::OpenWebsite(_))),
        "the app has no web page: that entry is not a place to click"
    );
    ui.event(UiEvent::Key(Key::Escape));
    ui.event(UiEvent::SecondaryPress { x: 5.0, y: 5.0 });
    assert!(ui.menu.is_none());
}

#[test]
fn the_menu_goes_away_when_the_page_changes_or_the_app_leaves_the_list() {
    let (mut m, mut ui) = (model(), Ui::new());
    let (x, y) = drawn(&mut ui, &m);
    ui.event(UiEvent::SecondaryPress { x, y });
    m.page = Page::Fixes;
    ui.frame(&m, 1440.0, 900.0, &FakeText, Duration::ZERO);
    assert!(ui.menu.is_none());
    m.page = Page::Apps;
    let (x, y) = drawn(&mut ui, &m);
    ui.event(UiEvent::SecondaryPress { x, y });
    m.apps = Some(Vec::new());
    ui.frame(&m, 1440.0, 900.0, &FakeText, Duration::ZERO);
    assert!(ui.menu.is_none());
}
