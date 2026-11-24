//! What a window of a way shows while something is going on: the sign of it, under the heading.

use purifex_domain::draw::Rect;

use crate::canvas::Canvas;
use crate::dialog_view::Busy;
use crate::{loaders, theme};

const WAITING_LINE: &str = "Esperando al desinstalador";
/// The room the waiting line takes under the body.
const WAITING_H: f32 = 32.0;

/// The room `busy` adds to the height of a window.
pub(crate) fn height(busy: &Busy) -> f32 {
    match busy {
        Busy::None => 0.0,
        Busy::Spinner => WAITING_H,
    }
}

/// Draws `busy` in a window whose heading starts at `rect` and whose body ends at `y`.
pub(crate) fn draw(c: &mut Canvas, busy: &Busy, rect: Rect, y: f32) {
    match busy {
        Busy::None => {}
        Busy::Spinner => {
            // Around the icon of the heading.
            loaders::spinner(c, rect.x + 44.0, rect.y + 44.0, 28.0, theme::AMBER);
            let style = theme::mono(12.5, theme::TEXT_3);
            let x = rect.x + 28.0;
            let after = x + c.width(WAITING_LINE, &style);
            c.text(x, y + 14.0, WAITING_LINE, style);
            loaders::breathing_dots(c, after, y + 14.0, style);
        }
    }
}
