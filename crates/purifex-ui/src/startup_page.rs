//! The Startup page: everything that starts with Windows, each entry with what it runs, the app
//! it belongs to, whether it is on, and what can be done to it (turn it off or on, take it out to
//! the quarantine, open its folder). Services are only shown.

use purifex_domain::draw::Rect;
use purifex_domain::startup::{StartupItem, StartupKind};
use purifex_domain::view::{Action, Confirm, Model};

use crate::apps_page::info;
use crate::canvas::Canvas;
use crate::dialog::{confirm_view, DialogView};
use crate::page;
use crate::theme::{self, Tone};
use crate::widgets::{button, button_width, icon_box, panel, tag};

const ROW_H: f32 = 64.0;

pub fn draw(c: &mut Canvas, m: &Model, area: Rect) -> f32 {
    let mut y = area.y;
    y += page::head(
        c,
        m,
        area,
        "Arranque",
        "lo que arranca con Windows",
        "Arranque",
        "Programas, servicios y tareas que arrancan solos. Desactivar uno no lo borra: Windows lo recuerda y puedes volver a activarlo. Quitarlo lo manda a la cuarentena, de donde se puede devolver.",
    );
    if let Some(error) = &m.error {
        y += page::error_box(c, Rect::new(area.x, y, area.w, 0.0), error);
    }
    let Some(items) = &m.startup else {
        if m.error.is_none() {
            y += page::reading(
                c,
                Rect::new(area.x, y, area.w, 60.0),
                "Leyendo lo que arranca con Windows…",
            );
        }
        return y - area.y;
    };
    if let Some(text) = &m.message {
        y += info(c, Rect::new(area.x, y, area.w, 0.0), text);
    }
    y += restore_bar(c, m, Rect::new(area.x, y, area.w, 0.0));
    y += table(c, items, Rect::new(area.x, y, area.w, 0.0));
    y - area.y
}

/// When the last thing taken out of the quarantine's open list was an entry of the startup, a way
/// to put it back.
fn restore_bar(c: &mut Canvas, m: &Model, area: Rect) -> f32 {
    let Some(latest) = m.quarantine.as_ref().and_then(|q| q.latest.as_ref()) else {
        return 0.0;
    };
    let Some(name) = latest.app.strip_prefix("Arranque: ") else {
        return 0.0;
    };
    let rect = Rect::new(area.x, area.y, area.w, 60.0);
    panel(c, rect);
    c.icon("history", rect.x + 18.0, rect.y + 20.0, 18.0, theme::AMBER);
    let text = format!("Quitaste {name} del arranque. Está en la cuarentena.");
    c.text_in(rect.x + 46.0, rect, &text, theme::sans(13.0, theme::TEXT));
    let label = "Devolver";
    let w = button_width(c, label);
    let go = Rect::new(rect.right() - 16.0 - w, rect.y + 8.0, w, 44.0);
    button(
        c,
        go,
        label,
        "history",
        false,
        Some(Action::RestoreApp(latest.app.clone())),
    );
    rect.h + 16.0
}

fn table(c: &mut Canvas, items: &[StartupItem], area: Rect) -> f32 {
    let height = 48.0 + items.len().max(1) as f32 * ROW_H;
    let rect = Rect::new(area.x, area.y, area.w, height);
    panel(c, rect);
    let on = items.iter().filter(|i| i.enabled).count();
    let head = Rect::new(rect.x, rect.y, rect.w, 48.0);
    let text = format!("{} entradas · {on} activas", items.len());
    c.text_in(rect.x + 16.0, head, &text, theme::eyebrow(theme::TEXT_3));
    c.line(
        rect.x,
        rect.y + 48.0,
        rect.right(),
        rect.y + 48.0,
        theme::LINE,
    );
    if items.is_empty() {
        c.text(
            rect.x + 16.0,
            rect.y + 70.0,
            "No hay nada que arranque con Windows.",
            theme::mono(12.5, theme::TEXT_3),
        );
    }
    for (i, item) in items.iter().enumerate() {
        c.reveal(if i < 12 { 18 * i as u64 } else { 0 });
        row(
            c,
            item,
            Rect::new(rect.x, rect.y + 48.0 + i as f32 * ROW_H, rect.w, ROW_H),
        );
    }
    c.reveal(0);
    height + 16.0
}

