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
}

fn folder() -> Result<PathBuf, String> {
    let base = std::env::var_os("LOCALAPPDATA")
        .ok_or("no se encuentra la carpeta de datos del usuario")?;
    Ok(PathBuf::from(base).join("Purifex").join("sessions"))
}

impl SessionFile for LocalSessionFile {
    fn create(&mut self, name: &str) -> Result<String, String> {
        let folder = folder()?;
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
        Ok(path.display().to_string())
    }

    fn append(&mut self, line: &str) -> Result<(), String> {
        let file = self
            .file
            .as_mut()
            .ok_or("el archivo de la sesión no está creado")?;
        writeln!(file, "{line}").map_err(|e| e.to_string())
    }
}
