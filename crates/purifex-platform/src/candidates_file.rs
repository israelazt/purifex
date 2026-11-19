//! The proposed driver candidates on disk: `%LOCALAPPDATA%\Purifex\candidates.jsonl`,
//! one JSON object per line, only ever added to. The MCP server writes it and the
//! window reads it, so each opens the file when it needs it and keeps nothing.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;

use purifex_domain::driver_candidate::Candidate;
use purifex_domain::ports::CandidateStore;

pub struct JsonlCandidates {
    path: Option<PathBuf>,
}

impl JsonlCandidates {
    pub fn new() -> Self {
        let path = std::env::var_os("LOCALAPPDATA")
            .map(|base| PathBuf::from(base).join("Purifex").join("candidates.jsonl"));
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

impl Default for JsonlCandidates {
    fn default() -> Self {
        Self::new()
    }
}

impl CandidateStore for JsonlCandidates {
    fn all(&self) -> Result<Vec<Candidate>, String> {
        let path = self.path()?;
        let text = match fs::read_to_string(path) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(format!("no se pudo leer {}: {e}", path.display())),
        };
        // A damaged line (the file may be edited by hand) is left out, not fatal:
        // a proposal is only a suggestion, unlike the history of changes.
        Ok(text
            .lines()
            .filter(|l| !l.trim().is_empty())
            .filter_map(|l| Candidate::from_line(l).ok())
            .collect())
    }

    fn add(&mut self, candidate: &Candidate) -> Result<(), String> {
        let path = self.path()?;
        if let Some(folder) = path.parent() {
            fs::create_dir_all(folder)
                .map_err(|e| format!("no se pudo crear {}: {e}", folder.display()))?;
        }
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .map_err(|e| format!("no se pudo abrir {}: {e}", path.display()))?;
        writeln!(file, "{}", candidate.to_line())
            .map_err(|e| format!("no se pudo guardar la propuesta: {e}"))
    }
}

#[cfg(test)]
mod tests {
    use purifex_domain::driver_candidate::Source;
    use purifex_domain::version::DriverVersion;

    use super::*;

    fn candidate(url: &str) -> Candidate {
        Candidate {
            device: r"USB\X\1".into(),
            source: Source::Suggested,
            url: url.into(),
            version: DriverVersion::parse("2.0.0.0").unwrap(),
            title: "t".into(),
        }
    }

    fn file(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("purifex-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir.join("candidates.jsonl")
    }

    #[test]
    fn what_is_added_is_read_back_in_order_and_no_file_means_none() {
        let path = file("cand-order");
        let mut store = JsonlCandidates::in_file(path.clone());
        assert_eq!(store.all().unwrap(), vec![]);
        store.add(&candidate("https://www.realtek.com/a")).unwrap();
        store.add(&candidate("https://www.realtek.com/b")).unwrap();
        let back = JsonlCandidates::in_file(path.clone()).all().unwrap();
        assert_eq!(back.len(), 2);
        assert_eq!(back[1].url, "https://www.realtek.com/b");
        let _ = fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn a_damaged_line_is_skipped_and_the_rest_still_reads() {
        let path = file("cand-damaged");
        let mut store = JsonlCandidates::in_file(path.clone());
        store.add(&candidate("https://www.realtek.com/a")).unwrap();
        let mut f = OpenOptions::new().append(true).open(&path).unwrap();
        writeln!(f, "{{rota").unwrap();
        store.add(&candidate("https://www.realtek.com/c")).unwrap();
        assert_eq!(store.all().unwrap().len(), 2);
        let _ = fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn without_a_data_folder_it_says_so() {
        let store = JsonlCandidates { path: None };
        assert!(store.all().unwrap_err().contains("carpeta de datos"));
    }
}
