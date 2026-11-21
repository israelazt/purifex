//! The window of traces left by an uninstalled app: what it says, and the rows it lists,
//! grouped (files and folders, registry) with what proves each one. The list that is shown
//! is the domain's (`CleanReport::listed`), the same one the marks refer to.

use purifex_domain::app_traces::{Trace, TraceGroup};
use purifex_domain::apps::size_text;
use purifex_domain::cleanup_log::CleanReport;
use purifex_domain::scan_mode::ScanMode;
use purifex_domain::trace_text::Counts;
use purifex_domain::view::{Action, Model};

use crate::dialog::{Btn, DialogView, Row};
use crate::theme::Tone;

pub(crate) fn traces_view(m: &Model, report: &CleanReport) -> DialogView {
    let rows = trace_rows(m, report);
    let checked = rows.iter().filter(|r| r.checked).count();
    let name = report.app.clone();
    let all_checked = checked == rows.len();
    DialogView {
        width: 900.0,
        icon: "trash-2",
        tone: Tone::Amber,
        title: format!("Restos de {name}"),
        body: traces_body(m, report),
        more: report.proposed.len().saturating_sub(rows.len()),
        rows,
        aside: Some(Btn {
            label: if all_checked {
                "Quitar marcas".into()
            } else {
                "Marcar todo".into()
            },
            icon: "list-checks",
            primary: false,
            action: Some(Action::SelectTraces(!all_checked)),
        }),
        corner: corner_buttons(m, report),
        buttons: buttons(m, report, checked),
        cancel: Action::CloseTraces,
    }
}

/// The buttons: while the app is still installed the way forward is to uninstall it (the
/// traces are looked for when it is gone); afterwards, to remove the traces.
fn buttons(m: &Model, report: &CleanReport, checked: usize) -> Vec<Btn> {
    let name = report.app.clone();
    let close = Btn {
        label: "Cerrar".into(),
        icon: "x",
        primary: false,
        action: Some(Action::CloseTraces),
    };
    if report.still_installed {
        let id = m
            .apps
            .iter()
            .flatten()
            .find(|a| a.name == name)
            .map(|a| a.id.clone());
        let uninstall = Btn {
            label: "Desinstalar y limpiar".into(),
            icon: "trash-2",
            primary: true,
            action: id.map(Action::AskUninstall),
        };
        return vec![close, uninstall];
    }
    vec![
        close,
        Btn {
            label: format!("Eliminar marcados ({checked})"),
            icon: "trash-2",
            primary: false,
            action: (checked > 0).then(|| Action::AskClean(name.clone())),
        },
        Btn {
            label: "Eliminar todo rastro".into(),
            icon: "trash-2",
            primary: true,
            action: Some(Action::AskCleanAll(name)),
        },
    ]
}

/// What sits beside the title: the mode of the search, and the way out for the traces that
/// only an administrator can remove.
fn corner_buttons(m: &Model, report: &CleanReport) -> Vec<Btn> {
    let mode = Btn {
        label: format!("Modo: {}", m.scan_mode.label()),
        icon: "sliders-horizontal",
        primary: false,
        action: Some(Action::SetScanMode(m.scan_mode.other())),
    };
    std::iter::once(mode)
        .chain(restart_button(m, report))
        .collect()
}

/// The way out for the traces that only an administrator can remove.
fn restart_button(m: &Model, report: &CleanReport) -> Option<Btn> {
    (report.needing_admin() > 0 && m.elevated == Some(false)).then(|| Btn {
        label: "Reiniciar como administrador".into(),
        icon: "shield-check",
        primary: false,
        action: Some(Action::AskRestartAsAdmin(report.app.clone())),
    })
}

/// What the window says above the list; when the search was cut short, that too.
fn traces_body(m: &Model, report: &CleanReport) -> String {
    if report.still_installed {
        return format!(
            "{} sigue instalada, así que esto aún no son restos: lo usa la propia app. Pulsa «Desinstalar y limpiar»: se abre su desinstalador y, cuando termine, Purifex busca lo que deje y te lo muestra para quitarlo.",
            report.app
        );
    }
    let mut body = "Esto es lo que quedó después de desinstalar. Marca lo que quieras quitar, o elimina todo de una vez sin elegir.".to_string();
    if m.scan_mode == ScanMode::Safe {
        body.push_str(" Modo Seguro: solo se muestra lo que apunta a la carpeta de la app; lo que se reconoce por el nombre queda fuera.");
    }
    if report.incomplete {
        body.push_str(" La búsqueda se cortó por tiempo y puede faltar algo: pulsa «Buscar otra vez» para completarla.");
    }
    let admin = report.needing_admin();
    if admin > 0 && m.elevated == Some(false) {
        body.push_str(&format!(
            " {admin} de estos restos solo los quita un administrador: sin él se quitan los demás y estos se quedan."
        ));
    }
    body
}

/// The rows the window lists, grouped: the first row of each group carries its heading.
fn trace_rows(m: &Model, report: &CleanReport) -> Vec<Row> {
    let mut previous = None;
    report
        .listed()
        .into_iter()
        .map(|t| {
            let group = t.group();
            let heading = (previous != Some(group)).then(|| group_heading(report, group));
            previous = Some(group);
            Row {
                heading,
                label: t.location.clone(),
                detail: row_detail(t, m.elevated == Some(false)),
                size: if t.bytes > 0 {
                    size_text(Some(t.bytes / 1024))
                } else {
                    String::new()
                },
                user_data: t.user_data,
                checked: !m.unchecked.contains(&t.key()),
                toggle: Action::ToggleTrace(t.key()),
            }
        })
        .collect()
}

/// «Archivos y carpetas (7) · 196 MB», «Registro (10)» or «Tareas programadas (1)»: how many
/// there are of the group in the whole report, not only in the list.
fn group_heading(report: &CleanReport, group: TraceGroup) -> String {
    let of_group = report.proposed.iter().filter(|t| t.group() == group);
    let counts = Counts::of(of_group);
    match group {
        TraceGroup::Registry => format!("Registro ({})", counts.registry),
        TraceGroup::Tasks => format!("Tareas programadas ({})", counts.tasks),
        TraceGroup::Files => format!(
            "Archivos y carpetas ({}) · {}",
            counts.files,
            size_text(Some(counts.bytes.div_ceil(1024)))
        ),
    }
}

/// What proves the trace is the app's, which value it is if it is a registry value, and
/// that it needs an administrator when the program is not one.
fn row_detail(t: &Trace, without_admin: bool) -> String {
    let proof = t.evidence.label();
    let mut detail = match &t.value {
        Some(value) => format!("{proof} · valor: {value}"),
        None => proof.to_string(),
    };
    if without_admin && t.needs_admin() {
        detail.push_str(" · requiere administrador");
    }
    detail
}
