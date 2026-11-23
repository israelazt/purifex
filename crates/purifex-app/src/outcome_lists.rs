//! The read-only lists of the command line: what starts with Windows, the logs of installations
//! and the junk of Windows, as text and as JSON.

use purifex_domain::apps::size_text;
use purifex_domain::junk::{items_text, rows, JunkItem};
use purifex_domain::monitor::InstallLog;
use purifex_domain::startup::StartupItem;

pub fn startup_to_text(items: &[StartupItem]) -> String {
    if items.is_empty() {
        return "No hay nada que arranque con Windows.".into();
    }
    items
        .iter()
        .map(|i| {
            let state = if i.enabled {
                "activa    "
            } else {
                "desactivada"
            };
            let app = i
                .app
                .as_deref()
                .map_or(String::new(), |a| format!("  [{a}]"));
            format!(
                "{state}  {:<14} {}  {}{app}",
                i.kind.label(),
                i.name,
                i.program()
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn startup_to_json(items: &[StartupItem]) -> serde_json::Value {
    let list: Vec<_> = items
        .iter()
        .map(|i| {
            serde_json::json!({
                "name": i.name, "kind": i.kind.label(), "place": i.place,
                "command": i.command, "enabled": i.enabled, "app": i.app,
                "can_toggle": i.can_toggle,
            })
        })
        .collect();
    serde_json::json!({ "startup": list })
}

pub fn logs_to_text(logs: &[InstallLog]) -> String {
    if logs.is_empty() {
        return "No hay registros de instalaciones.".into();
    }
    logs.iter()
        .map(|l| {
            let name = l.app.as_deref().unwrap_or(&l.installer);
            let from = if l.own { "" } else { " (importado)" };
            format!("{}  {name}{from}: {}", l.id, l.counts_text())
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn junk_to_text(items: &[JunkItem]) -> String {
    let lines: Vec<String> = rows(items)
        .iter()
        .map(|r| {
            format!(
                "{:<34} {:>14} {:>10}",
                r.kind.label(),
                items_text(r.count),
                size_text(Some(r.bytes.div_ceil(1024)))
            )
        })
        .collect();
    format!(
        "{}\nNo se mueve nada: la limpieza se hace desde la página «Limpieza».",
        lines.join("\n")
    )
}

pub fn junk_to_json(items: &[JunkItem]) -> serde_json::Value {
    let list: Vec<_> = rows(items)
        .iter()
        .map(|r| serde_json::json!({ "kind": r.kind.label(), "count": r.count, "bytes": r.bytes }))
        .collect();
    serde_json::json!({ "junk": list })
}
