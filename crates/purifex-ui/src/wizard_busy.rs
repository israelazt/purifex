//! What a window of a way shows while something is going on: the sign of it, under the heading.

use purifex_domain::draw::Rect;

use crate::canvas::Canvas;
use crate::dialog_view::{Busy, Check};
use crate::{loaders, theme};

const WAITING_LINE: &str = "Esperando al desinstalador";
/// The room the waiting line takes under the body.
const WAITING_H: f32 = 32.0;
/// The height of one part of a checklist.
const CHECK_H: f32 = 36.0;

/// The room `busy` adds to the height of a window.
pub(crate) fn height(busy: &Busy) -> f32 {
    match busy {
        Busy::None => 0.0,
        Busy::Spinner => WAITING_H,
        Busy::Checklist(parts) => parts.len() as f32 * CHECK_H + 16.0,
    }
}

/// Draws `busy` in a window whose heading starts at `rect` and whose body ends at `y`.
pub(crate) fn draw(c: &mut Canvas, busy: &Busy, rect: Rect, y: f32) {
    match busy {
        Busy::None => {}
        Busy::Spinner => waiting(c, rect, y),
        Busy::Checklist(parts) => checklist(c, parts, rect, y),
    }
}

/// A spinner around the icon of the heading and a line that says what is waited for.
fn waiting(c: &mut Canvas, rect: Rect, y: f32) {
    loaders::spinner(c, rect.x + 44.0, rect.y + 44.0, 28.0, theme::AMBER);
    let style = theme::mono(12.5, theme::TEXT_3);
    let x = rect.x + 28.0;
    let after = x + c.width(WAITING_LINE, &style);
    c.text(x, y + 14.0, WAITING_LINE, style);
    loaders::breathing_dots(c, after, y + 14.0, style);
}

/// One row per part: ticked when done, spinning the first that is not, dim the ones to come.
fn checklist(c: &mut Canvas, parts: &[Check], rect: Rect, y: f32) {
    let current = parts.iter().position(|p| !p.done);
    for (i, part) in parts.iter().enumerate() {
        let row = Rect::new(
            rect.x + 28.0,
            y + 16.0 + CHECK_H * i as f32,
            rect.w - 56.0,
            CHECK_H,
        );
        let (middle, going) = (row.y + CHECK_H / 2.0, current == Some(i));
        let color = if part.done || going {
            theme::TEXT
        } else {
            theme::TEXT_3
        };
        c.icon(part.icon, row.x, middle - 9.0, 18.0, color);
        c.text_in(row.x + 30.0, row, part.label, theme::mono(12.5, color));
        let at = row.right() - 12.0;
        if part.done {
            c.icon("circle-check", at - 9.0, middle - 9.0, 18.0, theme::ACCENT);
        } else if going {
            loaders::spinner(c, at, middle, 8.0, theme::AMBER);
        } else {
            c.icon("circle-dot", at - 9.0, middle - 9.0, 18.0, theme::TEXT_3);
        }
    }
}
