//! Uninstalling several apps in a row: the bar that appears when apps are checked in the list,
//! the progress while the uninstallers run one after the other, and the summary at the end
//! (what was done, what failed, and a way to look for the traces of each app).

use purifex_domain::batch::{Batch, BatchEntry, BatchResult};
use purifex_domain::draw::Rect;
use purifex_domain::view::{Action, Model};

use crate::canvas::Canvas;
use crate::dialog::{confirm_view, DialogView};
use crate::theme::{self, Tone};
use crate::widgets::{button, button_width, panel, tag};

/// The apps of the summary written one by one; the rest are counted.
const LINES_SHOWN: usize = 8;
const LINE_H: f32 = 34.0;

/// Nothing is drawn when no app is checked and there is no batch.
pub(crate) fn batch_panel(c: &mut Canvas, m: &Model, area: Rect, y: f32) -> f32 {
    match &m.batch {
        Some(batch) if batch.finished() => summary(c, batch, area, y),
        Some(batch) => progress(c, batch, area, y),
        None if !m.marked.is_empty() => marked_bar(c, m.marked.len(), area, y),
        None => 0.0,
    }
}

/// «Desinstalar N»: the apps checked in the list.
fn marked_bar(c: &mut Canvas, count: usize, area: Rect, y: f32) -> f32 {
    let rect = Rect::new(area.x, y, area.w, 68.0);
    panel(c, rect);
    c.icon(
        "list-checks",
        rect.x + 18.0,
        rect.y + 24.0,
        20.0,
        theme::ACCENT,
    );
    let text = if count == 1 {
        "1 app marcada".to_string()
    } else {
        format!("{count} apps marcadas")
    };
    c.text(
        rect.x + 50.0,
        rect.y + 17.0,
        &text,
        theme::sans(14.0, theme::TEXT).bold(),
    );
    c.text(
        rect.x + 50.0,
        rect.y + 38.0,
        "Se desinstalan una tras otra, cada una con su copia del registro.",
        theme::mono(12.0, theme::TEXT_2),
    );
    let label = format!("Desinstalar {count}");
    let (go_w, clear_w) = (button_width(c, &label), button_width(c, "Quitar marcas"));
    let go = Rect::new(rect.right() - 16.0 - go_w, rect.y + 12.0, go_w, 44.0);
    button(
        c,
        go,
        &label,
        "trash-2",
        true,
        Some(Action::AskUninstallMarked),
    );
    let clear = Rect::new(go.x - 12.0 - clear_w, rect.y + 12.0, clear_w, 44.0);
    button(
        c,
        clear,
        "Quitar marcas",
        "x",
        false,
        Some(Action::ClearMarks),
    );
    rect.h + 16.0
}

/// Which app the batch is on and how many are done; the only button lets go.
fn progress(c: &mut Canvas, batch: &Batch, area: Rect, y: f32) -> f32 {
    let rect = Rect::new(area.x, y, area.w, 84.0);
    panel(c, rect);
    c.icon("trash-2", rect.x + 18.0, rect.y + 18.0, 18.0, theme::AMBER);
    let at = batch.done.len() + 1;
    c.text(
        rect.x + 46.0,
        rect.y + 16.0,
        &format!("Desinstalando {at} de {}", batch.total),
        theme::sans(14.0, theme::TEXT).bold(),
    );
    let now = batch.current.as_deref().unwrap_or("la siguiente");
    let waiting = batch.queue.len();
    let next = if waiting == 0 {
        "es la última".to_string()
    } else {
        format!("después, {waiting} más")
    };
    let stop = "Dejar de esperar";
    let stop_w = button_width(c, stop);
    c.paragraph(
        rect.x + 46.0,
        rect.y + 40.0,
        rect.w - stop_w - 100.0,
        &format!("Termina el desinstalador de {now} en su ventana ({next})."),
        theme::mono(12.0, theme::TEXT_2),
        Some(2),
    );
    button(
        c,
        Rect::new(rect.right() - 16.0 - stop_w, rect.y + 20.0, stop_w, 44.0),
        stop,
        "x",
        false,
        Some(Action::StopWaiting),
    );
    rect.h + 16.0
}

