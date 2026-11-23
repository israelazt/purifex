//! The junk of Windows: temporary files, thumbnail caches, crash dumps, the downloads of
//! Windows Update and error reports. Everything here regenerates by itself, but it is still
//! moved to the quarantine (not deleted) so it can be put back. Pure: finding it is a port.

use crate::app_traces::{Trace, TraceKind};
use crate::apps::size_text;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JunkKind {
    Temp,
    Thumbnails,
    CrashDumps,
    UpdateCache,
    ErrorReports,
}

impl JunkKind {
    pub const ALL: [JunkKind; 5] = [
        JunkKind::Temp,
        JunkKind::Thumbnails,
        JunkKind::CrashDumps,
        JunkKind::UpdateCache,
        JunkKind::ErrorReports,
    ];

    pub fn label(self) -> &'static str {
        match self {
            JunkKind::Temp => "Archivos temporales",
            JunkKind::Thumbnails => "Miniaturas e iconos en caché",
            JunkKind::CrashDumps => "Volcados de errores",
            JunkKind::UpdateCache => "Descargas de Windows Update",
            JunkKind::ErrorReports => "Informes de errores de Windows",
        }
    }

    pub fn detail(self) -> &'static str {
        match self {
            JunkKind::Temp => "Lo que los programas dejan en las carpetas Temp, si lleva más de un día sin tocarse.",
            JunkKind::Thumbnails => "Windows las vuelve a crear al abrir las carpetas.",
            JunkKind::CrashDumps => "Copias de la memoria de un programa o del sistema cuando falló.",
            JunkKind::UpdateCache => "Paquetes ya descargados; Windows Update los descarga otra vez si hacen falta.",
            JunkKind::ErrorReports => "Informes que Windows guarda de programas que fallaron.",
        }
    }

    /// Part of it lives in a place only an administrator may change.
    pub fn needs_admin(self) -> bool {
        matches!(self, JunkKind::UpdateCache)
    }
}

/// One thing found: a file or a folder that is a direct child of a place that holds junk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JunkItem {
    pub kind: JunkKind,
    pub location: String,
    pub bytes: u64,
    pub folder: bool,
}

impl JunkItem {
    /// What moving it to the quarantine takes.
    pub fn trace(&self) -> Trace {
        Trace {
            kind: if self.folder {
                TraceKind::Folder
            } else {
                TraceKind::File
            },
            location: self.location.clone(),
            bytes: self.bytes,
            why: self.kind.label().to_string(),
            ..Trace::default()
        }
    }
}

/// How much there is of one kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JunkRow {
    pub kind: JunkKind,
    pub count: usize,
    pub bytes: u64,
}

/// One row per kind, also those with nothing.
pub fn rows(items: &[JunkItem]) -> Vec<JunkRow> {
    JunkKind::ALL
        .iter()
        .map(|&kind| {
            let of_kind = items.iter().filter(|i| i.kind == kind);
            JunkRow {
                kind,
                count: of_kind.clone().count(),
                bytes: of_kind.map(|i| i.bytes).sum(),
            }
        })
        .collect()
}

/// «1 elemento», «3 elementos».
pub fn items_text(n: usize) -> String {
    if n == 1 {
        "1 elemento".to_string()
    } else {
        format!("{n} elementos")
    }
}

/// What a cleaning did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct JunkOutcome {
    pub removed: usize,
    pub bytes: u64,
    /// What could not be moved, with why (in use, needs an administrator).
    pub failed: Vec<(String, String)>,
}

impl JunkOutcome {
    pub fn to_text(&self) -> String {
        let mut text = format!(
            "Se movieron {} ({}) a la cuarentena. El espacio se libera de verdad cuando la vacíes.",
            items_text(self.removed),
            size_text(Some(self.bytes.div_ceil(1024)))
        );
        if !self.failed.is_empty() {
            let admin = self
                .failed
                .iter()
                .filter(|(_, why)| why.contains("administrador"))
                .count();
            let used = self.failed.len() - admin;
            text += &format!(" No se pudieron mover {}", self.failed.len());
            match (used, admin) {
                (u, 0) => text += &format!(" (en uso: {u})."),
                (0, a) => text += &format!(" (hace falta administrador: {a})."),
                (u, a) => {
                    text += &format!(
                        " (en uso o con otro problema: {u}; hace falta administrador: {a})."
                    )
                }
            }
        }
        text
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(kind: JunkKind, location: &str, bytes: u64, folder: bool) -> JunkItem {
        JunkItem {
            kind,
            location: location.into(),
            bytes,
            folder,
        }
    }

    #[test]
    fn rows_add_up_each_kind_and_keep_the_empty_ones() {
        let items = [
            item(JunkKind::Temp, r"C:\T\a", 100, false),
            item(JunkKind::Temp, r"C:\T\b", 50, true),
            item(JunkKind::CrashDumps, r"C:\W\x.dmp", 7, false),
        ];
        let rows = rows(&items);
        assert_eq!(rows.len(), 5);
        assert_eq!((rows[0].count, rows[0].bytes), (2, 150));
        assert_eq!((rows[1].count, rows[3].count), (0, 0));
        assert_eq!(rows[2].bytes, 7);
    }

    #[test]
    fn an_item_becomes_a_trace_of_its_kind_and_only_the_update_cache_needs_admin() {
        let t = item(JunkKind::Temp, r"C:\T\a", 5, true).trace();
        assert_eq!(
            (t.kind, t.location.as_str()),
            (TraceKind::Folder, r"C:\T\a")
        );
        assert_eq!(
            item(JunkKind::Temp, "x", 1, false).trace().kind,
            TraceKind::File
        );
        assert!(JunkKind::ALL.iter().filter(|k| k.needs_admin()).count() == 1);
    }

    #[test]
    fn the_outcome_says_what_moved_and_why_the_rest_did_not() {
        let done = JunkOutcome {
            removed: 3,
            bytes: 3 * 1024 * 1024,
            failed: vec![],
        };
        assert!(done.to_text().contains("3 elementos") && done.to_text().contains("vacíes"));
        let partly = JunkOutcome {
            removed: 1,
            bytes: 10,
            failed: vec![
                ("a".into(), "requiere administrador".into()),
                ("b".into(), "en uso".into()),
            ],
        };
        assert!(partly.to_text().contains("No se pudieron mover 2"));
        assert!(partly.to_text().contains("administrador: 1"));
    }
}
