//! The logs of installations: `%LOCALAPPDATA%\Purifex\logs\<id>.jsonl`, one log (one line) per
//! file, so deleting, exporting and importing one is just a file.

use std::fs;
use std::path::PathBuf;

use purifex_domain::monitor::InstallLog;
use purifex_domain::ports::InstallLogs;

/// A file from outside is read up to this size.
const MAX_IMPORT: u64 = 64 * 1024 * 1024;

pub struct LocalInstallLogs {
    base: Option<PathBuf>,
    /// Where exports go (the user's Documents when none).
    documents: Option<PathBuf>,
}

impl LocalInstallLogs {
    pub fn new() -> Self {
        Self {
            documents: None,
            base: std::env::var_os("LOCALAPPDATA")
                .map(|b| PathBuf::from(b).join("Purifex").join("logs")),
        }
    }

    #[cfg(test)]
    fn in_dir(base: PathBuf, documents: PathBuf) -> Self {
        Self {
            base: Some(base),
            documents: Some(documents),
        }
    }

    fn dir(&self) -> Result<&PathBuf, String> {
        self.base
            .as_ref()
            .ok_or_else(|| "no se encuentra la carpeta de datos del usuario".to_string())
    }

    /// The file of a log. An id is only letters, digits and dashes: it never leaves the folder.
    fn file(&self, id: &str) -> Result<PathBuf, String> {
        let safe = !id.is_empty() && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-');
        if !safe {
            return Err("nombre de registro no válido".into());
        }
        Ok(self.dir()?.join(format!("{id}.jsonl")))
    }
}

impl Default for LocalInstallLogs {
    fn default() -> Self {
        Self::new()
    }
}

fn read(path: &std::path::Path) -> Result<InstallLog, String> {
    let text = fs::read_to_string(path).map_err(|e| e.to_string())?;
    InstallLog::from_line(text.lines().next().unwrap_or_default())
}

impl InstallLogs for LocalInstallLogs {
    fn all(&self) -> Result<Vec<InstallLog>, String> {
        let Ok(read_dir) = fs::read_dir(self.dir()?) else {
            return Ok(Vec::new());
        };
        let mut logs: Vec<InstallLog> = read_dir
            .flatten()
            .filter(|e| e.path().extension().is_some_and(|x| x == "jsonl"))
            .filter_map(|e| read(&e.path()).ok())
            .collect();
        logs.sort_by(|a, b| b.id.cmp(&a.id));
        Ok(logs)
    }

    fn save(&self, log: &InstallLog) -> Result<(), String> {
        let file = self.file(&log.id)?;
        fs::create_dir_all(self.dir()?).map_err(|e| e.to_string())?;
        fs::write(file, format!("{}\n", log.to_line())).map_err(|e| e.to_string())
    }

    fn delete(&self, id: &str) -> Result<(), String> {
        fs::remove_file(self.file(id)?).map_err(|e| e.to_string())
    }

    fn export(&self, id: &str) -> Result<String, String> {
        let documents = self.documents.clone().or_else(|| {
            std::env::var_os("USERPROFILE").map(|p| PathBuf::from(p).join("Documents"))
        });
        let target = documents
            .ok_or("no se encuentra la carpeta Documentos")?
            .join(format!("purifex-registro-{id}.jsonl"));
        fs::copy(self.file(id)?, &target).map_err(|e| e.to_string())?;
        Ok(target.to_string_lossy().into_owned())
    }

    fn import(&self, file: &str) -> Result<InstallLog, String> {
        let size = fs::metadata(file).map_err(|e| e.to_string())?.len();
        if size > MAX_IMPORT {
            return Err("el archivo es demasiado grande para ser un registro".into());
        }
        let mut log = read(std::path::Path::new(file))?;
        // From outside: it never counts as made here, and its id is its own (never a path).
        log.own = false;
        log.id = format!(
            "importado-{}",
            log.id
                .chars()
                .filter(|c| c.is_ascii_alphanumeric() || *c == '-')
                .collect::<String>()
        );
        self.save(&log)?;
        Ok(log)
    }
}

#[cfg(test)]
mod tests {
    use purifex_domain::app_traces::TraceKind;
    use purifex_domain::monitor::Mark;

    use super::*;

    fn log(id: &str) -> InstallLog {
        InstallLog {
            id: id.into(),
            installer: "setup.exe".into(),
            app: Some("Zed".into()),
            on: None,
            own: true,
            marks: vec![Mark {
                kind: TraceKind::Folder,
                location: r"C:\ProgramData\Zed".into(),
                value: None,
                stamp: "1".into(),
                bytes: 0,
            }],
        }
    }

    #[test]
    fn logs_are_saved_listed_exported_imported_as_not_own_and_deleted() {
        let dir = std::env::temp_dir().join(format!("purifex-logs-{}", std::process::id()));
        let store = LocalInstallLogs::in_dir(dir.join("logs"), dir.clone());
        store.save(&log("2026-11-23-1")).unwrap();
        store.save(&log("2026-11-23-2")).unwrap();
        assert_eq!(
            store
                .all()
                .unwrap()
                .iter()
                .map(|l| l.id.as_str())
                .collect::<Vec<_>>(),
            ["2026-11-23-2", "2026-11-23-1"]
        );
        let out = store.export("2026-11-23-1").unwrap();
        assert!(out.ends_with("purifex-registro-2026-11-23-1.jsonl"));
        let imported = store.import(&out).unwrap();
        assert!(!imported.own && imported.id == "importado-2026-11-23-1");
        assert_eq!(store.all().unwrap().len(), 3);
        store.delete("2026-11-23-2").unwrap();
        assert!(store.delete("..\\x").is_err() && store.delete("").is_err());
        assert_eq!(store.all().unwrap().len(), 2);
        let _ = fs::remove_dir_all(dir);
    }
}
