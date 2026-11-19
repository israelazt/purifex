//! The panels under the cards: the leads of the last game, the console of events
//! and the principles.

use purifex_domain::draw::Rect;
use purifex_domain::overview::{ConsoleLine, Overview};

use crate::canvas::Canvas;
use crate::lead_list;
use crate::theme::{self, Tone};
use crate::widgets::{dot, panel};

/// The panel of the leads of the last game; returns the height it used.
pub(crate) fn leads(c: &mut Canvas, area: Rect, o: &Overview) -> f32 {
    // Measure first, so the panel is as tall as what it holds.
    let findings = o
        .session
        .as_ref()
        .map_or(&[][..], |s| s.findings.as_slice());
    let wrapped = lead_list::wrap(c, findings, area.w - 36.0 - 30.0);
    let height = lead_list::height(c, &wrapped, 56.0 + 8.0);
    let rect = Rect::new(area.x, area.y, area.w, height);
    panel(c, rect);
    leads_head(c, rect, o);
    let y = rect.y + 70.0;
    if wrapped.is_empty() {
        no_leads(c, rect, y, o.session.is_some());
    }
    lead_list::draw(c, rect, y, &wrapped);
    height + 20.0
}

/// The title of the panel and the id of the game it speaks of.
fn leads_head(c: &mut Canvas, rect: Rect, o: &Overview) {
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
}

/// What the panel says when there is nothing to point at.
fn no_leads(c: &mut Canvas, rect: Rect, y: f32, has_game: bool) {
    let text = if has_game {
        "Nada destacable en la última partida."
    } else {
        "Sin partidas grabadas: aún no hay pistas."
    };
    c.icon("circle-check", rect.x + 18.0, y, 16.0, theme::TEXT_3);
    c.text(rect.x + 42.0, y, text, theme::mono(12.5, theme::TEXT_3));
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
    console_head(c, rect);
    let inner = Rect::new(rect.x + 16.0, rect.y + 72.0, rect.w - 32.0, rect.h - 88.0);
    console_lines(c, inner, lines, date);
}

/// The title of the console and the «lectura local» mark.
fn console_head(c: &mut Canvas, rect: Rect) {
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
}

/// The events inside the dark box: date, tag and text, wrapped, one under another.
fn console_lines(c: &mut Canvas, inner: Rect, lines: &[ConsoleLine], date: &str) {
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
    principle_items(c, rect);
    principles_foot(c, rect);
}

/// Each principle: a check, its title and its explanation.
fn principle_items(c: &mut Canvas, rect: Rect) {
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
}

/// The foot of the panel: what the interface is made of.
fn principles_foot(c: &mut Canvas, rect: Rect) {
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
