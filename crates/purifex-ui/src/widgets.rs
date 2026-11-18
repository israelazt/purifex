//! The pieces the pages are made of, as in the design: tags, badges, buttons,
//! the cards with marked corners, the boxes with an icon. Each draws itself on
//! the canvas and returns the room it took.

use purifex_domain::draw::Rect;
use purifex_domain::view::Action;

use crate::canvas::Canvas;
use crate::theme::{self, Tone};

/// A small colored dot.
pub fn dot(c: &mut Canvas, cx: f32, cy: f32, tone: Tone) {
    c.fill(Rect::new(cx - 3.5, cy - 3.5, 7.0, 7.0), 3.5, tone.color());
}

/// A panel: the dark rectangle with a thin border that holds a section.
pub fn panel(c: &mut Canvas, rect: Rect) {
    c.fill(rect, 8.0, theme::PANEL);
    c.stroke(rect, 8.0, 1.0, theme::LINE);
}

/// A 36 x 36 box with an icon, at the top-right of a card or beside a name.
pub fn icon_box(c: &mut Canvas, x: f32, y: f32, size: f32, icon: &'static str, tone: Tone) {
    let rect = Rect::new(x, y, size, size);
    let (fill, border) = match tone {
        Tone::Green | Tone::Gray => (theme::ICON_BOX, theme::LINE),
        other => (
            other.color().with_alpha(0.08),
            other.color().with_alpha(0.3),
        ),
    };
    c.fill(rect, 6.0, fill);
    c.stroke(rect, 6.0, 1.0, border);
    let inner = size * 0.5;
    let color = if tone == Tone::Gray {
        theme::TEXT_2
    } else {
        tone.color()
    };
    c.icon(
        icon,
        x + (size - inner) / 2.0,
        y + (size - inner) / 2.0,
        inner,
        color,
    );
}

/// A tag with a state (`Correcto`, `Sin driver`); returns its width.
pub fn tag(
    c: &mut Canvas,
    x: f32,
    y: f32,
    label: &str,
    tone: Tone,
    icon: Option<&'static str>,
) -> f32 {
    let style = theme::mono(11.0, tone.color()).bold();
    let text_w = c.width(label, &style);
    let icon_w = if icon.is_some() { 18.0 } else { 0.0 };
    let width = 18.0 + icon_w + text_w;
    let rect = Rect::new(x, y, width, 22.0);
    c.fill(rect, 4.0, tone.fill());
    c.stroke(rect, 4.0, 1.0, tone.border());
    let mut cursor = x + 9.0;
    if let Some(name) = icon {
        c.icon(name, cursor, y + 5.0, 12.0, tone.color());
        cursor += icon_w;
    }
    c.text_in(cursor, rect, label, style);
    width
}

/// A small label in capitals (`ACTIVO`, `PRONTO`) with its right edge at `right`.
pub fn badge(c: &mut Canvas, right: f32, bar: Rect, label: &str, tone: Tone) -> f32 {
    let style = theme::mono(10.0, tone.color()).tracked(0.6);
    let text = label.to_uppercase();
    let width = c.width(&text, &style) + 12.0;
    let rect = Rect::new(right - width, bar.y + (bar.h - 18.0) / 2.0, width, 18.0);
    let (fill, border) = if tone == Tone::Gray {
        (theme::BG_SIDE, theme::LINE)
    } else {
        (tone.fill(), tone.border())
    };
    c.fill(rect, 4.0, fill);
    c.stroke(rect, 4.0, 1.0, border);
    c.text_in(rect.x + 6.0, rect, &text, style);
    width
}

/// A button. The main one of a page is filled with the accent; the others are
/// dark with a border. With `action` it can be clicked.
pub fn button(
    c: &mut Canvas,
    rect: Rect,
    label: &str,
    icon: &'static str,
    primary: bool,
    action: Option<Action>,
) {
    let hovered = action.is_some() && c.hovered(rect);
    if primary {
        let glow = rect.inset(-3.0);
        c.stroke(glow, 11.0, 3.0, theme::ACCENT.with_alpha(0.12));
        c.fill(
            rect,
            8.0,
            if hovered {
                theme::ACCENT_2.with_alpha(1.0)
            } else {
                theme::ACCENT
            },
        );
    } else {
        c.fill(rect, 8.0, theme::PANEL);
        c.stroke(
            rect,
            8.0,
            1.0,
            if hovered { theme::TEXT_3 } else { theme::LINE },
        );
    }
    let color = if primary {
        theme::ON_ACCENT
    } else {
        theme::TEXT
    };
    let style = theme::mono(12.0, color).bold().tracked(1.0);
    let text = label.to_uppercase();
    let content_w = 16.0 + 10.0 + c.width(&text, &style);
    let x = rect.x + (rect.w - content_w) / 2.0;
    c.icon(icon, x, rect.y + (rect.h - 16.0) / 2.0, 16.0, color);
    c.text_in(x + 26.0, rect, &text, style);
    if let Some(action) = action {
        c.hit(rect, action);
    }
}

