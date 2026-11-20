//! What a set of traces says as text: how many of each kind and how big, and the list as a
//! terminal prints it. The window builds its own wording (`purifex-ui`); this is the
//! command line and the logs.

use crate::app_traces::Trace;
use crate::apps::InstalledApp;

/// How many traces of each kind, and their size.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Counts {
    /// Files and folders.
    pub files: usize,
    /// Registry keys and values.
    pub registry: usize,
    pub bytes: u64,
}

impl Counts {
    pub fn of<'a>(traces: impl IntoIterator<Item = &'a Trace>) -> Counts {
        let mut c = Counts::default();
        for t in traces {
            if t.is_registry() {
                c.registry += 1;
            } else {
                c.files += 1;
            }
            c.bytes += t.bytes;
        }
        c
    }

    /// `Archivos y carpetas: 18 · Registro: 19 · 83 KB`
    pub fn to_text(&self) -> String {
        format!(
            "Archivos y carpetas: {} · Registro: {} · {}",
            self.files,
            self.registry,
            crate::apps::size_text(Some(self.bytes.div_ceil(1024)))
        )
    }
}

pub fn traces_to_text(app: &InstalledApp, traces: &[Trace]) -> String {
    traces_to_text_for(&app.name, traces)
}

pub fn traces_to_text_for(app: &str, traces: &[Trace]) -> String {
    use std::fmt::Write;
    if traces.is_empty() {
        return format!("No se encontraron restos de {app}.");
    }
    let mut out = format!("Restos de {app} (nada se ha borrado):\n");
    for t in traces {
        let user = if t.user_data { "  [datos tuyos]" } else { "" };
        let admin = if t.needs_admin() {
            "  [requiere administrador]"
        } else {
            ""
        };
        let mark = format!("{user}{admin}");
        let place = match &t.value {
            Some(v) => format!("{}  [valor: {v}]", t.location),
            None => t.location.clone(),
        };
        let _ = writeln!(
            out,
            "• {place}  {}{mark}\n    evidencia: {} · por qué: {}",
            crate::apps::size_text(Some(t.bytes / 1024)),
            t.evidence.label(),
            t.why
        );
    }
    let _ = writeln!(out, "\n{}", Counts::of(traces).to_text());
    let admin = traces.iter().filter(|t| t.needs_admin()).count();
    if admin > 0 {
        let _ = writeln!(
            out,
            "{admin} de estos restos solo los quita un administrador: abre Purifex como administrador para quitarlos."
        );
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_traces::{Evidence, TraceKind};

    fn counted(kind: TraceKind, bytes: u64, value: Option<&str>) -> Trace {
        Trace {
            kind,
            location: r"HKCU\Software\X".into(),
            bytes,
            why: "w".into(),
            value: value.map(String::from),
            evidence: Evidence::NameMatch,
            ..Default::default()
        }
    }

    #[test]
    fn counts_split_files_from_registry_and_the_text_says_so() {
        let traces = [
            counted(TraceKind::Folder, 2048, None),
            counted(TraceKind::File, 1024, None),
            counted(TraceKind::RegistryKey, 0, None),
            counted(TraceKind::RegistryValue, 0, Some("v")),
            counted(TraceKind::RegistryValue, 0, Some("w")),
        ];
        let c = Counts::of(&traces);
        assert_eq!((c.files, c.registry, c.bytes), (2, 3, 3072));
        assert_eq!(c.to_text(), "Archivos y carpetas: 2 · Registro: 3 · 3 KB");
        let text = traces_to_text_for("X", &traces);
        assert!(text.contains("[valor: v]"));
        assert!(text.contains("Archivos y carpetas: 2 · Registro: 3"));
        assert_eq!(
            Counts::of(&[]).to_text(),
            "Archivos y carpetas: 0 · Registro: 0 · –"
        );
    }
}
