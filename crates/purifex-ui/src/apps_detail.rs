//! The details of the app that was clicked in the list: where it lives and the page it
//! registered, with the buttons to open them. What is not known is said so, not left blank.

use purifex_domain::apps::InstalledApp;
use purifex_domain::draw::Rect;
use purifex_domain::view::Action;

use crate::canvas::Canvas;
use crate::theme;
use crate::widgets::{button, button_width};

/// Height of the details under a row.
pub(crate) const DETAIL_H: f32 = 176.0;
/// Room kept at the right of the lines for the buttons.
const BUTTONS_W: f32 = 520.0;

pub(crate) fn app_detail(c: &mut Canvas, app: &InstalledApp, starts: Option<&str>, bar: Rect) {
    c.fill(bar, 0.0, theme::TEXT.with_alpha(0.03));
    let label = theme::eyebrow(theme::TEXT_3);
    let value = theme::mono(12.0, theme::TEXT_2);
    let key = app.registry_key();
    let lines = [
        ("CARPETA", app.install_location.as_deref()),
        ("WEB", app.website.as_deref()),
        ("REGISTRO", key.as_deref()),
        ("DESINSTALADOR", app.uninstall.as_deref()),
        ("INICIO", starts),
    ];
    for (i, (name, text)) in lines.into_iter().enumerate() {
        let y = bar.y + 14.0 + i as f32 * 28.0;
        c.text(bar.x + 70.0, y + 2.0, name, label);
        let room = bar.w - 150.0 - BUTTONS_W;
        c.text_cut(
            bar.x + 150.0,
            y,
            text.unwrap_or(if name == "INICIO" {
                "no arranca con Windows"
            } else {
                "no lo indica"
            }),
            value,
            room,
        );
    }
    buttons(c, app, bar);
}

/// What the app lets be opened, as buttons at the right (the ones that do not apply are not drawn).
fn buttons(c: &mut Canvas, app: &InstalledApp, bar: Rect) {
    let id = || app.id.clone();
    let entries = [
        app.website
            .clone()
            .map(|s| ("Sitio web", "arrow-right", Action::OpenWebsite(s))),
        app.registry_key()
            .map(|_| ("Abrir registro", "key-round", Action::OpenRegistryKey(id()))),
        app.install_location
            .clone()
            .map(|f| ("Abrir carpeta", "folder-open", Action::OpenFolder(f))),
    ];
    let mut right = bar.right() - 16.0;
    for (label, icon, action) in entries.into_iter().flatten() {
        let width = button_width(c, label);
        right -= width;
        let rect = Rect::new(right, bar.y + 22.0, width, 44.0);
        button(c, rect, label, icon, false, Some(action));
        right -= 12.0;
    }
}

#[cfg(test)]
mod tests {
    use purifex_domain::draw::Op;

    use super::*;
    use crate::canvas::testing::FakeText;

    fn render(app: &InstalledApp) -> (Vec<String>, Vec<Action>) {
        let mut c = Canvas::new(&FakeText, None);
        app_detail(&mut c, app, None, Rect::new(0.0, 0.0, 1100.0, DETAIL_H));
        let (list, hits) = c.finish();
        let texts = list.ops.iter().filter_map(|op| match op {
            Op::Text { text, .. } => Some(text.clone()),
            _ => None,
        });
        (
            texts.collect(),
            hits.into_iter().map(|h| h.action).collect(),
        )
    }

    #[test]
    fn it_offers_what_the_app_registered_and_says_what_it_did_not() {
        let full = InstalledApp {
            install_location: Some(r"C:\Program Files\X".into()),
            website: Some("https://x.example".into()),
            ..Default::default()
        };
        let (texts, actions) = render(&full);
        assert!(texts.iter().any(|t| t == r"C:\Program Files\X"));
        assert!(actions.contains(&Action::OpenFolder(r"C:\Program Files\X".into())));
        assert!(actions.contains(&Action::OpenWebsite("https://x.example".into())));
        let (texts, actions) = render(&InstalledApp::default());
        assert_eq!(texts.iter().filter(|t| *t == "no lo indica").count(), 4);
        assert!(texts.iter().any(|t| t == "no arranca con Windows"));
        assert!(actions.is_empty(), "nothing to open");
    }
}
