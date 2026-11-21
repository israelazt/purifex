//! The menu that opens with the right button over an app of the list. It is drawn over
//! everything, at the pointer (kept inside the window); an entry that does not apply to the
//! app (no folder, no web page, protected) is shown dim and does nothing.

use purifex_domain::apps::InstalledApp;
use purifex_domain::draw::Rect;
use purifex_domain::view::Action;

use crate::canvas::Canvas;
use crate::theme;

pub(crate) const MENU_W: f32 = 280.0;
const ITEM_H: f32 = 38.0;
const PAD: f32 = 6.0;

/// One line of the menu: what it says, its icon, and what it does (`None` when it does not
/// apply to this app).
pub(crate) struct Item {
    pub label: &'static str,
    pub icon: &'static str,
    pub action: Option<Action>,
}

/// The entries for an app, in the order they are shown.
pub(crate) fn items(app: &InstalledApp) -> Vec<Item> {
    let free = app.protection().is_none();
    let id = || app.id.clone();
    let folder = app.install_location.clone();
    let site = app.website.clone();
    vec![
        Item {
            label: "Desinstalar",
            icon: "trash-2",
            action: free.then(|| Action::AskUninstall(id())),
        },
        Item {
            label: "Desinstalación forzada",
            icon: "zap",
            action: free.then(|| Action::AskForceUninstall(id())),
        },
        Item {
            label: "Abrir carpeta de instalación",
            icon: "folder-open",
            action: folder.map(Action::OpenFolder),
        },
        Item {
            label: "Abrir clave de registro",
            icon: "key-round",
            action: app.registry_key().map(|_| Action::OpenRegistryKey(id())),
        },
        Item {
            label: "Sitio web",
            icon: "arrow-right",
            action: site.map(Action::OpenWebsite),
        },
        Item {
            label: "Propiedades",
            icon: "info",
            action: Some(Action::SelectApp(id())),
        },
    ]
}

/// The height of a menu with `count` entries.
fn height(count: usize) -> f32 {
    count as f32 * ITEM_H + 2.0 * PAD
}

/// Draws the menu with its top-left corner at `at`, moved so that it fits in `window`;
/// returns where it ended up.
pub(crate) fn draw(c: &mut Canvas, app: &InstalledApp, at: (f32, f32), window: (f32, f32)) -> Rect {
    let list = items(app);
    let (w, h) = (MENU_W, height(list.len()));
    let x = at.0.min(window.0 - w - 8.0).max(8.0);
    let y = at.1.min(window.1 - h - 8.0).max(8.0);
    let rect = Rect::new(x, y, w, h);
    c.fill(rect.inset(-2.0), 12.0, theme::BG.with_alpha(0.6));
    c.fill(rect, 8.0, theme::PANEL);
    c.stroke(rect, 8.0, 1.0, theme::LINE);
    for (i, item) in list.into_iter().enumerate() {
        let row = Rect::new(x + PAD, y + PAD + i as f32 * ITEM_H, w - 2.0 * PAD, ITEM_H);
        let hover = if item.action.is_some() {
            c.hover(row)
        } else {
            0.0
        };
        if hover > 0.01 {
            c.fill(row, 6.0, theme::TEXT.with_alpha(0.07 * hover));
        }
        let color = if item.action.is_some() {
            theme::TEXT
        } else {
            theme::TEXT_3.with_alpha(0.6)
        };
        c.icon(item.icon, row.x + 10.0, row.y + 10.0, 18.0, color);
        c.text_in(row.x + 40.0, row, item.label, theme::sans(13.0, color));
        if let Some(action) = item.action {
            c.hit(row, action);
        }
    }
    rect
}

#[cfg(test)]
mod tests {
    use super::*;

    fn labels(app: &InstalledApp) -> Vec<(&'static str, bool)> {
        items(app)
            .into_iter()
            .map(|i| (i.label, i.action.is_some()))
            .collect()
    }

    #[test]
    fn an_entry_that_does_not_apply_is_there_but_does_nothing() {
        let plain = InstalledApp {
            id: "HKLM64:x".into(),
            name: "X".into(),
            ..Default::default()
        };
        let l = labels(&plain);
        assert!(l.contains(&("Desinstalar", true)) && l.contains(&("Propiedades", true)));
        assert!(
            l.contains(&("Abrir carpeta de instalación", false))
                && l.contains(&("Sitio web", false))
        );
        let protected = InstalledApp {
            name: "Microsoft Edge".into(),
            ..Default::default()
        };
        assert!(labels(&protected).contains(&("Desinstalar", false)));
        let full = InstalledApp {
            id: "HKLM64:Tool".into(),
            install_location: Some(r"C:\X".into()),
            website: Some("https://x.example".into()),
            ..plain
        };
        assert!(labels(&full).iter().all(|(_, on)| *on));
    }
}