/// The width a button with this label needs.
pub fn button_width(c: &Canvas, label: &str) -> f32 {
    let style = theme::mono(12.0, theme::TEXT).bold().tracked(1.0);
    c.width(&label.to_uppercase(), &style) + 16.0 + 10.0 + 40.0
}

/// A card of the overview: dark, with the marked corners of the design.
pub fn kpi_card(c: &mut Canvas, rect: Rect) {
    c.fill(rect, 4.0, theme::PANEL);
    c.stroke(rect, 4.0, 1.0, theme::LINE);
    let arm = 8.0;
    let (l, t, r, b) = (rect.x, rect.y, rect.right(), rect.bottom());
    for (x1, y1, x2, y2) in [
        (l, t, l + arm, t),
        (l, t, l, t + arm),
        (r, b, r - arm, b),
        (r, b, r, b - arm),
    ] {
        c.line(x1, y1, x2, y2, theme::ACCENT);
    }
}

/// A thin bar filled to `percent` (0 to 100).
pub fn progress(c: &mut Canvas, rect: Rect, percent: f32) {
    c.fill(rect, rect.h / 2.0, theme::LINE_SOFT);
    let w = rect.w * percent.clamp(0.0, 100.0) / 100.0;
    if w > 0.0 {
        c.fill(
            Rect::new(rect.x, rect.y, w, rect.h),
            rect.h / 2.0,
            theme::ACCENT,
        );
    }
}

#[cfg(test)]
mod tests {
    use purifex_domain::draw::Op;

    use super::*;
    use crate::canvas::testing::FakeText;

    #[test]
    fn a_tag_is_as_wide_as_its_text_plus_its_padding_and_its_icon() {
        let mut c = Canvas::new(&FakeText, None);
        let plain = tag(&mut c, 0.0, 0.0, "Correcto", Tone::Green, None);
        let with_icon = tag(
            &mut c,
            0.0,
            0.0,
            "Correcto",
            Tone::Green,
            Some("circle-check"),
        );
        assert_eq!(with_icon - plain, 18.0);
        assert!(plain > 18.0);
    }

    #[test]
    fn a_button_with_an_action_can_be_clicked_and_one_without_cannot() {
        let mut c = Canvas::new(&FakeText, None);
        button(
            &mut c,
            Rect::new(0.0, 0.0, 170.0, 44.0),
            "Reescanear",
            "refresh-cw",
            true,
            Some(Action::Rescan),
        );
        button(
            &mut c,
            Rect::new(0.0, 60.0, 170.0, 44.0),
            "Leyendo…",
            "refresh-cw",
            false,
            None,
        );
        let (_, hits) = c.finish();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].action, Action::Rescan);
    }

    #[test]
    fn a_card_has_its_four_corner_marks() {
        let mut c = Canvas::new(&FakeText, None);
        kpi_card(&mut c, Rect::new(10.0, 10.0, 100.0, 80.0));
        let (list, _) = c.finish();
        assert_eq!(
            list.ops
                .iter()
                .filter(|op| matches!(op, Op::Line { .. }))
                .count(),
            4
        );
    }

    #[test]
    fn a_progress_bar_never_passes_its_ends() {
        let mut c = Canvas::new(&FakeText, None);
        progress(&mut c, Rect::new(0.0, 0.0, 100.0, 4.0), 250.0);
        progress(&mut c, Rect::new(0.0, 0.0, 100.0, 4.0), -5.0);
        let (list, _) = c.finish();
        let widths: Vec<f32> = list
            .ops
            .iter()
            .filter_map(|op| match op {
                Op::Fill { rect, .. } => Some(rect.w),
                _ => None,
            })
            .collect();
        assert_eq!(
            widths,
            [100.0, 100.0, 100.0],
            "track, full, track (no fill at zero)"
        );
    }

    #[test]
    fn a_badge_ends_where_it_was_told() {
        let mut c = Canvas::new(&FakeText, None);
        let w = badge(
            &mut c,
            200.0,
            Rect::new(0.0, 0.0, 10.0, 40.0),
            "activo",
            Tone::Green,
        );
        let (list, _) = c.finish();
        let Op::Fill { rect, .. } = &list.ops[0] else {
            panic!()
        };
        assert_eq!(rect.right(), 200.0);
        assert_eq!(rect.w, w);
    }
}
