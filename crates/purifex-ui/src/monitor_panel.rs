//! The install monitor on the apps page: watch an installer to learn what it creates, and the
//! logs kept of earlier ones (open, export, delete, import).

use purifex_domain::app_traces::TraceKind;
use purifex_domain::draw::Rect;
use purifex_domain::monitor::InstallLog;
use purifex_domain::view::{Action, Model};

use crate::canvas::Canvas;
use crate::dialog::{confirm_view, DialogView};
use crate::theme;
use crate::widgets::{button, button_width, panel};

/// The logs written one by one; the rest are counted.
const LOGS_SHOWN: usize = 4;
/// The places of an open log written; the rest are counted.
const PLACES_SHOWN: usize = 10;
const LOG_H: f32 = 44.0;
const PLACE_H: f32 = 18.0;

fn logs_of(m: &Model) -> &[InstallLog] {
    m.install_logs.as_deref().unwrap_or_default()
}

fn log_height(m: &Model, log: &InstallLog) -> f32 {
    if m.log_open.as_deref() == Some(log.id.as_str()) {
        let shown = log.marks.len().min(PLACES_SHOWN) + usize::from(log.marks.len() > PLACES_SHOWN);
        LOG_H + 8.0 + shown.max(1) as f32 * PLACE_H
    } else {
        LOG_H
    }
}

pub(crate) fn monitor_panel(c: &mut Canvas, m: &Model, area: Rect, y: f32) -> f32 {
    let logs = logs_of(m);
    let shown = &logs[..logs.len().min(LOGS_SHOWN)];
    let list: f32 = shown.iter().map(|l| log_height(m, l)).sum();
    let more = if logs.len() > shown.len() { 26.0 } else { 0.0 };
    let rect = Rect::new(area.x, y, area.w, 92.0 + list + more);
    panel(c, rect);
    c.icon(
        "activity",
        rect.x + 18.0,
        rect.y + 18.0,
        18.0,
        theme::ACCENT,
    );
    c.text(
        rect.x + 46.0,
        rect.y + 16.0,
        "Monitor de instalaciones",
        theme::sans(14.0, theme::TEXT).bold(),
    );
    header_buttons(c, m, rect);
    c.paragraph(
        rect.x + 46.0,
        rect.y + 40.0,
        rect.w - 520.0,
        &intro(m),
        theme::mono(12.0, theme::TEXT_2),
        Some(2),
    );
    let mut row_y = rect.y + 84.0;
    for log in shown {
        log_row(
            c,
            m,
            log,
            Rect::new(rect.x, row_y, rect.w, log_height(m, log)),
        );
        row_y += log_height(m, log);
    }
    if more > 0.0 {
        let text = format!("… y {} registros más", logs.len() - shown.len());
        c.text(
            rect.x + 46.0,
            row_y + 4.0,
            &text,
            theme::mono(12.0, theme::TEXT_3),
        );
    }
    rect.h + 16.0
}

fn intro(m: &Model) -> String {
    match &m.monitoring {
        Some(name) => format!("Vigilando {name}. Termina la instalación y pulsa «Terminé la instalación»: Purifex guarda todo lo nuevo que dejó."),
        None => "Elige un instalador: Purifex mira el sistema antes y después, y guarda lo que creó. Al quitar esa app, sus restos se encuentran sin adivinar por el nombre.".into(),
    }
}

fn header_buttons(c: &mut Canvas, m: &Model, rect: Rect) {
    let mut right = rect.right() - 16.0;
    let mut place = |c: &mut Canvas, label: &str, icon, primary, action| {
        let w = button_width(c, label);
        right -= w;
        button(
            c,
            Rect::new(right, rect.y + 20.0, w, 44.0),
            label,
            icon,
            primary,
            Some(action),
        );
        right -= 12.0;
    };
    if m.monitoring.is_some() {
        place(
            c,
            "Terminé la instalación",
            "circle-check",
            true,
            Action::MonitorFinish,
        );
        place(c, "Cancelar", "x", false, Action::MonitorCancel);
    } else {
        place(
            c,
            "Instalar con monitor",
            "package",
            true,
            Action::MonitorStart,
        );
        place(
            c,
            "Importar registro",
            "folder-open",
            false,
            Action::ImportLog,
        );
    }
}

fn log_row(c: &mut Canvas, m: &Model, log: &InstallLog, bar: Rect) {
    c.line(
        bar.x + 16.0,
        bar.y,
        bar.right() - 16.0,
        bar.y,
        theme::LINE_SOFT,
    );
    let name = log.app.as_deref().unwrap_or(&log.installer);
    let from = if log.own { "" } else { "  (importado)" };
    c.text_cut(
        bar.x + 46.0,
        bar.y + 6.0,
        &format!("{name}{from}"),
        theme::sans(13.0, theme::TEXT).bold(),
        bar.w - 560.0,
    );
    let detail = format!(
        "{} · {}",
        log.counts_text(),
        log.on.map_or(String::new(), |d| d.to_string())
    );
    c.text_cut(
        bar.x + 46.0,
        bar.y + 25.0,
        &detail,
        theme::mono(11.5, theme::TEXT_3),
        bar.w - 560.0,
    );
    let open = m.log_open.as_deref() == Some(log.id.as_str());
    log_buttons(c, log, bar, open);
    if open {
        places(c, log, bar);
    }
}

