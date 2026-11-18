//! Opening the window: Edge in "app" mode (no address bar, no tabs), which every
//! Windows 11 has. It gets its own profile folder so it never touches the user's
//! browsing; if Edge is missing, the default browser opens the address.

use std::path::PathBuf;
use std::process::{Child, Command, Stdio};

fn edge_path() -> Option<PathBuf> {
    ["ProgramFiles(x86)", "ProgramFiles"]
        .iter()
        .filter_map(std::env::var_os)
        .map(|base| PathBuf::from(base).join(r"Microsoft\Edge\Application\msedge.exe"))
        .find(|path| path.is_file())
}

/// Opens `url` in a window of its own. The process of the window, when there is
/// one to follow (the default browser gives none).
pub fn open(url: &str) -> Result<Option<Child>, String> {
    if let Some(edge) = edge_path() {
        let profile = std::env::var_os("LOCALAPPDATA")
            .map(|base| PathBuf::from(base).join("Purifex").join("edge-profile"))
            .ok_or("no se encuentra la carpeta de datos del usuario")?;
        let child = Command::new(edge)
            .arg(format!("--app={url}"))
            .arg(format!("--user-data-dir={}", profile.display()))
            .args([
                "--no-first-run",
                "--no-default-browser-check",
                "--window-size=1440,920",
            ])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("no se pudo abrir Edge: {e}"))?;
        return Ok(Some(child));
    }
    Command::new("cmd")
        .args(["/C", "start", "", url])
        .spawn()
        .map_err(|e| format!("no se pudo abrir el navegador: {e}"))?;
    Ok(None)
}
