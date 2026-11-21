//! The filter chips of the drivers page: one per category with its count, and a red one
//! for what needs attention. Which rows each filter lets through is decided in the
//! domain (`Model::visible_drivers`).

use purifex_domain::draw::{Rect, TextStyle};
use purifex_domain::view::{Action, DriverFilter, Model};

use crate::canvas::Canvas;
use crate::theme::{self, Tone};
use crate::widgets::dot;

/// What a filter chip says and does.
pub(crate) struct Chip<'a> {
    pub(crate) label: &'a str,
    pub(crate) count: usize,
    pub(crate) active: bool,
    /// The chip of what needs attention: red, with a dot.
    pub(crate) red: bool,
    pub(crate) action: Action,
}

impl Chip<'_> {
    /// The style of the label and of the count.
    fn styles(&self) -> (TextStyle, TextStyle) {
        let color = if self.active {
            theme::ON_ACCENT
        } else if self.red {
            Tone::Red.color().with_alpha(0.85)
        } else {
            theme::TEXT_2
        };
        let label = theme::mono(12.5, color);
        let label = if self.active { label.bold() } else { label };
        let count_color = if self.active {
            theme::ON_ACCENT
        } else {
            theme::TEXT_2
        };
        (label, theme::mono(11.0, count_color))
    }

    /// Room for the red dot of the chip of what needs attention.
    fn lead(&self) -> f32 {
        if self.red {
            16.0
        } else {
            0.0
        }
    }
}

/// A filter chip; returns its width.
pub(crate) fn chip(c: &mut Canvas, x: f32, y: f32, chip: &Chip) -> f32 {
    let (style, count_style) = chip.styles();
    let count_text = chip.count.to_string();
    let count_width = c.width(&count_text, &count_style);
    let width = 28.0 + chip.lead() + c.width(chip.label, &style) + 10.0 + count_width + 12.0;
    let rect = Rect::new(x, y, width, 38.0);
    let hover = c.hover(rect);
    chip_background(c, rect, chip, hover);
    let mut cursor = x + 14.0;
    if chip.red {
        dot(c, cursor + 3.5, y + 19.0, Tone::Red);
        cursor += chip.lead();
    }
    cursor += c.text_in(cursor, rect, chip.label, style) + 10.0;
    let count_box = Rect::new(cursor - 6.0, y + 10.0, count_width + 12.0, 18.0);
    let shade = if chip.active {
        theme::BG.with_alpha(0.18)
    } else {
        theme::TEXT.with_alpha(0.08)
    };
    c.fill(count_box, 4.0, shade);
    c.text_in(count_box.x + 6.0, count_box, &count_text, count_style);
    c.hit(rect, chip.action.clone());
    width
}

/// The fill and outline of a chip: lit when active, red for attention, otherwise a panel
/// whose border follows the hover.
fn chip_background(c: &mut Canvas, rect: Rect, chip: &Chip, hover: f32) {
    if chip.active {
        c.fill(rect, 6.0, theme::ACCENT_2);
    } else if chip.red {
        c.fill(rect, 6.0, Tone::Red.fill());
        c.stroke(rect, 6.0, 1.0, Tone::Red.border());
    } else {
        c.fill(rect, 6.0, theme::PANEL);
        c.stroke(rect, 6.0, 1.0, theme::LINE.mix(theme::TEXT_3, hover));
    }
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
