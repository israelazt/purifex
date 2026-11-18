//! The recordings on disk, as the archive the application reads: the CSV files
//! of the sessions folder (see `session_file`).

use std::fs;
use std::path::PathBuf;

use purifex_domain::ports::{SessionArchive, SessionInfo};
use purifex_domain::session_id::is_valid_id;

use crate::session_file::sessions_folder;

pub struct LocalSessionArchive {
    /// Another folder than the user's data one (for the tests).
    folder: Option<PathBuf>,
}

impl LocalSessionArchive {
    pub fn new() -> Self {
        Self { folder: None }
    }

    /// Reads from `folder` instead of the user's data folder.
    pub fn in_folder(folder: PathBuf) -> Self {
        Self {
            folder: Some(folder),
        }
    }

    fn folder(&self) -> Result<PathBuf, String> {
        match &self.folder {
            Some(folder) => Ok(folder.clone()),
            None => sessions_folder(),
        }
    }
}

impl Default for LocalSessionArchive {
    fn default() -> Self {
        Self::new()
    }
}

impl SessionArchive for LocalSessionArchive {
    fn list(&self) -> Result<Vec<SessionInfo>, String> {
        let folder = self.folder()?;
        let entries = match fs::read_dir(&folder) {
            Ok(entries) => entries,
            // No folder yet: nothing has been recorded.
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(format!("no se pudo leer {}: {error}", folder.display())),
        };
        let mut sessions = Vec::new();
        for entry in entries.flatten() {
            let path = entry.path();
            let Some(id) = path
                .extension()
                .is_some_and(|e| e == "csv")
                .then(|| path.file_stem().map(|s| s.to_string_lossy().into_owned()))
                .flatten()
                .filter(|id| is_valid_id(id))
            else {
                continue;
            };
            let Ok(meta) = entry.metadata() else { continue };
            sessions.push(SessionInfo {
                has_summary: folder.join(format!("{id}.summary.json")).is_file(),
                bytes: meta.len(),
                id,
            });
        }
        Ok(sessions)
    }

    fn read_csv(&self, id: &str) -> Result<String, String> {
        if !is_valid_id(id) {
            return Err(format!("«{id}» no es un nombre de grabación válido"));
        }
        let path = self.folder()?.join(format!("{id}.csv"));
        fs::read_to_string(&path).map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                format!("no existe la grabación «{id}»")
            } else {
                format!("no se pudo leer {}: {error}", path.display())
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_folder(name: &str) -> PathBuf {
        let folder = std::env::temp_dir().join(format!("purifex-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&folder);
        fs::create_dir_all(&folder).unwrap();
        folder
    }

    #[test]
    fn it_lists_the_csv_files_and_says_which_have_a_summary() {
        let folder = temp_folder("archive-list");
        fs::write(folder.join("lol-20261118-145915.csv"), "abc").unwrap();
        fs::write(folder.join("lol-20261118-145915.summary.json"), "{}").unwrap();
        fs::write(folder.join("lol-20261118-165329.csv"), "abcdef").unwrap();
        fs::write(folder.join("notes.txt"), "x").unwrap();
        fs::write(folder.join("bad name.csv"), "x").unwrap();

        let mut sessions = LocalSessionArchive::in_folder(folder.clone())
            .list()
            .unwrap();
        sessions.sort_by(|a, b| a.id.cmp(&b.id));
        assert_eq!(
            sessions,
            [
                SessionInfo {
                    id: "lol-20261118-145915".into(),
                    bytes: 3,
                    has_summary: true
                },
                SessionInfo {
                    id: "lol-20261118-165329".into(),
                    bytes: 6,
                    has_summary: false
                },
            ]
        );
        let _ = fs::remove_dir_all(folder);
    }

    #[test]
    fn a_folder_that_does_not_exist_is_an_empty_archive() {
        let missing = std::env::temp_dir().join("purifex-no-such-folder-xyz");
        assert!(LocalSessionArchive::in_folder(missing)
            .list()
            .unwrap()
            .is_empty());
    }

    #[test]
    fn a_name_that_is_a_path_is_refused_and_a_missing_one_is_said() {
        let folder = temp_folder("archive-read");
        fs::write(folder.join("a.csv"), "t_s").unwrap();
        let archive = LocalSessionArchive::in_folder(folder.clone());
        assert_eq!(archive.read_csv("a").unwrap(), "t_s");
        for bad in ["..\\a", "../a", "a.csv", "C:\\x"] {
            assert!(
                archive
                    .read_csv(bad)
                    .unwrap_err()
                    .contains("no es un nombre"),
                "{bad}"
            );
        }
        assert!(archive.read_csv("zzz").unwrap_err().contains("no existe"));
        let _ = fs::remove_dir_all(folder);
    }
}
