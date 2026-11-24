//! `uninstall`: what the official uninstaller of a real app does, and nothing more. It removes the
//! program and its entry in «Apps», gives `.fexcomun` back to whoever had it, and forgets the rest
//! on purpose: that is what Purifex must then find.

use crate::manifest::{by_id, Hive};
use crate::{removal, system};

/// What the uninstaller takes away.
const REMOVED: [&str; 2] = ["1", "3"];
const FEXCOMUN: &str = r"SOFTWARE\Classes\.fexcomun";
const OWN_KEY: &str = r"SOFTWARE\FexSonda";
const PREVIOUS: &str = "PrevFexcomun";

/// Gives `.fexcomun` back to the class it had before and forgets that it was kept.
fn give_fexcomun_back() -> Result<(), String> {
    let Some(owner) = system::read_value(Hive::Hklm, OWN_KEY, PREVIOUS) else {
        return Ok(());
    };
    system::set_text(Hive::Hklm, FEXCOMUN, "", &owner)?;
    system::delete_value(Hive::Hklm, OWN_KEY, PREVIOUS)
}

pub fn uninstall() -> Result<(), String> {
    if !system::is_admin() {
        return Err("uninstall necesita administrador: abre la consola como administrador".into());
    }
    for id in REMOVED {
        let row = by_id(id).ok_or_else(|| format!("la fila {id} no está en el manifiesto"))?;
        removal::remove(&row.item)?;
    }
    give_fexcomun_back()
}
