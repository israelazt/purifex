//! The copies of the registration of the apps uninstalled from here. Windows forgets an app
//! when its uninstaller finishes; the copy lets the person register it again (to run the
//! uninstaller once more, or because it was a mistake). Only apps that are gone are offered.

use purifex_domain::draw::Rect;
use purifex_domain::safeguard::BackupEntry;
use purifex_domain::view::{Action, Model};

use crate::canvas::Canvas;
use crate::theme;
use crate::widgets::{button, button_width, panel};

/// How many copies the panel lists (the newest).
const LISTED: usize = 3;
const ROW_H: f32 = 52.0;

/// The copies of apps that are not installed now, newest first.
fn offered(m: &Model) -> Vec<&BackupEntry> {
    let installed = m.apps.as_deref().unwrap_or_default();
    m.backups
        .iter()
        .flatten()
        .filter(|e| !installed.iter().any(|a| a.name == e.app))
        .take(LISTED)
        .collect()
}

/// Nothing is drawn when there is no copy of an app that is gone.
pub(crate) fn backups_panel(c: &mut Canvas, m: &Model, area: Rect, y: f32) -> f32 {
    let entries = offered(m);
    if entries.is_empty() {
        return 0.0;
    }
    let rect = Rect::new(area.x, y, area.w, 44.0 + entries.len() as f32 * ROW_H);
    panel(c, rect);
    c.icon("history", rect.x + 18.0, rect.y + 18.0, 18.0, theme::TEXT_2);
    c.text(
        rect.x + 46.0,
        rect.y + 14.0,
        "Copias del registro de apps desinstaladas",
        theme::sans(14.0, theme::TEXT).bold(),
    );
    for (i, entry) in entries.iter().enumerate() {
        row(c, entry, rect, 44.0 + i as f32 * ROW_H);
    }
    rect.h + 16.0
}

fn row(c: &mut Canvas, entry: &BackupEntry, rect: Rect, top: f32) {
    let day = entry.on.map_or(String::new(), |d| format!(" · {d}"));
    c.text(
        rect.x + 46.0,
        rect.y + top + 16.0,
        &format!("{}{day}", entry.app),
        theme::mono(12.0, theme::TEXT_2),
    );
    let label = "Registrar de nuevo";
    let width = button_width(c, label);
    button(
        c,
        Rect::new(rect.right() - 16.0 - width, rect.y + top, width, 44.0),
        label,
        "undo-2",
        false,
        Some(Action::RestoreBackup(entry.registry_copy.clone())),
    );
}

#[cfg(test)]
mod tests {
    use purifex_domain::apps::InstalledApp;

    use super::*;

    fn entry(app: &str) -> BackupEntry {
        BackupEntry {
            app: app.into(),
            key: "HKCU".into(),
            registry_copy: format!("{app}.reg"),
            on: None,
        }
    }

    #[test]
    fn only_the_copies_of_apps_that_are_gone_are_offered_newest_first() {
        let mut m = Model {
            backups: Some(vec![entry("Discord"), entry("OBS"), entry("Kdenlive")]),
            apps: Some(vec![InstalledApp {
                id: "HKLM64:OBS".into(),
                name: "OBS".into(),
                ..Default::default()
            }]),
            ..Default::default()
        };
        let names: Vec<&str> = offered(&m).iter().map(|e| e.app.as_str()).collect();
        assert_eq!(names, ["Discord", "Kdenlive"], "OBS is installed: no need");
        m.backups = Some((0..5).map(|i| entry(&format!("app{i}"))).collect());
        assert_eq!(offered(&m).len(), LISTED);
        m.backups = None;
        assert!(offered(&m).is_empty());
    }
}
