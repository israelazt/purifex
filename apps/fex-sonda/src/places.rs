//! Where each `Root` of the manifest is on this computer, read from the environment.

use std::path::{Path, PathBuf};

use crate::manifest::Root;

fn env(name: &str) -> Result<PathBuf, String> {
    std::env::var_os(name)
        .map(PathBuf::from)
        .ok_or_else(|| format!("falta la variable de entorno {name}"))
}

pub fn root(root: Root) -> Result<PathBuf, String> {
    match root {
        Root::ProgramFiles => env("ProgramFiles"),
        Root::AppData => env("APPDATA"),
        Root::LocalAppData => env("LOCALAPPDATA"),
        Root::ProgramData => env("ProgramData"),
        Root::StartMenuCommon => {
            Ok(env("ProgramData")?.join(r"Microsoft\Windows\Start Menu\Programs"))
        }
    }
}

/// A path of the manifest, from its root.
pub fn path_of(at: Root, relative: &str) -> Result<PathBuf, String> {
    Ok(root(at)?.join(relative))
}

/// Where Fex Sonda is installed.
pub fn install_dir() -> Result<PathBuf, String> {
    path_of(Root::ProgramFiles, "FexSonda")
}

/// A folder and the parents it needs.
pub fn make_parents(file: &Path) -> Result<(), String> {
    match file.parent() {
        Some(parent) => std::fs::create_dir_all(parent)
            .map_err(|e| format!("no se pudo crear {}: {e}", parent.display())),
        None => Ok(()),
    }
}
