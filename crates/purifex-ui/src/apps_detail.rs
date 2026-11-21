//! The details of the app that was clicked in the list: where it lives and the page it
//! registered, with the buttons to open them. What is not known is said so, not left blank.

use purifex_domain::apps::InstalledApp;
use purifex_domain::draw::Rect;
use purifex_domain::view::Action;

use crate::canvas::Canvas;
use crate::theme;
use crate::widgets::{button, button_width};

/// Height of the details under a row.
pub(crate) const DETAIL_H: f32 = 92.0;

pub(crate) fn app_detail(c: &mut Canvas, app: &InstalledApp, bar: Rect) {
    c.fill(bar, 0.0, theme::TEXT.with_alpha(0.03));
    let label = theme::eyebrow(theme::TEXT_3);
    let value = theme::mono(12.0, theme::TEXT_2);
    let (folder, site) = (app.install_location.as_deref(), app.website.as_deref());
    let buttons_w = 360.0;
    for (i, (name, text)) in [("CARPETA", folder), ("WEB", site)].into_iter().enumerate() {
        let y = bar.y + 14.0 + i as f32 * 28.0;
        c.text(bar.x + 70.0, y + 2.0, name, label);
        c.text_cut(
            bar.x + 150.0,
            y,
            text.unwrap_or("no lo indica"),
            value,
            bar.w - 150.0 - buttons_w,
        );
    }
    let mut right = bar.right() - 16.0;
    for (label, icon, action) in [
        site.map(|s| {
            (
                "Sitio web",
                "arrow-right",
                Action::OpenWebsite(s.to_string()),
            )
        }),
        folder.map(|f| {
            (
                "Abrir carpeta",
                "folder-open",
                Action::OpenFolder(f.to_string()),
            )
        }),
    ]
    .into_iter()
    .flatten()
    {
        let width = button_width(c, label);
        right -= width;
        button(
            c,
            Rect::new(right, bar.y + 22.0, width, 44.0),
            label,
            icon,
            false,
            Some(action),
        );
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
        app_detail(&mut c, app, Rect::new(0.0, 0.0, 1100.0, DETAIL_H));
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
        assert_eq!(texts.iter().filter(|t| *t == "no lo indica").count(), 2);
        assert!(actions.is_empty(), "nothing to open");
    }
}
