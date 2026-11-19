//! The history of changes on the fixes page: each change with its state and, while it can
//! be undone, a button for it; the question that follows a click on "Deshacer" is drawn
//! at the foot of the panel.

use purifex_domain::draw::Rect;
use purifex_domain::fix::{is_undone, spec, HistoryEntry, HistoryState};
use purifex_domain::view::{Action, Confirm, Model};

use crate::canvas::Canvas;
use crate::fixes_page::{confirm_height, confirmation};
use crate::theme::{self, Tone};
use crate::widgets::{button, button_width, panel, tag};

const UNDO_QUESTION: &str = "El ajuste volverá al valor que tenía antes de este cambio. ¿Deshacer?";

fn history_height(c: &Canvas, m: &Model, rows: usize, width: f32) -> f32 {
    let asked = matches!(m.confirm, Some(Confirm::Undo(_)));
    56.0 + if rows == 0 { 56.0 } else { rows as f32 * 56.0 }
        + if asked {
            confirm_height(c, UNDO_QUESTION, width - 32.0) + 24.0
        } else {
            0.0
        }
        + 8.0
}

pub(crate) fn history_panel(
    c: &mut Canvas,
    m: &Model,
    entries: &[HistoryEntry],
    area: Rect,
    y: f32,
) -> f32 {
    let height = history_height(c, m, entries.len(), area.w);
    let rect = Rect::new(area.x, y, area.w, height);
    panel(c, rect);
    c.icon("history", rect.x + 18.0, rect.y + 20.0, 16.0, theme::TEXT);
    c.text(
        rect.x + 42.0,
        rect.y + 20.0,
        "HISTORIAL DE CAMBIOS",
        theme::mono(12.0, theme::TEXT).tracked(1.0),
    );
    c.line(
        rect.x,
        rect.y + 56.0,
        rect.right(),
        rect.y + 56.0,
        theme::LINE_SOFT,
    );
    if entries.is_empty() {
        c.text(
            rect.x + 18.0,
            rect.y + 56.0 + 20.0,
            "Todavía no se ha cambiado nada.",
            theme::mono(12.5, theme::TEXT_3),
        );
        return height + 16.0;
    }
    for (i, entry) in entries.iter().rev().enumerate() {
        let bar = Rect::new(rect.x, rect.y + 56.0 + i as f32 * 56.0, rect.w, 56.0);
        history_row(c, m, entries, entry, bar);
    }
    if let Some(Confirm::Undo(seq)) = &m.confirm {
        let h = confirm_height(c, UNDO_QUESTION, rect.w - 32.0);
        let text = format!("Cambio #{seq}. {UNDO_QUESTION}");
        let place = Rect::new(rect.x + 16.0, rect.bottom() - h - 16.0, rect.w - 32.0, h);
        confirmation(c, &text, place);
    }
    height + 16.0
}

/// One change of the history: its number, what it changed, its state and, while it can
/// be undone, the button to undo it.
fn history_row(
    c: &mut Canvas,
    m: &Model,
    entries: &[HistoryEntry],
    entry: &HistoryEntry,
    bar: Rect,
) {
    c.line(
        bar.x,
        bar.bottom() - 0.5,
        bar.right(),
        bar.bottom() - 0.5,
        theme::LINE_SOFT,
    );
    c.text_in(
        bar.x + 18.0,
        bar,
        &format!("#{}", entry.seq),
        theme::mono(12.5, theme::TEXT_3),
    );
    let title = spec(&entry.fix).map_or(entry.fix.as_str(), |s| s.title);
    c.text_cut(
        bar.x + 64.0,
        bar.y + 10.0,
        title,
        theme::sans(13.5, theme::TEXT).bold(),
        bar.w - 64.0 - 330.0,
    );
    c.text_cut(
        bar.x + 64.0,
        bar.y + 31.0,
        &format!("{} → {}", entry.before, entry.after),
        theme::mono(11.5, theme::TEXT_2),
        bar.w - 64.0 - 330.0,
    );
    let undone = is_undone(entries, entry.seq);
    let (label, tone) = state_label(entry, undone);
    tag(
        c,
        bar.right() - 18.0 - 140.0 - 110.0,
        bar.y + 17.0,
        &label,
        tone,
        None,
    );
    if entry.state == HistoryState::Applied && !undone && m.confirm.is_none() {
        let w = button_width(c, "Deshacer");
        let place = Rect::new(bar.right() - 18.0 - w, bar.y + 8.0, w, 40.0);
        button(
            c,
            place,
            "Deshacer",
            "undo-2",
            false,
            Some(Action::AskUndo(entry.seq)),
        );
    }
}

/// How the state of a change is said: applied, undone later, or the undoing of another.
fn state_label(entry: &HistoryEntry, undone: bool) -> (String, Tone) {
    match entry.state {
        HistoryState::Applied if undone => ("Deshecho después".to_string(), Tone::Gray),
        HistoryState::Applied => ("Aplicado".to_string(), Tone::Green),
        HistoryState::Undone => (
            format!("Deshace #{}", entry.undoes.unwrap_or(0)),
            Tone::Gray,
        ),
    }
}
