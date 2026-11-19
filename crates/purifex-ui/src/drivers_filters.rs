//! The filter chips of the drivers page: one per category with its count, and a red one
//! for what needs attention. Which rows each filter lets through is decided in the
//! domain (`Model::visible_drivers`).

use purifex_domain::draw::Rect;
use purifex_domain::view::{Action, DriverFilter, Model};

use crate::canvas::Canvas;
use crate::theme::{self, Tone};
use crate::widgets::dot;

/// A filter chip; returns its width.
/// What a filter chip says and does.
struct Chip<'a> {
    label: &'a str,
    count: usize,
    active: bool,
    /// The chip of what needs attention: red, with a dot.
    red: bool,
    action: Action,
}

fn chip(c: &mut Canvas, x: f32, y: f32, chip: &Chip) -> f32 {
    let Chip {
        label,
        count,
        active,
        red,
        ref action,
    } = *chip;
    let style = theme::mono(
        12.5,
        if active {
            theme::ON_ACCENT
        } else if red {
            Tone::Red.color().with_alpha(0.85)
        } else {
            theme::TEXT_2
        },
    );
    let style = if active { style.bold() } else { style };
    let count_text = count.to_string();
    let count_style = theme::mono(
        11.0,
        if active {
            theme::ON_ACCENT
        } else {
            theme::TEXT_2
        },
    );
    let lead = if red { 16.0 } else { 0.0 };
    let width =
        28.0 + lead + c.width(label, &style) + 10.0 + c.width(&count_text, &count_style) + 12.0;
    let rect = Rect::new(x, y, width, 38.0);
    let hover = c.hover(rect);
    if active {
        c.fill(rect, 6.0, theme::ACCENT_2);
    } else if red {
        c.fill(rect, 6.0, Tone::Red.fill());
        c.stroke(rect, 6.0, 1.0, Tone::Red.border());
    } else {
        c.fill(rect, 6.0, theme::PANEL);
        c.stroke(rect, 6.0, 1.0, theme::LINE.mix(theme::TEXT_3, hover));
    }
    let mut cursor = x + 14.0;
    if red {
        dot(c, cursor + 3.5, y + 19.0, Tone::Red);
        cursor += lead;
    }
    cursor += c.text_in(cursor, rect, label, style) + 10.0;
    let box_rect = Rect::new(
        cursor - 6.0,
        y + 10.0,
        c.width(&count_text, &count_style) + 12.0,
        18.0,
    );
    c.fill(
        box_rect,
        4.0,
        if active {
            theme::BG.with_alpha(0.18)
        } else {
            theme::TEXT.with_alpha(0.08)
        },
    );
    c.text_in(box_rect.x + 6.0, box_rect, &count_text, count_style);
    c.hit(rect, action.clone());
    width
}

pub(crate) fn filters(c: &mut Canvas, m: &Model, area: Rect, y: f32) -> f32 {
    let Some(view) = &m.drivers else { return 0.0 };
    let mut entries: Vec<(String, usize, DriverFilter, bool)> = vec![(
        "Todos los dispositivos".into(),
        view.total,
        DriverFilter::All,
        false,
    )];
    if view.attention > 0 {
        entries.push((
            "Con problema".into(),
            view.attention,
            DriverFilter::Attention,
            true,
        ));
    }
    for category in &view.categories {
        entries.push((
            category.label.into(),
            category.count,
            DriverFilter::Category(category.category),
            false,
        ));
    }
    let (mut x, mut row) = (area.x, 0.0);
    for (label, count, filter, red) in entries {
        // Measure the chip first by drawing it off to the side only when it fits.
        let probe = theme::mono(12.5, theme::TEXT_2);
        let width = 28.0
            + if red { 16.0 } else { 0.0 }
            + c.width(&label, &probe)
            + 10.0
            + c.width(&count.to_string(), &theme::mono(11.0, theme::TEXT_2))
            + 12.0;
        if x + width > area.right() && x > area.x {
            x = area.x;
            row += 48.0;
        }
        let chip_spec = Chip {
            label: &label,
            count,
            active: m.filter == filter,
            red,
            action: Action::SetFilter(filter),
        };
        x += chip(c, x, y + row, &chip_spec) + 10.0;
    }
    row + 38.0 + 16.0
}
