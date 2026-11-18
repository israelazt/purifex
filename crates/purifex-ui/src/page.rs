//! What every page has: the head with its tag line, title, lead and main button,
//! the notice of a failure and the "reading" line.

use purifex_domain::draw::Rect;
use purifex_domain::view::{Action, Model};

use crate::canvas::Canvas;
use crate::theme::{self, Tone};
use crate::widgets::{button, button_width, dot};

/// The head of a page. `button_label` is "Reescanear" (or "Leyendo…" while busy).
/// Returns the height it took, with the space below it.
pub fn head(
    c: &mut Canvas,
    m: &Model,
    area: Rect,
    pill: &str,
    side: &str,
    title: &str,
    lead: &str,
) -> f32 {
    // The tag line: a pill and a muted remark.
    let pill_style = theme::mono(11.0, theme::ACCENT).tracked(0.8);
    let text = pill.to_uppercase();
    let pill_w = 34.0 + c.width(&text, &pill_style);
    let pill_rect = Rect::new(area.x, area.y, pill_w, 24.0);
    c.fill(pill_rect, 4.0, theme::ACCENT.with_alpha(0.06));
    c.stroke(pill_rect, 4.0, 1.0, theme::ACCENT.with_alpha(0.35));
    dot(c, pill_rect.x + 14.0, pill_rect.y + 12.0, Tone::Green);
    c.text_in(pill_rect.x + 26.0, pill_rect, &text, pill_style);
    c.text_in(
        pill_rect.right() + 12.0,
        pill_rect,
        side,
        theme::mono(12.0, theme::TEXT_2),
    );

    // The title and the lead.
    let title_style = theme::sans(34.0, theme::TEXT).bold();
    let title_y = area.y + 38.0;
    c.text(area.x, title_y, title, title_style);
    let lead_y = title_y + c.line_height(&title_style) + 10.0;
    let lead_w = (area.w - 200.0).clamp(300.0, 720.0);
    let lead_h = c.paragraph(
        area.x,
        lead_y,
        lead_w,
        lead,
        theme::mono(13.0, theme::TEXT_2),
        None,
    );

    // The main button, at the right: it does nothing while reading.
    let label = if m.busy { "Leyendo…" } else { "Reescanear" };
    let w = button_width(c, "Reescanear");
    let action = (!m.busy).then_some(Action::Rescan);
    button(
        c,
        Rect::new(area.right() - w, area.y, w, 44.0),
        label,
        "refresh-cw",
        !m.busy,
        action,
    );

    (lead_y + lead_h - area.y) + 24.0
}

/// A failure, in red, with what failed. Returns its height with the space below.
pub fn error_box(c: &mut Canvas, area: Rect, message: &str) -> f32 {
    let style = theme::mono(12.0, Tone::Red.color().with_alpha(0.85));
    let lines = c.wrap(message, &style, area.w - 64.0, Some(4));
    let h = 40.0 + lines.len() as f32 * (c.line_height(&style) + 2.0);
    let rect = Rect::new(area.x, area.y, area.w, h);
    c.fill(rect, 8.0, Tone::Red.fill());
    c.stroke(rect, 8.0, 1.0, Tone::Red.border());
    c.icon(
        "circle-alert",
        rect.x + 18.0,
        rect.y + 18.0,
        18.0,
        Tone::Red.color(),
    );
    c.text(
        rect.x + 48.0,
        rect.y + 14.0,
        "No se pudo leer",
        theme::mono(12.0, Tone::Red.color()).bold(),
    );
    c.paragraph(
        rect.x + 48.0,
        rect.y + 36.0,
        area.w - 64.0,
        message,
        style,
        Some(4),
    );
    h + 16.0
}

/// The line shown while something is being read for the first time.
pub fn reading(c: &mut Canvas, area: Rect, what: &str) -> f32 {
    c.icon("refresh-cw", area.x, area.y + 40.0, 18.0, theme::TEXT_3);
    c.text(
        area.x + 30.0,
        area.y + 40.0,
        what,
        theme::mono(13.0, theme::TEXT_3),
    );
    90.0
}

#[cfg(test)]
mod tests {
    use purifex_domain::draw::Op;

    use super::*;
    use crate::canvas::testing::FakeText;

    fn area() -> Rect {
        Rect::new(264.0 + 32.0, 80.0, 1000.0, 600.0)
    }

    fn texts(c: Canvas) -> (Vec<String>, Vec<crate::canvas::Hit>) {
        let (list, hits) = c.finish();
        (
            list.ops
                .iter()
                .filter_map(|op| {
                    if let Op::Text { text, .. } = op {
                        Some(text.clone())
                    } else {
                        None
                    }
                })
                .collect(),
            hits,
        )
    }

    #[test]
    fn the_head_has_the_tag_the_title_the_lead_and_a_button_that_rescans() {
        let mut c = Canvas::new(&FakeText, None);
        let h = head(
            &mut c,
            &Model::default(),
            area(),
            "Estado general",
            "Escaneo",
            "Estado de tu PC",
            "Lo que hay instalado.",
        );
        let (texts, hits) = texts(c);
        for expected in ["ESTADO GENERAL", "Escaneo", "Estado de tu PC", "REESCANEAR"] {
            assert!(texts.iter().any(|t| t == expected), "{expected}: {texts:?}");
        }
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].action, Action::Rescan);
        assert!(h > 80.0);
    }

    #[test]
    fn while_reading_the_button_says_so_and_cannot_be_pressed_again() {
        let mut c = Canvas::new(&FakeText, None);
        let model = Model {
            busy: true,
            ..Model::default()
        };
        head(&mut c, &model, area(), "x", "y", "t", "l");
        let (texts, hits) = texts(c);
        assert!(texts.iter().any(|t| t == "LEYENDO…"));
        assert!(hits.is_empty());
    }

    #[test]
    fn a_failure_is_shown_with_its_message() {
        let mut c = Canvas::new(&FakeText, None);
        let h = error_box(&mut c, area(), "sin acceso al inventario");
        let (texts, _) = texts(c);
        assert!(texts.iter().any(|t| t == "No se pudo leer"));
        assert!(texts.iter().any(|t| t.contains("sin acceso")));
        assert!(h > 40.0);
    }
}
