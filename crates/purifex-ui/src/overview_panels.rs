//! The panels under the cards: the leads of the last game, the console of events
//! and the principles.

use purifex_domain::draw::Rect;
use purifex_domain::overview::{ConsoleLine, Overview};

use crate::canvas::Canvas;
use crate::theme::{self, Tone};
use crate::widgets::{dot, panel};

#[expect(clippy::too_many_lines, reason = "legacy: split")]
pub(crate) fn leads(c: &mut Canvas, area: Rect, o: &Overview) -> f32 {
    let findings = o.session.as_ref().map(|s| s.findings.as_slice());
    let style_title = theme::sans(14.0, theme::TEXT).bold();
    let style_text = theme::mono(12.0, theme::TEXT_2);
    // Measure first, so the panel is as tall as what it holds.
    let wrap_w = area.w - 36.0 - 30.0;
    let mut height = 56.0 + 8.0;
    let rows: Vec<(String, Vec<String>)> = findings
        .unwrap_or_default()
        .iter()
        .map(|f| {
            (
                f.title.clone(),
                c.wrap(&f.evidence, &style_text, wrap_w, None),
            )
        })
        .collect();
    if rows.is_empty() {
        height += 34.0;
    }
    for (_, lines) in &rows {
        height += 20.0 + 6.0 + lines.len() as f32 * (c.line_height(&style_text) + 2.0) + 22.0;
    }
    let rect = Rect::new(area.x, area.y, area.w, height);
    panel(c, rect);
    c.icon(
        "list-checks",
        rect.x + 18.0,
        rect.y + 20.0,
        16.0,
        theme::TEXT,
    );
    c.text(
        rect.x + 42.0,
        rect.y + 20.0,
        "PISTAS DE LA ÚLTIMA PARTIDA",
        theme::mono(12.0, theme::TEXT).tracked(1.0),
    );
    if let Some(s) = &o.session {
        let id = s.id.to_uppercase();
        let id_style = theme::mono(12.0, theme::TEXT_3).tracked(0.6);
        c.text(
            rect.right() - 18.0 - c.width(&id, &id_style),
            rect.y + 20.0,
            &id,
            id_style,
        );
    }
    c.line(
        rect.x,
        rect.y + 56.0,
        rect.right(),
        rect.y + 56.0,
        theme::LINE_SOFT,
    );
    let mut y = rect.y + 70.0;
    if rows.is_empty() {
        let text = if o.session.is_some() {
            "Nada destacable en la última partida."
        } else {
            "Sin partidas grabadas: aún no hay pistas."
        };
        c.icon("circle-check", rect.x + 18.0, y, 16.0, theme::TEXT_3);
        c.text(rect.x + 42.0, y, text, theme::mono(12.5, theme::TEXT_3));
    }
    for (title, lines) in rows {
        c.icon("triangle-alert", rect.x + 18.0, y + 1.0, 18.0, theme::AMBER);
        c.text(rect.x + 48.0, y, &title, style_title);
        y += 26.0;
        for line in lines {
            c.text(rect.x + 48.0, y, &line, style_text);
            y += c.line_height(&style_text) + 2.0;
        }
        y += 22.0;
    }
    height + 20.0
}

pub(crate) fn tag_color(tag: &str) -> Tone {
    match tag {
        "PISTA" => Tone::Amber,
        "DRIVERS" => Tone::Blue,
        _ => Tone::Green,
    }
}

