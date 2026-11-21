//! The window of traces left by an uninstalled app: what it says, and the rows it lists,
//! grouped (files and folders, registry) with what proves each one. The list that is shown
//! is the domain's (`CleanReport::listed`), the same one the marks refer to.

use purifex_domain::app_traces::Trace;
use purifex_domain::apps::size_text;
use purifex_domain::cleanup_log::CleanReport;
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
        corner: restart_button(m, report),
        buttons: vec![
            Btn {
                label: "Cerrar".into(),
                icon: "x",
                primary: false,
                action: Some(Action::CloseTraces),
            },
            Btn {
                label: format!("Eliminar marcados ({checked})"),
                icon: "trash-2",
                primary: false,
                action: (checked > 0 && !report.still_installed)
                    .then(|| Action::AskClean(name.clone())),
            },
            Btn {
                label: "Eliminar todo rastro".into(),
                icon: "trash-2",
                primary: true,
                action: (!report.still_installed).then_some(Action::AskCleanAll(name)),
            },
        ],
        cancel: Action::CloseTraces,
    }
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
            "{} sigue instalada, así que esto aún no son restos: lo usa la propia app. Desinstálala primero (botón «Desinstalar» de la lista) y vuelve a buscar; entonces se podrá quitar lo que quede.",
            report.app
        );
    }
    let mut body = "Esto es lo que quedó después de desinstalar. Marca lo que quieras quitar, o elimina todo de una vez sin elegir.".to_string();
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
    let mut previous_registry = None;
    report
        .listed()
        .into_iter()
        .map(|t| {
            let registry = t.is_registry();
            let heading =
                (previous_registry != Some(registry)).then(|| group_heading(report, registry));
            previous_registry = Some(registry);
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

/// «Archivos y carpetas (7) · 196 MB» or «Registro (10)»: how many there are of the group
/// in the whole report, not only in the list.
fn group_heading(report: &CleanReport, registry: bool) -> String {
    let group = report
        .proposed
        .iter()
        .filter(|t| t.is_registry() == registry);
    let counts = Counts::of(group);
    if registry {
        format!("Registro ({})", counts.registry)
    } else {
        format!(
            "Archivos y carpetas ({}) · {}",
            counts.files,
            size_text(Some(counts.bytes.div_ceil(1024)))
        )
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
