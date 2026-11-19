//! The settings the fixes change, one adapter each, and the registry that finds
//! the one a fix id names.

mod history;
mod lol_client;

use std::path::PathBuf;

use purifex_domain::fix::{SettingValue, LOL_CLOSE_CLIENT};
use purifex_domain::ports::FixBackend;

pub use history::JsonlHistory;
use lol_client::LolClient;

/// Where the copies of the files a fix changes are kept.
fn backups_folder() -> Result<PathBuf, String> {
    let base = std::env::var_os("LOCALAPPDATA")
        .ok_or("no se encuentra la carpeta de datos del usuario")?;
    Ok(PathBuf::from(base).join("Purifex").join("backups"))
}

pub struct SystemFixes {
    lol: Option<LolClient>,
}

impl SystemFixes {
    pub fn new() -> Self {
        Self {
            lol: backups_folder().ok().map(LolClient::new),
        }
    }

    fn lol(&self) -> Result<&LolClient, String> {
        self.lol
            .as_ref()
            .ok_or_else(|| "no se encuentra la carpeta de datos del usuario".to_string())
    }
}

impl Default for SystemFixes {
    fn default() -> Self {
        Self::new()
    }
}

fn unknown(id: &str) -> String {
    format!("no se conoce el ajuste «{id}»")
}

impl FixBackend for SystemFixes {
    fn current(&self, id: &str) -> Result<SettingValue, String> {
        match id {
            LOL_CLOSE_CLIENT => self.lol()?.current(),
            _ => Err(unknown(id)),
        }
    }

    fn blockers(&self, id: &str) -> Result<Vec<String>, String> {
        match id {
            LOL_CLOSE_CLIENT => self.lol()?.blockers(),
            _ => Err(unknown(id)),
        }
    }

    fn set(&mut self, id: &str, value: &SettingValue) -> Result<(), String> {
        match id {
            LOL_CLOSE_CLIENT => self.lol()?.set(value),
            _ => Err(unknown(id)),
        }
    }
}
