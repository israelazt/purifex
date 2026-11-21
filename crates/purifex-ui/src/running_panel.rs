//! The programs of an app that is gone that still run from its folder. They hold its files
//! open, so the panel lists them and offers to close them (after a confirmation: a program
//! closed this way loses what it had not saved).

use purifex_domain::draw::Rect;
use purifex_domain::view::{Action, Model};

use crate::canvas::Canvas;
use crate::theme;
use crate::widgets::{button, button_width, panel};

/// The most names written; the rest are counted.
const NAMES_SHOWN: usize = 4;

/// Nothing is drawn when no program of the app is running.
pub(crate) fn running_panel(c: &mut Canvas, m: &Model, area: Rect, y: f32) -> f32 {
    let Some(running) = m.running.as_ref() else {
        return 0.0;
    };
    let rect = Rect::new(area.x, y, area.w, 84.0);
    panel(c, rect);
    c.icon("activity", rect.x + 18.0, rect.y + 18.0, 18.0, theme::AMBER);
    c.text(
        rect.x + 46.0,
        rect.y + 16.0,
        &format!("{} sigue abierta", running.app),
        theme::sans(14.0, theme::TEXT).bold(),
    );
    let label = format!("Cerrar programas ({})", running.names.len());
    let width = button_width(c, &label);
    c.paragraph(
        rect.x + 46.0,
        rect.y + 40.0,
        rect.w - width - 100.0,
        &format!(
            "Se está ejecutando desde su carpeta: {}. Ciérralos para soltar sus archivos.",
            names_text(&running.names)
        ),
        theme::mono(12.0, theme::TEXT_2),
        Some(2),
    );
    button(
        c,
        Rect::new(rect.right() - 16.0 - width, rect.y + 20.0, width, 44.0),
        &label,
        "x",
        false,
        Some(Action::AskCloseProcesses(running.app.clone())),
    );
    rect.h + 16.0
}

fn names_text(names: &[String]) -> String {
    let shown = names
        .iter()
        .take(NAMES_SHOWN)
        .cloned()
        .collect::<Vec<_>>()
        .join(", ");
    match names.len().saturating_sub(NAMES_SHOWN) {
        0 => shown,
        more => format!("{shown} y {more} más"),
    }
}

#[cfg(test)]
mod tests {
    use purifex_domain::draw::Op;
    use purifex_domain::view::Running;

    use super::*;
    use crate::canvas::testing::FakeText;

    fn render(m: &Model) -> (Vec<String>, Vec<Action>) {
        let mut c = Canvas::new(&FakeText, None);
        let h = running_panel(&mut c, m, Rect::new(0.0, 0.0, 1100.0, 0.0), 0.0);
        assert_eq!(h > 0.0, m.running.is_some());
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
    fn the_panel_lists_the_programs_and_offers_to_close_them_and_is_silent_without_any() {
        let mut m = Model::default();
        assert!(render(&m).1.is_empty());
        m.running = Some(Running {
            app: "Old Tool".into(),
            names: vec!["a.exe".into(), "b.exe".into()],
        });
        let (texts, actions) = render(&m);
        assert!(texts.iter().any(|t| t.contains("Old Tool sigue abierta")));
        assert!(texts.join(" ").contains("a.exe, b.exe"));
        assert!(actions.contains(&Action::AskCloseProcesses("Old Tool".into())));
    }

    #[test]
    fn many_names_are_counted_not_all_written() {
        let names: Vec<String> = (1..=7).map(|n| format!("p{n}.exe")).collect();
        assert_eq!(names_text(&names), "p1.exe, p2.exe, p3.exe, p4.exe y 3 más");
    }
}
