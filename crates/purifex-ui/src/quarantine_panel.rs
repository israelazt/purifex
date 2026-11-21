//! What the quarantine holds, with the buttons to empty it and to put back what was cleaned last.

use purifex_domain::draw::Rect;
use purifex_domain::quarantine::QuarantineInfo;
use purifex_domain::view::{Action, Model};

use crate::canvas::Canvas;
use crate::theme;
use crate::widgets::{button, button_width, panel};

/// A name short enough to go on a button.
pub(crate) fn short(name: &str) -> String {
    const MAX: usize = 18;
    if name.chars().count() <= MAX {
        return name.to_string();
    }
    let head: String = name.chars().take(MAX - 1).collect();
    format!("{}…", head.trim_end())
}

/// The button to empty the quarantine, and beside it the one that puts back the traces of the
/// app cleaned last.
fn quarantine_buttons(c: &mut Canvas, info: &QuarantineInfo, rect: Rect) {
    let label = "Vaciar cuarentena";
    let width = button_width(c, label);
    button(
        c,
        Rect::new(rect.right() - 16.0 - width, rect.y + 14.0, width, 44.0),
        label,
        "trash-2",
        false,
        Some(Action::AskEmptyQuarantine),
    );
    if let Some(latest) = &info.latest {
        let label = format!("Devolver {}", short(&latest.app));
        let put_back = button_width(c, &label);
        button(
            c,
            Rect::new(
                rect.right() - 16.0 - width - 12.0 - put_back,
                rect.y + 14.0,
                put_back,
                44.0,
            ),
            &label,
            "undo-2",
            false,
            Some(Action::RestoreApp(latest.app.clone())),
        );
    }
}

/// What the quarantine holds and the buttons to put things back or empty it. Nothing is drawn when it holds
/// nothing: there is nothing to say.
pub(crate) fn quarantine_panel(c: &mut Canvas, m: &Model, area: Rect, y: f32) -> f32 {
    let Some(info) = m.quarantine.as_ref().filter(|q| q.items > 0) else {
        return 0.0;
    };
    let rect = Rect::new(area.x, y, area.w, 72.0);
    panel(c, rect);
    c.icon(
        "hard-drive",
        rect.x + 18.0,
        rect.y + 18.0,
        18.0,
        theme::TEXT_2,
    );
    c.text(
        rect.x + 46.0,
        rect.y + 14.0,
        "Cuarentena",
        theme::sans(14.0, theme::TEXT).bold(),
    );
    c.paragraph(
        rect.x + 46.0,
        rect.y + 38.0,
        rect.w - if info.latest.is_some() { 600.0 } else { 300.0 },
        &info.to_text(),
        theme::mono(12.0, theme::TEXT_2),
        Some(2),
    );
    quarantine_buttons(c, info, rect);
    rect.h + 16.0
}
