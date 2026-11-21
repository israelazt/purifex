//! What an uninstalled app left that a program has open: it could not be moved to the
//! quarantine. The panel says so, and says what the only way out costs: Windows deletes it at
//! the next restart, and then it cannot be put back.

use purifex_domain::draw::Rect;
use purifex_domain::view::{Action, Model};

use crate::canvas::Canvas;
use crate::theme;
use crate::widgets::{button, button_width, panel};

/// Nothing is drawn when nothing was left open.
pub(crate) fn in_use_panel(c: &mut Canvas, m: &Model, area: Rect, y: f32) -> f32 {
    let Some(in_use) = m.in_use.as_ref() else {
        return 0.0;
    };
    let rect = Rect::new(area.x, y, area.w, 84.0);
    panel(c, rect);
    c.icon(
        "triangle-alert",
        rect.x + 18.0,
        rect.y + 18.0,
        18.0,
        theme::AMBER,
    );
    c.text(
        rect.x + 46.0,
        rect.y + 16.0,
        &format!("Restos de {} en uso", in_use.app),
        theme::sans(14.0, theme::TEXT).bold(),
    );
    let label = format!("Borrar al reiniciar ({})", in_use.items);
    let width = button_width(c, &label);
    c.paragraph(
        rect.x + 46.0,
        rect.y + 40.0,
        rect.w - width - 100.0,
        "Un programa los tiene abiertos y no se pueden mover a la cuarentena. Windows puede borrarlos al reiniciar, pero entonces no se podrán devolver.",
        theme::mono(12.0, theme::TEXT_2),
        Some(2),
    );
    button(
        c,
        Rect::new(rect.right() - 16.0 - width, rect.y + 20.0, width, 44.0),
        &label,
        "trash-2",
        false,
        Some(Action::AskDeleteAtReboot(in_use.app.clone())),
    );
    rect.h + 16.0
}

#[cfg(test)]
mod tests {
    use purifex_domain::draw::Op;
    use purifex_domain::view::InUse;

    use super::*;
    use crate::canvas::testing::FakeText;

    fn render(m: &Model) -> (Vec<String>, Vec<Action>) {
        let mut c = Canvas::new(&FakeText, None);
        let h = in_use_panel(&mut c, m, Rect::new(0.0, 0.0, 1100.0, 0.0), 0.0);
        assert_eq!(h > 0.0, m.in_use.is_some());
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
    fn the_panel_says_what_is_open_and_offers_the_delete_that_cannot_be_undone() {
        let mut m = Model::default();
        assert!(render(&m).1.is_empty(), "nothing open: nothing drawn");
        m.in_use = Some(InUse {
            app: "Kdenlive".into(),
            items: 3,
        });
        let (texts, actions) = render(&m);
        assert!(texts
            .iter()
            .any(|t| t.contains("Restos de Kdenlive en uso")));
        assert!(texts.join(" ").contains("no se podrán devolver"));
        assert!(actions.contains(&Action::AskDeleteAtReboot("Kdenlive".into())));
    }
}