/// What became of each app, with «Buscar restos» for those that were uninstalled.
fn summary(c: &mut Canvas, batch: &Batch, area: Rect, y: f32) -> f32 {
    let shown = batch.done.len().min(LINES_SHOWN);
    let more = batch.done.len() - shown;
    let height = 76.0 + shown as f32 * LINE_H + if more > 0 { 26.0 } else { 0.0 };
    let rect = Rect::new(area.x, y, area.w, height);
    panel(c, rect);
    c.icon(
        "circle-check",
        rect.x + 18.0,
        rect.y + 18.0,
        18.0,
        theme::ACCENT,
    );
    c.text(
        rect.x + 46.0,
        rect.y + 16.0,
        "Desinstalación en lote terminada",
        theme::sans(14.0, theme::TEXT).bold(),
    );
    c.paragraph(
        rect.x + 46.0,
        rect.y + 40.0,
        rect.w - 240.0,
        &batch.summary(),
        theme::mono(12.0, theme::TEXT_2),
        Some(2),
    );
    let close = "Cerrar";
    let close_w = button_width(c, close);
    button(
        c,
        Rect::new(rect.right() - 16.0 - close_w, rect.y + 16.0, close_w, 44.0),
        close,
        "x",
        false,
        Some(Action::CloseBatch),
    );
    for (i, entry) in batch.done.iter().take(shown).enumerate() {
        let line = Rect::new(rect.x, rect.y + 76.0 + i as f32 * LINE_H, rect.w, LINE_H);
        entry_line(c, entry, line);
    }
    if more > 0 {
        c.text(
            rect.x + 46.0,
            rect.bottom() - 26.0,
            &format!("… y {more} más"),
            theme::mono(12.0, theme::TEXT_3),
        );
    }
    rect.h + 16.0
}

fn entry_line(c: &mut Canvas, entry: &BatchEntry, line: Rect) {
    c.line(
        line.x + 16.0,
        line.y,
        line.right() - 16.0,
        line.y,
        theme::LINE_SOFT,
    );
    let (label, tone) = match &entry.result {
        BatchResult::Uninstalled => ("Desinstalada", Tone::Green),
        BatchResult::Failed(_) => ("No se pudo empezar", Tone::Red),
        BatchResult::StillInstalled => ("Sigue instalada", Tone::Amber),
    };
    let mid = line.y + line.h / 2.0;
    tag(c, line.x + 46.0, mid - 11.0, label, tone, None);
    let name_x = line.x + 220.0;
    let free = line.right() - name_x - 200.0;
    c.text_cut(
        name_x,
        mid - 9.0,
        &entry.name,
        theme::sans(13.0, theme::TEXT),
        free,
    );
    match &entry.result {
        BatchResult::Uninstalled => {
            let label = "Buscar restos";
            let w = button_width(c, label).min(170.0);
            button(
                c,
                Rect::new(line.right() - 16.0 - w, mid - 15.0, w, 30.0),
                label,
                "search",
                false,
                Some(Action::FindTraces(entry.name.clone())),
            );
        }
        BatchResult::Failed(why) => {
            let style = theme::mono(11.5, theme::TEXT_3);
            c.text_cut(line.right() - 16.0 - 190.0, mid - 8.0, why, style, 190.0);
        }
        BatchResult::StillInstalled => {}
    }
}

/// The question before the checked apps are uninstalled one after the other.
pub(crate) fn question(m: &Model) -> DialogView {
    let apps = m.apps.iter().flatten();
    let names: Vec<&str> = m
        .marked
        .iter()
        .filter_map(|id| apps.clone().find(|a| a.id == *id))
        .map(|a| a.name.as_str())
        .collect();
    let shown = names.iter().take(6).copied().collect::<Vec<_>>().join(", ");
    let rest = names.len().saturating_sub(6);
    let list = if rest > 0 {
        format!("{shown} y {rest} más")
    } else {
        shown
    };
    confirm_view(
        format!("¿Desinstalar {} apps?", names.len()),
        &format!("{list}. Se abre el desinstalador de cada una, una tras otra: terminas cada uno en su ventana y Purifex pasa a la siguiente. Se pide un solo punto de restauración y se guarda la copia del registro de cada app."),
        "Desinstalar",
        "trash-2",
    )
}

#[cfg(test)]
mod tests {
    use purifex_domain::apps::InstalledApp;
    use purifex_domain::view::Confirm;

    use super::*;

    #[test]
    fn the_question_for_the_batch_names_the_apps_and_says_how_it_goes() {
        let mut m = Model {
            apps: Some(vec![
                InstalledApp {
                    id: "a".into(),
                    name: "Alpha".into(),
                    ..Default::default()
                },
                InstalledApp {
                    id: "b".into(),
                    name: "Beta".into(),
                    ..Default::default()
                },
            ]),
            marked: vec!["a".into(), "b".into()],
            confirm: Some(Confirm::UninstallMarked),
            ..Model::default()
        };
        let v = crate::dialog::view(&m).expect("a question");
        assert_eq!(v.title, "¿Desinstalar 2 apps?");
        assert!(v.body.starts_with("Alpha, Beta."), "{}", v.body);
        assert!(v.body.contains("un solo punto de restauración"));
        m.confirm = None;
        assert!(crate::dialog::view(&m).is_none());
    }
}