fn row(c: &mut Canvas, item: &StartupItem, bar: Rect) {
    let mid = bar.y + bar.h / 2.0;
    c.line(
        bar.x,
        bar.bottom() - 0.5,
        bar.right(),
        bar.bottom() - 0.5,
        theme::LINE_SOFT,
    );
    let tone = if item.enabled {
        Tone::Green
    } else {
        Tone::Gray
    };
    icon_box(c, bar.x + 16.0, mid - 20.0, 40.0, "power", tone);
    let ink = if item.enabled {
        theme::TEXT
    } else {
        theme::TEXT_3
    };
    let room = bar.w - 810.0;
    c.text_cut(
        bar.x + 70.0,
        bar.y + 11.0,
        &item.name,
        theme::sans(13.5, ink).bold(),
        room,
    );
    c.text_cut(
        bar.x + 70.0,
        bar.y + 34.0,
        &item.command,
        theme::mono(11.0, theme::TEXT_3),
        room,
    );
    let mut x = bar.right() - 725.0;
    x += tag(c, x, mid - 11.0, item.kind.label(), Tone::Gray, None) + 8.0;
    if let Some(app) = &item.app {
        tag(c, x, mid - 11.0, app, Tone::Blue, Some("package"));
    }
    actions(c, item, bar);
}

/// The buttons of a row, from the right edge in.
fn actions(c: &mut Canvas, item: &StartupItem, bar: Rect) {
    let mid = bar.y + bar.h / 2.0;
    let mut right = bar.right() - 16.0;
    let mut put = |c: &mut Canvas, label: &str, icon, action: Action| {
        let w = button_width(c, label).min(150.0);
        right -= w;
        button(
            c,
            Rect::new(right, mid - 15.0, w, 30.0),
            label,
            icon,
            false,
            Some(action),
        );
        right -= 8.0;
    };
    if item.trace().is_some() {
        put(
            c,
            "Quitar",
            "trash-2",
            Action::AskRemoveStartup(item.id.clone()),
        );
    }
    if item.folder().is_some() {
        put(
            c,
            "Carpeta",
            "folder-open",
            Action::OpenStartupFolder(item.id.clone()),
        );
    }
    if item.can_toggle {
        let label = if item.enabled {
            "Desactivar"
        } else {
            "Activar"
        };
        put(c, label, "power", Action::ToggleStartup(item.id.clone()));
    }
    if item.kind == StartupKind::Service {
        c.text(
            right - 130.0,
            mid - 9.0,
            "solo se muestra",
            theme::mono(11.5, theme::TEXT_3),
        );
    }
}

/// The question before an entry is taken out.
pub(crate) fn dialog(m: &Model) -> Option<DialogView> {
    let Some(Confirm::RemoveStartup(id)) = &m.confirm else {
        return None;
    };
    let name = m
        .startup
        .iter()
        .flatten()
        .find(|i| i.id == *id)
        .map_or(id.as_str(), |i| i.name.as_str());
    Some(confirm_view(
        format!("¿Quitar {name} del arranque?"),
        "Se borra la entrada de lo que arranca con Windows, pero se guarda en la cuarentena y se puede devolver. El programa sigue instalado. Si solo quieres que no arranque, es mejor «Desactivar».",
        "Quitar del arranque",
        "trash-2",
    ))
}

#[cfg(test)]
mod tests {
    use purifex_domain::draw::Op;

    use super::*;
    use crate::canvas::testing::FakeText;

