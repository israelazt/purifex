//! The leads of a game as a list inside a panel (the overview and the detail of a
//! recording show the same one): each is a warning, a title and its evidence, wrapped to
//! the panel. Measured first, so the panel is as tall as what it holds.

use purifex_domain::diagnosis::Finding;
use purifex_domain::draw::{Rect, TextStyle};

use crate::canvas::Canvas;
use crate::theme;

/// A lead with its evidence already wrapped into lines.
pub(crate) struct Lead {
    title: String,
    lines: Vec<String>,
}

fn title_style() -> TextStyle {
    theme::sans(14.0, theme::TEXT).bold()
}

fn text_style() -> TextStyle {
    theme::mono(12.0, theme::TEXT_2)
}

/// The leads of `findings`, their evidence wrapped to `width`.
pub(crate) fn wrap(c: &Canvas, findings: &[Finding], width: f32) -> Vec<Lead> {
    let style = text_style();
    findings
        .iter()
        .map(|f| Lead {
            title: f.title.clone(),
            lines: c.wrap(&f.evidence, &style, width, None),
        })
        .collect()
}

/// The height of the list: `base` (the head of the panel and its padding), a line of
/// text when there are no leads, or room for each lead.
pub(crate) fn height(c: &Canvas, leads: &[Lead], base: f32) -> f32 {
    let line = c.line_height(&text_style()) + 2.0;
    let mut height = base;
    if leads.is_empty() {
        height += 34.0;
    }
    for lead in leads {
        height += 26.0 + lead.lines.len() as f32 * line + 22.0;
    }
    height
}

/// Draws the leads one under another from `y`, inside `panel`.
pub(crate) fn draw(c: &mut Canvas, panel: Rect, mut y: f32, leads: &[Lead]) {
    let style = text_style();
    for lead in leads {
        c.icon(
            "triangle-alert",
            panel.x + 18.0,
            y + 1.0,
            18.0,
            theme::AMBER,
        );
        c.text(panel.x + 48.0, y, &lead.title, title_style());
        y += 26.0;
        for line in &lead.lines {
            c.text(panel.x + 48.0, y, line, style);
            y += c.line_height(&style) + 2.0;
        }
        y += 22.0;
    }
}
