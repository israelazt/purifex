//! Recordings on disk: `%LOCALAPPDATA%\Purifex\sessions\<name>-<date>-<time>.csv`,
//! written a line at a time so that nothing is lost if the program is closed.

use std::fs::{self, File};
use std::io::Write;
use std::path::PathBuf;

use purifex_domain::ports::SessionFile;
use windows::Win32::System::SystemInformation::GetLocalTime;

#[derive(Default)]
pub struct LocalSessionFile {
    file: Option<File>,
    path: Option<PathBuf>,
    /// Another folder than the user's data one (for the tests).
    folder: Option<PathBuf>,
}

impl LocalSessionFile {
    /// Writes in `folder` instead of the user's data folder.
    pub fn in_folder(folder: PathBuf) -> Self {
        Self {
            folder: Some(folder),
            ..Self::default()
        }
    }
}

/// Where the recordings are kept: `%LOCALAPPDATA%\\Purifex\\sessions`.
pub(crate) fn sessions_folder() -> Result<PathBuf, String> {
    let base = std::env::var_os("LOCALAPPDATA")
        .ok_or("no se encuentra la carpeta de datos del usuario")?;
    Ok(PathBuf::from(base).join("Purifex").join("sessions"))
}

impl SessionFile for LocalSessionFile {
    fn create(&mut self, name: &str) -> Result<String, String> {
        let folder = match &self.folder {
            Some(folder) => folder.clone(),
            None => sessions_folder()?,
        };
        fs::create_dir_all(&folder)
            .map_err(|e| format!("no se pudo crear {}: {e}", folder.display()))?;
        // SAFETY: a plain query with no arguments.
        let t = unsafe { GetLocalTime() };
        let file_name = format!(
            "{name}-{:04}{:02}{:02}-{:02}{:02}{:02}.csv",
            t.wYear, t.wMonth, t.wDay, t.wHour, t.wMinute, t.wSecond
        );
        let path = folder.join(file_name);
        self.file = Some(
            File::create(&path).map_err(|e| format!("no se pudo crear {}: {e}", path.display()))?,
        );
        let shown = path.display().to_string();
        self.path = Some(path);
        Ok(shown)
    }

    fn append(&mut self, line: &str) -> Result<(), String> {
        let file = self
            .file
            .as_mut()
            .ok_or("el archivo de la sesión no está creado")?;
        writeln!(file, "{line}").map_err(|e| e.to_string())
    }

    fn write_summary(&mut self, json: &str) -> Result<(), String> {
        let csv = self
            .path
            .as_ref()
            .ok_or("el archivo de la sesión no está creado")?;
        let summary = csv.with_extension("summary.json");
        fs::write(&summary, json)
            .map_err(|e| format!("no se pudo escribir {}: {e}", summary.display()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_recording_is_written_line_by_line_with_its_summary_beside_it() {
        let folder = std::env::temp_dir().join(format!("purifex-test-{}", std::process::id()));
        let mut file = LocalSessionFile::in_folder(folder.clone());
        let shown = file.create("lol").unwrap();
        file.append("a,b").unwrap();
        file.append("1,2").unwrap();
        file.write_summary("{\"samples\": 1}").unwrap();

        let csv = PathBuf::from(&shown);
        assert_eq!(
            fs::read_to_string(&csv).unwrap(),
            "a,b
1,2
"
        );
        let summary = csv.with_extension("summary.json");
        assert!(summary.to_string_lossy().ends_with(".summary.json"));
        assert_eq!(fs::read_to_string(summary).unwrap(), "{\"samples\": 1}");
        assert!(csv
            .file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("lol-"));
        let _ = fs::remove_dir_all(folder);
    }

    #[test]
    fn writing_before_creating_is_an_error_not_a_crash() {
        let mut file = LocalSessionFile::default();
        assert!(file.append("x").is_err());
        assert!(file.write_summary("{}").is_err());
    }
}
