//! The history of changes on disk: `%LOCALAPPDATA%\Purifex\history.jsonl`, one
//! JSON object per line. Lines are only added, so what was recorded is never
//! rewritten; a damaged line is reported, not skipped, because the history is
//! what undoing relies on.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;

use purifex_domain::fix::HistoryEntry;
use purifex_domain::ports::HistoryStore;

pub struct JsonlHistory {
    path: Option<PathBuf>,
}

impl JsonlHistory {
    pub fn new() -> Self {
        let path = std::env::var_os("LOCALAPPDATA")
            .map(|base| PathBuf::from(base).join("Purifex").join("history.jsonl"));
        Self { path }
    }

    pub fn in_file(path: PathBuf) -> Self {
        Self { path: Some(path) }
    }

    fn path(&self) -> Result<&PathBuf, String> {
        self.path
            .as_ref()
            .ok_or_else(|| "no se encuentra la carpeta de datos del usuario".to_string())
    }
}

impl Default for JsonlHistory {
    fn default() -> Self {
        Self::new()
    }
}

impl HistoryStore for JsonlHistory {
    fn entries(&self) -> Result<Vec<HistoryEntry>, String> {
        let path = self.path()?;
        let text = match fs::read_to_string(path) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(format!("no se pudo leer {}: {e}", path.display())),
        };
        text.lines()
            .enumerate()
            .filter(|(_, line)| !line.trim().is_empty())
            .map(|(i, line)| {
                HistoryEntry::from_line(line).map_err(|e| {
                    format!("la línea {} de {} está dañada: {e}", i + 1, path.display())
                })
            })
            .collect()
    }

    fn append(&mut self, entry: &HistoryEntry) -> Result<(), String> {
        let path = self.path()?;
        if let Some(folder) = path.parent() {
            fs::create_dir_all(folder)
                .map_err(|e| format!("no se pudo crear {}: {e}", folder.display()))?;
        }
        let line = entry.to_line();
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .map_err(|e| format!("no se pudo abrir {}: {e}", path.display()))?;
        writeln!(file, "{line}").map_err(|e| format!("no se pudo escribir en el historial: {e}"))
    }
}

#[cfg(test)]
mod tests {
    use purifex_domain::fix::{HistoryState, SettingValue};

    use super::*;

    fn entry(seq: u64) -> HistoryEntry {
        HistoryEntry {
            seq,
            fix: "x".into(),
            before: SettingValue::Bool(false),
            after: SettingValue::Bool(true),
            state: HistoryState::Applied,
            undoes: None,
            session: Some("lol-1".into()),
        }
    }

    fn file(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("purifex-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir.join("history.jsonl")
    }

    #[test]
    fn lines_are_added_in_order_and_read_back() {
        let path = file("hist-add");
        let mut h = JsonlHistory::in_file(path.clone());
        assert!(
            h.entries().unwrap().is_empty(),
            "no file yet is an empty history"
        );
        h.append(&entry(1)).unwrap();
        h.append(&entry(2)).unwrap();
        assert_eq!(h.entries().unwrap(), [entry(1), entry(2)]);
        assert_eq!(
            fs::read_to_string(&path).unwrap().lines().count(),
            2,
            "one line each"
        );
        let _ = fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn a_damaged_line_is_an_error_that_says_which_and_what_was_there_stays() {
        let path = file("hist-bad");
        let mut h = JsonlHistory::in_file(path.clone());
        h.append(&entry(1)).unwrap();
        fs::write(
            &path,
            format!(
                "{}\nesto no es json\n",
                fs::read_to_string(&path).unwrap().trim_end()
            ),
        )
        .unwrap();
        let error = h.entries().unwrap_err();
        assert!(
            error.contains("la línea 2") && error.contains("dañada"),
            "{error}"
        );
        assert!(
            fs::read_to_string(&path)
                .unwrap()
                .contains("esto no es json"),
            "nothing is rewritten"
        );
        let _ = fs::remove_dir_all(path.parent().unwrap());
    }
}