pub(crate) fn console(c: &mut Canvas, rect: Rect, lines: &[ConsoleLine], date: &str) {
    panel(c, rect);
    c.icon(
        "square-terminal",
        rect.x + 18.0,
        rect.y + 20.0,
        16.0,
        theme::TEXT,
    );
    c.text(
        rect.x + 42.0,
        rect.y + 20.0,
        "PURIFEX // CONSOLA DE EVENTOS",
        theme::mono(12.0, theme::TEXT).tracked(1.0),
    );
    let label = "LECTURA LOCAL";
    let style = theme::mono(11.0, theme::TEXT_3).tracked(0.6);
    let w = c.width(label, &style);
    c.text(rect.right() - 18.0 - w, rect.y + 20.0, label, style);
    dot(
        c,
        rect.right() - 18.0 - w - 12.0,
        rect.y + 28.0,
        Tone::Green,
    );
    c.line(
        rect.x,
        rect.y + 56.0,
        rect.right(),
        rect.y + 56.0,
        theme::LINE_SOFT,
    );
    let inner = Rect::new(rect.x + 16.0, rect.y + 72.0, rect.w - 32.0, rect.h - 88.0);
    c.fill(inner, 6.0, theme::BG);
    c.stroke(inner, 6.0, 1.0, theme::LINE_SOFT);
    c.clip(inner);
    let text_style = theme::mono(12.0, theme::TEXT);
    let step = c.line_height(&text_style) + 4.0;
    let text_x = inner.x + 14.0 + 92.0 + 100.0;
    let mut y = inner.y + 14.0;
    for line in lines {
        c.text(inner.x + 14.0, y, date, theme::mono(12.0, theme::TEXT_3));
        c.text(
            inner.x + 14.0 + 92.0,
            y,
            &format!("[{}]", line.tag),
            theme::mono(12.0, tag_color(&line.tag).color()).bold(),
        );
        let wrapped = c.wrap(&line.text, &text_style, inner.right() - 14.0 - text_x, None);
        for part in &wrapped {
            c.text(text_x, y, part, text_style);
            y += step;
        }
        y += 4.0;
    }
    c.unclip();
}

/// How tall the console must be for its lines at this width.
pub(crate) fn console_height(c: &Canvas, width: f32, lines: &[ConsoleLine]) -> f32 {
    let style = theme::mono(12.0, theme::TEXT);
    let step = c.line_height(&style) + 4.0;
    let text_w = width - 32.0 - 28.0 - 92.0 - 100.0;
    let rows: usize = lines
        .iter()
        .map(|l| c.wrap(&l.text, &style, text_w, None).len().max(1))
        .sum();
    88.0 + 28.0 + rows as f32 * step + lines.len() as f32 * 4.0
}

const PRINCIPLES: [(&str, &str); 3] = [
    ("Solo lectura", "Esta versión no cambia nada de tu sistema."),
    ("Sin telemetría", "Nada sale de tu PC."),
    (
        "Datos locales",
        r"Las grabaciones están en %LOCALAPPDATA%\Purifex.",
    ),
];

/// How tall the principles panel must be to hold its items at this width.
pub(crate) fn principles_height(c: &Canvas, width: f32) -> f32 {
    let small = theme::mono(11.0, theme::TEXT_3);
    let items: f32 = PRINCIPLES
        .iter()
        .map(|(_, text)| {
            24.0 + c.wrap(text, &small, width - 64.0, None).len() as f32
                * (c.line_height(&small) + 2.0)
                + 12.0
        })
        .sum();
    74.0 + items + 40.0
}

pub(crate) fn principles(c: &mut Canvas, rect: Rect) {
    panel(c, rect);
    c.text(
        rect.x + 18.0,
        rect.y + 20.0,
        "PRINCIPIOS",
        theme::mono(12.0, theme::TEXT).tracked(1.0),
    );
    c.line(
        rect.x,
        rect.y + 56.0,
        rect.right(),
        rect.y + 56.0,
        theme::LINE_SOFT,
    );
    let mut y = rect.y + 74.0;
    for (title, small) in PRINCIPLES {
        c.fill(
            Rect::new(rect.x + 18.0, y + 2.0, 16.0, 16.0),
            3.0,
            theme::CHECK_BLUE,
        );
        c.icon("circle-check", rect.x + 20.0, y + 4.0, 12.0, theme::TEXT);
        c.text(
            rect.x + 46.0,
            y,
            title,
            theme::mono(12.5, theme::TEXT).bold(),
        );
        let h = c.paragraph(
            rect.x + 46.0,
            y + 20.0,
            rect.w - 64.0,
            small,
            theme::mono(11.0, theme::TEXT_3),
            None,
        );
        y += 24.0 + h + 12.0;
    }
    let foot = rect.bottom() - 40.0;
    c.line(rect.x, foot, rect.right(), foot, theme::LINE_SOFT);
    c.text(
        rect.x + 18.0,
        foot + 13.0,
        "Interfaz:",
        theme::mono(11.0, theme::TEXT_3),
    );
    let value = "nativa · Direct2D";
    let style = theme::mono(11.0, theme::ACCENT).bold();
    c.text(
        rect.right() - 18.0 - c.width(value, &style),
        foot + 13.0,
        value,
        style,
    );
}