    fn item(kind: StartupKind, name: &str, enabled: bool) -> StartupItem {
        StartupItem {
            id: format!("{kind:?}|{name}"),
            name: name.into(),
            command: r#""C:\Apps\Zed\z.exe" -s"#.into(),
            kind,
            place: r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run".into(),
            enabled,
            can_toggle: kind != StartupKind::Service,
            needs_admin: false,
            app: Some("Zed".into()),
        }
    }

    fn render(m: &Model) -> (Vec<String>, Vec<Action>) {
        let mut c = Canvas::new(&FakeText, None);
        draw(&mut c, m, Rect::new(0.0, 0.0, 1100.0, 0.0));
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
    fn each_entry_offers_what_it_allows_and_a_service_only_shows() {
        let reg = item(StartupKind::Registry, "Zed", true);
        let svc = item(StartupKind::Service, "ZedSvc", true);
        let m = Model {
            startup: Some(vec![reg.clone(), svc.clone()]),
            ..Model::default()
        };
        let (texts, actions) = render(&m);
        assert!(
            texts.contains(&"2 entradas · 2 activas".to_string()),
            "{texts:?}"
        );
        assert!(texts.contains(&"solo se muestra".to_string()));
        for wanted in [
            Action::ToggleStartup(reg.id.clone()),
            Action::AskRemoveStartup(reg.id.clone()),
            Action::OpenStartupFolder(reg.id),
            Action::OpenStartupFolder(svc.id.clone()),
        ] {
            assert!(actions.contains(&wanted), "{wanted:?}");
        }
        assert!(!actions.contains(&Action::ToggleStartup(svc.id.clone())));
        assert!(!actions.contains(&Action::AskRemoveStartup(svc.id)));
    }

    #[test]
    fn the_entry_that_was_taken_out_last_can_be_put_back_from_the_page() {
        use purifex_domain::quarantine::{Latest, QuarantineInfo};
        let latest = Latest {
            app: "Arranque: Zed".into(),
            items: 1,
        };
        let mut m = Model {
            startup: Some(vec![]),
            quarantine: Some(QuarantineInfo {
                latest: Some(latest),
                ..Default::default()
            }),
            ..Model::default()
        };
        let (texts, actions) = render(&m);
        assert!(texts
            .iter()
            .any(|t| t.contains("Quitaste Zed del arranque")));
        assert!(actions.contains(&Action::RestoreApp("Arranque: Zed".into())));
        m.quarantine = Some(QuarantineInfo {
            latest: Some(Latest {
                app: "Discord".into(),
                items: 3,
            }),
            ..Default::default()
        });
        assert!(
            !render(&m)
                .1
                .iter()
                .any(|a| matches!(a, Action::RestoreApp(_))),
            "an app's leftovers are not offered here"
        );
    }

    #[test]
    fn an_entry_that_is_off_offers_to_turn_it_on_and_the_page_says_when_it_is_reading() {
        let off = item(StartupKind::Task, "Start", false);
        let m = Model {
            startup: Some(vec![off]),
            ..Model::default()
        };
        let (texts, _) = render(&m);
        assert!(
            texts.contains(&"ACTIVAR".to_string()) || texts.contains(&"Activar".to_string()),
            "{texts:?}"
        );
        let (reading, _) = render(&Model::default());
        assert!(reading.iter().any(|t| t.contains("Leyendo lo que arranca")));
    }

    #[test]
    fn the_question_names_the_entry_and_points_to_turning_it_off() {
        let reg = item(StartupKind::Registry, "Zed", true);
        let m = Model {
            startup: Some(vec![reg.clone()]),
            confirm: Some(Confirm::RemoveStartup(reg.id)),
            ..Model::default()
        };
        let v = dialog(&m).unwrap();
        assert!(v.title.contains("Zed") && v.body.contains("Desactivar"));
        assert!(dialog(&Model::default()).is_none());
    }
}