/// «Ver», «Exportar» and «Borrar» of a log, from the right edge in.
fn log_buttons(c: &mut Canvas, log: &InstallLog, bar: Rect, open: bool) {
    let mut right = bar.right() - 16.0;
    for (label, icon, action) in [
        ("Borrar", "trash-2", Action::AskDeleteLog(log.id.clone())),
        ("Exportar", "folder-open", Action::ExportLog(log.id.clone())),
        (
            if open { "Ocultar" } else { "Ver" },
            "list-checks",
            Action::OpenLog(log.id.clone()),
        ),
    ] {
        let w = button_width(c, label).min(130.0);
        right -= w;
        button(
            c,
            Rect::new(right, bar.y + 7.0, w, 30.0),
            label,
            icon,
            false,
            Some(action),
        );
        right -= 8.0;
    }
}

/// What a log holds: its places, as the installer left them.
fn places(c: &mut Canvas, log: &InstallLog, bar: Rect) {
    let style = theme::mono(11.5, theme::TEXT_2);
    for (i, mark) in log.marks.iter().take(PLACES_SHOWN).enumerate() {
        let kind = match mark.kind {
            TraceKind::Folder => "carpeta",
            TraceKind::File => "archivo",
            TraceKind::RegistryKey | TraceKind::RegistryValue => "registro",
            TraceKind::Task => "tarea",
        };
        let at = bar.y + LOG_H + 4.0 + i as f32 * PLACE_H;
        c.text(bar.x + 46.0, at, kind, theme::mono(11.5, theme::TEXT_3));
        c.text_cut(bar.x + 130.0, at, &mark.location, style, bar.w - 180.0);
    }
    let rest = log.marks.len().saturating_sub(PLACES_SHOWN);
    let line = if log.marks.is_empty() {
        Some("No se encontró nada nuevo.".to_string())
    } else {
        (rest > 0).then(|| format!("… y {rest} más"))
    };
    if let Some(text) = line {
        let at = bar.y + LOG_H + 4.0 + log.marks.len().min(PLACES_SHOWN) as f32 * PLACE_H;
        c.text(bar.x + 46.0, at, &text, theme::mono(11.5, theme::TEXT_3));
    }
}

/// The question before a log is deleted, when it is being asked.
pub(crate) fn dialog(m: &Model) -> Option<DialogView> {
    match &m.confirm {
        Some(purifex_domain::view::Confirm::DeleteLog(id)) => Some(delete_question(m, id)),
        _ => None,
    }
}

fn delete_question(m: &Model, id: &str) -> DialogView {
    let name = logs_of(m)
        .iter()
        .find(|l| l.id == id)
        .map_or(id, |l| l.app.as_deref().unwrap_or(&l.installer));
    confirm_view(
        format!("¿Borrar el registro de {name}?"),
        "Se borra el registro de lo que creó su instalador. Si quitas la app después, Purifex ya no tendrá esa pista para encontrar sus restos (seguirá buscando por nombre y por rutas).",
        "Borrar registro",
        "trash-2",
    )
}

#[cfg(test)]
mod tests {
    use purifex_domain::draw::Op;
    use purifex_domain::monitor::Mark;

    use super::*;
    use crate::canvas::testing::FakeText;

    fn log(id: &str, app: &str, places: usize) -> InstallLog {
        InstallLog {
            id: id.into(),
            installer: "setup.exe".into(),
            app: Some(app.into()),
            on: None,
            own: true,
            marks: (0..places)
                .map(|i| Mark {
                    kind: TraceKind::Folder,
                    location: format!(r"C:\ProgramData\Zed{i}"),
                    value: None,
                    stamp: String::new(),
                    bytes: 0,
                })
                .collect(),
        }
    }

    fn render(m: &Model) -> (Vec<String>, Vec<Action>) {
        let mut c = Canvas::new(&FakeText, None);
        monitor_panel(&mut c, m, Rect::new(0.0, 0.0, 1100.0, 0.0), 0.0);
        let (list, hits) = c.finish();
        let texts = list.ops.iter().filter_map(|op| match op {
            Op::Text { text, .. } => Some(text.clone()),
            _ => None,
        });
        (
            texts.collect(),
            hits.into_iter().map(|h| h.action).collect(),
        )
    }

    #[test]
    fn the_buttons_follow_the_state_and_each_log_can_be_opened_exported_and_deleted() {
        let mut m = Model {
            install_logs: Some(vec![log("a", "Zed", 12)]),
            ..Model::default()
        };
        let (_, actions) = render(&m);
        for wanted in [
            Action::MonitorStart,
            Action::ImportLog,
            Action::OpenLog("a".into()),
            Action::ExportLog("a".into()),
            Action::AskDeleteLog("a".into()),
        ] {
            assert!(actions.contains(&wanted), "{wanted:?}");
        }
        assert!(!actions.contains(&Action::MonitorFinish));
        m.log_open = Some("a".into());
        let (texts, _) = render(&m);
        assert!(texts.iter().any(|t| t.contains(r"C:\ProgramData\Zed0")));
        assert!(texts.contains(&"… y 2 más".to_string()), "{texts:?}");
        m.monitoring = Some("zed-setup.exe".into());
        let (texts, actions) = render(&m);
        assert!(
            actions.contains(&Action::MonitorFinish) && actions.contains(&Action::MonitorCancel)
        );
        assert!(!actions.contains(&Action::MonitorStart));
        assert!(texts.iter().any(|t| t.contains("Vigilando zed-setup.exe")));
    }

    #[test]
    fn the_question_names_the_log_and_the_panel_marks_imported_ones() {
        let mut imported = log("b", "Other", 1);
        imported.own = false;
        let m = Model {
            install_logs: Some(vec![imported]),
            ..Model::default()
        };
        assert!(delete_question(&m, "b").title.contains("Other"));
        let (texts, _) = render(&m);
        assert!(texts.iter().any(|t| t.contains("(importado)")));
    }
}
