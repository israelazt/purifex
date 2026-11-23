//! Starting the installer the person chose.

use std::path::Path;
use std::process::Command;

use purifex_domain::ports::InstallerRunner;

pub struct ShellInstaller;

impl InstallerRunner for ShellInstaller {
    fn run(&self, installer: &str) -> Result<(), String> {
        let path = Path::new(installer);
        let extension = path
            .extension()
            .map(|e| e.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        if !path.is_file() {
            return Err(format!("no existe el archivo {installer}"));
        }
        let started = match extension.as_str() {
            "exe" => Command::new(path).spawn(),
            "msi" => Command::new("msiexec").arg("/i").arg(path).spawn(),
            _ => return Err("solo se ejecutan instaladores .exe y .msi".into()),
        };
        started
            .map(|_| ())
            .map_err(|e| format!("no se pudo iniciar el instalador: {e}"))
    }
}
