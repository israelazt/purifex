//! `install` and `run`: what Fex Sonda does to look like an app that was installed and used.

use crate::items::{self, Context};
use crate::manifest::{by_id, Hive, Item};
use crate::{places, system};

/// What `.fexcomun` belonged to before Fex Sonda took it, kept to give it back.
const PREVIOUS: &str = "PrevFexcomun";
const FEXCOMUN: &str = r"SOFTWARE\Classes\.fexcomun";
const OWN_KEY: &str = r"SOFTWARE\FexSonda";

/// What belongs to other programs, written first and only if it is not there already.
const CONTROLS: [&str; 6] = ["C1a", "C1b", "C1", "C2", "C2b", "C2c"];
/// What Fex Sonda writes when it is installed, in order.
const INSTALLED: [&str; 9] = ["1", "2", "3", "6", "8", "9", "10", "11", "12"];
/// What it writes when it is used.
const USED: [&str; 3] = ["4", "5", "7"];

fn item_of(id: &str) -> Result<&'static Item, String> {
    by_id(id)
        .map(|row| &row.item)
        .ok_or_else(|| format!("la fila {id} no está en el manifiesto"))
}

fn context() -> Result<Context, String> {
    Ok(Context {
        install_dir: places::install_dir()?.to_string_lossy().into_owned(),
        exe: std::env::current_exe()
            .map_err(|e| format!("no se sabe dónde está el programa: {e}"))?,
    })
}

fn write_all(ids: &[&str], ctx: &Context) -> Result<(), String> {
    ids.iter()
        .try_for_each(|id| items::write(item_of(id)?, ctx))
}

/// The controls that are not on the computer yet.
fn write_controls(ctx: &Context) -> Result<(), String> {
    for id in CONTROLS {
        let item = item_of(id)?;
        if !items::exists(item)? {
            items::write(item, ctx)?;
        }
    }
    Ok(())
}

/// Takes `.fexcomun`: whoever had it is remembered once, so installing twice does not lose it.
fn take_fexcomun() -> Result<(), String> {
    let kept = system::read_value(Hive::Hklm, OWN_KEY, PREVIOUS).is_some();
    if !kept {
        let owner = system::read_value(Hive::Hklm, FEXCOMUN, "").unwrap_or_default();
        system::set_text(Hive::Hklm, OWN_KEY, PREVIOUS, &owner)?;
    }
    system::set_text(Hive::Hklm, FEXCOMUN, "", "FexSonda.Doc")
}

pub fn install() -> Result<(), String> {
    if !system::is_admin() {
        return Err("install necesita administrador: abre la consola como administrador".into());
    }
    let ctx = context()?;
    write_controls(&ctx)?;
    write_all(&INSTALLED, &ctx)?;
    take_fexcomun()?;
    usage()
}

/// Writes what using the app leaves: its settings, its cache and its last run.
pub fn usage() -> Result<(), String> {
    write_all(&USED, &context()?)
}
