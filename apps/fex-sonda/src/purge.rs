//! `purge`: removes everything the manifest lists, controls included, whatever state an
//! interrupted install or uninstall left. It can be run as many times as needed.

use crate::manifest::ITEMS;
use crate::{removal, say, system};

pub fn purge() -> Result<(), String> {
    if !system::is_admin() {
        return Err("purge necesita administrador: abre la consola como administrador".into());
    }
    let mut failed = Vec::new();
    for row in ITEMS.iter().rev() {
        if let Err(error) = removal::remove(&row.item) {
            // A removal that is refused because the path is not ours stops everything.
            if error.starts_with("se niega") {
                return Err(error);
            }
            failed.push(format!("{}: {error}", row.id));
        }
    }
    match failed.is_empty() {
        true => {
            say::line("Todo lo de Fex Sonda y de sus controles se quitó.");
            Ok(())
        }
        false => Err(format!("quedó algo sin quitar:\n{}", failed.join("\n"))),
    }
}
